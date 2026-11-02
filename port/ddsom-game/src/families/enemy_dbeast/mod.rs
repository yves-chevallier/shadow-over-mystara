//! "Displacer Beast" enemy and its parts (ENEMY type 19, 0x71e44). See `docs/families/enemy.md`.
//!
//! A single handler for fourteen variants (`+3`): the beast itself and the parts that accompany it (limbs, projectiles,
//! shadow...), tied to it by `+0x42` (parent) and aiming at a player through `+0x50` (target). Main state `+4` is the
//! one shared by all enemies (0 creation, 2 active, 4 hit, 6 release); in the active state `+5` chooses a group of
//! behaviours and `+6`, `+7` their steps. One file per group:
//!
//! | `+5` | File | Role (h) |
//! |---|---|---|
//! | 0 | `parts` | parts (variants 0 to 13 according to `+6`): limbs, projectiles, leaps |
//! | 2 | `stance` | the standing beast: pursuit, gaze, choice of an attack |
//! | 4 | `attacks` | the beast's attacks (`+6` = the attack) |
//! | 6 | `attacks_b` | second series of attacks |
//! | 8, 10, 12 | `moves` | charges, leaps, entrance |
//! | state 4 | `hit` | reactions to hits |
//!
//! The parts are not separate objects in the original: the same code animates them all.

use crate::families::cx::Bench;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

mod attacks;
mod attacks_b;
mod hit;
mod moves;
mod parts;
mod stance;

/// Fields specific to this type (the common fields are in `enemy_common::f`).
mod g {
    /// AI profile (0 or 1, difficulty of the beast).
    pub const PROFILE: u32 = 0x2f;
    /// Display priority.
    pub const LAYER: u32 = 0x3b;
    /// Parent (word, RAM address) and target (word, player).
    pub const PARENT: u32 = 0x42;
    pub const TARGET: u32 = 0x50;
    /// Damage of the current attack: `+0x44` type, `+0x45` strength.
    pub const STRIKE_KIND: u32 = 0x44;
    pub const STRIKE_POWER: u32 = 0x45;
    /// Vulnerable (1) or invulnerable (0) during the move.
    pub const VULNERABLE: u32 = 0x67;
    /// AI parameters set at creation.
    pub const AI_STEP: u32 = 0x6b;
    pub const AI_REPEAT: u32 = 0x6e;
    pub const AI_TURN: u32 = 0x71;
    pub const AI_KICK: u32 = 0x77;
    pub const AI_SHOT: u32 = 0x79;
    /// Exit countdown of the entrance.
    pub const ENTRY_TIMER: u32 = 0x90;
    /// Timer of the current step (byte or word).
    pub const TIMER: u32 = 0xa0;
    /// Wanted animation / current animation (so as not to restart the same one).
    pub const WANT_ANIM: u32 = 0xa5;
    pub const CUR_ANIM: u32 = 0xa2;
    /// The move is over (set by the beast for its parts).
    pub const DONE: u32 = 0xa3;
    /// Step to resume after a turn-around: `+0xa4` = `+6`, `+0xa6` = `+4..+7`.
    pub const RESUME_SUB2: u32 = 0xa4;
    pub const RESUME_STATE: u32 = 0xa6;
    /// Number of repetitions of an attack (`+0xab`, `+0xae`) and move counter (`+0xb5`).
    pub const REPEAT: u32 = 0xab;
    pub const REPEAT_LEFT: u32 = 0xae;
    pub const SHOTS: u32 = 0xb5;
    /// Aimed point (X, Y) for a part rejoining its parent.
    pub const AIM_X: u32 = 0xb0;
    pub const AIM_Y: u32 = 0xb2;
    /// Rank of the part in the beast (−1: not a part) and offset of the animation series.
    pub const PART: u32 = 0xbc;
    pub const ANIM_SHIFT: u32 = 0xbf;
    /// Animation started last (so as not to restart it).
    pub const LAST_ANIM: u32 = 0xaf;
}

use g::*;
use crate::objects::Current;

/// Animation sets: movements and attacks.
const SET_A: u32 = 0x166fb6;
const SET_B: u32 = 0x167054;
/// Animation-number offsets per series (`+0xbf`), ROM tables.
const SHIFT_A: u32 = 0x74f7a;
const SHIFT_B: u32 = 0x74f8a;
const SHIFT_PICK: u32 = 0x74f56;
/// Long states (`+4..+7`).
const ACTIVE_STANCE: u32 = 0x0202_0000;
const ACTIVE_IDLE: u32 = 0x020a_0000;
/// Hit state, substate 8: the beast is thrown back or runs away.
const KNOCKED: u32 = 0x0408_0000;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        st::DEAD => free(x),
        _ => {}
    }
}

// --- aides ------------------------------------------------------------------------------------

/// 0x74f66: animation number of the movement series (numbers from 0x4b on are not shifted).
fn anim_a<R: Bench>(x: &mut Cx<'_, R>, n: u8) -> u32 {
    let shift = x.u8(ANIM_SHIFT);
    if (n as i8) >= 0x4b {
        return n as u32;
    }
    n.wrapping_add(x.rom8(SHIFT_A + shift as u32)) as u32
}

/// 0x74f7e: animation number of the attack series, always shifted.
fn anim_b<R: Bench>(x: &mut Cx<'_, R>, n: u8) -> u32 {
    let shift = x.u8(ANIM_SHIFT);
    n.wrapping_add(x.rom8(SHIFT_B + shift as u32)) as u32
}

/// Plays animation `n` of the movement series.
fn play<R: Bench>(x: &mut Cx<'_, R>, n: u8) {
    let n = anim_a(x, n);
    x.anim_start(SET_A, n as u16);
}

/// Plays animation `n` of the attack series.
fn play_b<R: Bench>(x: &mut Cx<'_, R>, n: u8) {
    let n = anim_b(x, n);
    x.anim_start(SET_B, n as u16);
}

/// Plays animation `n` (movement series) if it is not already the current one (`+0xaf`).
fn play_once<R: Bench>(x: &mut Cx<'_, R>, n: u8) {
    let n = anim_a(x, n);
    if n as u8 != x.u8(LAST_ANIM) {
        x.set8(LAST_ANIM, n as u8);
        x.anim_start(SET_A, n as u16);
    }
}

/// One animation step (the "finished" result is ignored).
fn step<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
}

/// Movement according to the speed table (X, Y).
fn move_xy<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_by_table_xy();
}

/// Chooses speed table `n`.
fn speed<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
}

/// Sets the three speeds of a leap: X, Z, acceleration in Z.
fn set_velocity<R: Bench>(x: &mut Cx<'_, R>, vx: i16, vz: u16, az: i16) {
    x.set16(0x14, vx as u16);
    x.set16(0x16, 0);
    x.set16(0x18, vz);
    x.set16(0x1a, az as u16);
}

/// The long state (`+4..+7`) of another object.
fn state_of<R: Bench>(x: &mut Cx<'_, R>, o: Obj) -> u32 {
    o.u32(x.b, f::STATE)
}

fn set_state_long<R: Bench>(x: &mut Cx<'_, R>, v: u32) {
    x.set32(f::STATE, v);
}

/// The parent (`+0x42`) or `None` if there is none.
fn parent<R: Bench>(x: &mut Cx<'_, R>) -> Option<Obj> {
    if x.u16(PARENT) == 0 {
        return None;
    }
    Some(x.linked(PARENT))
}

/// 0x725c8: part without a parent, it becomes a standing beast again and plays animation 8.
fn orphan<R: Bench>(x: &mut Cx<'_, R>) {
    set_state_long(x, ACTIVE_STANCE);
    play(x, 8);
}

/// Screen coordinates (camera): X in `A5+0x556`, Y in `A5+0x55a`.
fn camera<R: Bench>(x: &mut Cx<'_, R>) -> (u16, u16) {
    (var::CAMERA_ALT_X.get(x.b), var::CAMERA_ALT_Y.get(x.b))
}

/// 0x71ff4: places itself on the parent, offset according to the rank of the part (X inverted when the part looks
/// left).
fn follow_offset<R: Bench>(x: &mut Cx<'_, R>) {
    const OFFSETS: u32 = 0x7202a;
    let p = x.linked(PARENT);
    let (px, py) = (p.u16(x.b, 0x8), p.u16(x.b, 0x10));
    x.set16(0x8, px);
    x.set16(0x10, py);
    let row = (x.u8(PART).wrapping_mul(4)) as u32;
    let mut dx = x.rom16(OFFSETS + row);
    let dy = x.rom16(OFFSETS + row + 2);
    if x.u8(f::FLIP) != 0 {
        dx = dx.wrapping_neg();
    }
    x.add16(0x8, dx);
    x.add16(0x10, dy);
}

/// 0x7208c: places itself on the parent (position, orientation, altitude) according to the rank of the part. Without a
/// parent, the part becomes a standing beast again (and the function returns control to the caller, which continues).
fn follow_body<R: Bench>(x: &mut Cx<'_, R>) {
    const OFFSETS: u32 = 0x720ee;
    let Some(p) = parent(x) else { return orphan(x) };
    let (px, py, flip) = (p.u16(x.b, 0x8), p.u16(x.b, 0x10), p.u8(x.b, f::FLIP));
    x.set16(0x8, px);
    x.set16(0x10, py);
    x.set8(f::FLIP, flip);
    x.set32(0xc, 0);
    let row = (x.u8(PART).wrapping_mul(8)) as u32;
    let mut dx = x.rom16(OFFSETS + row);
    let dy = x.rom16(OFFSETS + row + 2);
    let dz = x.rom16(OFFSETS + row + 4);
    let fraction = x.rom16(OFFSETS + row + 6) as i16 as i32 as u32;
    if flip != 0 {
        dx = dx.wrapping_neg();
    }
    x.add16(0x8, dx);
    x.add16(0x10, dy);
    x.add16(0xc, dz);
    x.add32(0x10, fraction);
}

/// 0x74eac: the beast must turn around to face its target.
fn player_ahead<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    needs_turn(x)
}

/// 0x74efe: the target is far, in depth and in width.
fn target_far<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let Some(t) = target(x) else { return false };
    let dy = t.u16(x.b, 0x10).wrapping_sub(x.u16(0x10)).wrapping_add(0x30);
    if dy <= 0x60 {
        return false;
    }
    let dx = t.u16(x.b, 0x8).wrapping_sub(x.u16(0x8)).wrapping_add(0xa0);
    dx > 0x140
}

/// 0x74f8e: a player is very close (box in front of the beast).
fn player_near<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    for p in 0..4 {
        let player = Obj(0xff85de + 0x200 * p);
        if player.u8(x.b, 0) != 0 && lib::target_in_box(x, 0, 0x60, 0x28) {
            return true;
        }
    }
    false
}

// --- state 0: creation --------------------------------------------------------------------------

/// AI parameters per profile (`+0x2f`): (field, value).
const PROFILE_A: [(u32, u8); 12] =
    [(0x68, 2), (0x69, 2), (0x6a, 1), (0x6c, 2), (0x6d, 2), (0x72, 2), (0x73, 2), (0x74, 2), (0x75, 0x18), (0x76, 2), (0x81, 4), (0x80, 5)];
const PROFILE_B: [(u32, u8); 12] =
    [(0x68, 2), (0x69, 2), (0x6a, 1), (0x6c, 3), (0x6d, 3), (0x72, 4), (0x73, 4), (0x74, 3), (0x75, 0x20), (0x76, 3), (0x81, 5), (0x80, 6)];

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    x.add8(f::STATE, 2);
    x.set8(f::BOX_SET, 0xc);
    lib::initial_hp(x);
    lib::set_immunities(x);
    x.set8(0x70, 0);
    x.set8(0x7e, 0xa);
    x.set8(0x7f, 0);
    x.set8(0x8e, 0);
    x.set8(0x78, 0xff);
    let profile = match x.u8(PROFILE) {
        0 | 1 => {
            // random offset of this beast's animation series
            let r = (x.random() as u32) as u8 & 0xf;
            let shift = x.rom8(SHIFT_PICK + r as u32);
            x.set8(ANIM_SHIFT, shift);
            if x.u8(PROFILE) == 0 { PROFILE_A } else { PROFILE_B }
        }
        _ => return,
    };
    for (off, v) in profile {
        x.set8(off, v);
    }
    pick_target(x);
    if (x.u8(Obj::VARIANT) as i8) > 3 {
        if x.u16(TARGET) == 0 {
            set_state_long(x, ACTIVE_IDLE);
            return;
        }
        lib::draw_stance(x);
        lib::draw_6b(x);
    }
    parts::spawn(x);
}

// --- state 2: active -----------------------------------------------------------------------------

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
        return held(x);
    }
    if paused(x) {
        return held(x);
    }
    match x.u8(f::SUB) {
        0 => parts::run(x),
        2 => stance::run(x),
        4 => attacks::run(x),
        6 => attacks_b::run(x),
        8 => moves::charge(x),
        10 => moves::enter(x),
        12 => moves::leave(x),
        _ => {}
    }
    // parts at rest (substates 0 and 0xc) do not test the play zone
    let s = x.u16(f::STATE);
    if s != 0x0200 && s != 0x020c {
        lib::keep_in_zone(x);
        lib::collide_terrain(x);
    }
    x.reinsert();
}

/// Freeze or pause: a part waiting for its parent still follows it.
fn held<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB) == 0 && x.u8(f::SUB2) == 2 {
        follow_body(x);
    }
}

handlers! {
    0x71e44 "enemy_19" run,
}
