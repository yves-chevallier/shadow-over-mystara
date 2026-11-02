//! "Kobold" enemy (ENEMY type 0, 0x54914). See `docs/families/enemy.md`.
//!
//! The biggest of the enemies: fourteen arrival variants (`+3`), four AI profiles (`+0x2f`) and about twenty moves.
//! Framework of types 1 and 6 (skeleton, dog) with its own tables.
//!
//! | `+5` | File | Role (h) |
//! |---|---|---|
//! | 0 | `arrival` | arrival according to `+6` (run, jump, fall from above, appearance, undead rising) |
//! | 2 | `stance` | standing: approach and choice of a move, turn-around, wait |
//! | 4 | `attacks` | attacks (`+6` = 0 to 36) |
//! | 6, 8 | `moves` | leaps and movements |
//! | 10 | `moves` | stroll as long as no player is alive |
//! | state 4 | `hit` | reactions to hits, recoil, fall, grab by a player |
//!
//! Animations 0x15d4ae (movements) and 0x15d4ec (attacks); `+0xa2/+0xa3` current and previous look.

use crate::families::cx::Bench;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

mod arrival;
mod attacks;
mod hit;
mod moves;
mod stance;

pub(self) mod g {
    pub const GROUP: u32 = 0x2f;
    pub const STRIKE_KIND: u32 = 0x44;
    pub const STRIKE_POWER: u32 = 0x45;
    pub const VULNERABLE: u32 = 0x67;
    pub const AI_WAIT: u32 = 0x6e;
    pub const AI_CHARGE: u32 = 0x71;
    pub const AI_WANDER: u32 = 0x6b;
    pub const AI_LUNGE: u32 = 0x77;
    pub const AI_SHOT: u32 = 0x78;
    pub const SHOT_KIND: u32 = 0x79;
    /// Timer (word) of a gesture.
    pub const TIMER: u32 = 0xa0;
    pub const LOOK: u32 = 0xa2;
    pub const LOOK_PREV: u32 = 0xa3;
    pub const RESUME_SUB2: u32 = 0xa5;
    pub const RESUME_STATE: u32 = 0xa6;
    pub const TIMER_W: u32 = 0xaa;
    pub const REPEAT: u32 = 0xac;
    pub const GUARD: u32 = 0xad;
}
use g::*;
use crate::objects::pools;
use crate::objects::Current;

pub(self) const SET_MOVE: u32 = 0x15d4ae;
pub(self) const SET_STRIKE: u32 = 0x15d4ec;
pub(self) const STANCE: u32 = 0x0202_0000;
pub(self) const HELD: u32 = 0x0408_0000;
pub(self) const IDLE: u32 = 0x020a_0000;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        st::DEAD => x.free(&pools::ENEMY),
        _ => {}
    }
}

// --- aides --------------------------------------------------------------------------------------

pub(self) fn mv<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_MOVE, n as u16);
}

pub(self) fn strike<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_STRIKE, n as u16);
}

/// The wanted look: `+0xa2` and its twin `+0xa3`.
pub(self) fn look<R: Bench>(x: &mut Cx<'_, R>, v: u8) {
    x.set8(LOOK, v);
    x.set8(LOOK_PREV, v);
}

pub(self) fn set_speed<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
}

/// Walking direction: 8 (looks left) or 0x18.
pub(self) fn set_angle<R: Bench>(x: &mut Cx<'_, R>) {
    let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
    x.set8(Obj::SPEED_INDEX, angle);
}

/// Common step: waits for animation key `+0x33` (0x54fae).
pub(self) fn wait_key<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::KEY_FRAME) != 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 0);
    }
    x.anim_step();
}

/// Once the animation is over, the Kobold stands up, facing its target (0x54e9a).
pub(self) fn stand_when_done<R: Bench>(x: &mut Cx<'_, R>) {
    if !x.anim_step() {
        return;
    }
    set_state_long(x, STANCE);
    mv(x, 0);
}

/// A puff of dust (0xb10c8) every `mask + 1` frames (counter `A5+0x6e` is masked in place).
pub(self) fn puff<R: Bench>(x: &mut Cx<'_, R>, mask: u16, d1: u16, d2: i16, d3: i16) {
    let v = x.a5_u16(0x6e) & mask;
    x.set_a5_u16(0x6e, v);
    if v == 0 {
        lib::shock(x, 0, d1, d2 as u16, d3 as u16);
    }
}

/// 0x57b9c: the enemy must turn around to face its target (false without a target or a saved look).
pub(self) fn must_turn<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u8(LOOK_PREV) == 0 || x.u16(f::TARGET) == 0 {
        return false;
    }
    let target = x.linked(f::TARGET);
    let dx = target.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X)) as i16;
    let want = if dx > 0 { 0x20 } else { 0 };
    let turn = want != x.u8(f::FLIP);
    turn
}

/// 0x57bee: the target is more than 0x30 away in depth and more than 0xa0 in X.
pub(self) fn target_far_in_line<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u16(f::TARGET) == 0 {
        return false;
    }
    let target = x.linked(f::TARGET);
    let dz = target.u16(x.b, Obj::Y).wrapping_sub(x.u16(Obj::Y)).wrapping_add(0x30);
    if dz <= 0x60 {
        return false;
    }
    let dx = target.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X)).wrapping_add(0xa0);
    let far = dx > 0x140;
    far
}

/// 0x57c34: strength of the attack (`+0x45`) from the attack type `+0x44` and profile `+0x2f` (table 0x57c52).
pub(self) fn set_power<R: Bench>(x: &mut Cx<'_, R>) {
    let kind = x.u8(STRIKE_KIND).wrapping_mul(4) as u32;
    let group = x.u8(GROUP) as u32;
    let p = x.data8(0x57c52 + kind + group);
    x.set8(STRIKE_POWER, p);
}

/// 0x57c66: the enemy is in the right half of the screen (0xc0 from the camera).
pub(self) fn on_right_half<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let cam = if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA_ALT_X.get(x.b) } else { var::SCROLL_X.get(x.b) };
    let d1 = cam.wrapping_add(0xc0).wrapping_sub(x.u16(Obj::X));
    let right = (d1 as i16) <= 0;
    right
}

/// 0x56c70 / 0x56c78: chooses the next attack (`+6`) from the difficulty row of profile `+0x2f`; `bias` = 0x14 for the
/// far series.
pub(self) fn pick_attack<R: Bench>(x: &mut Cx<'_, R>, bias: u8) {
    let profile = x.u8(GROUP).wrapping_mul(4) as u32;
    let table = if bias == 0 { 0x56d04 } else { 0x56d14 };
    let list = x.data32(table + profile);
    let row = ((var::RANK.get(x.b) >> 1) << 5) as u32;
    let r = rand_and(x, 0x1f);
    let mv = x.data8(list + row + r);
    x.set8(f::SUB2, mv.wrapping_add(bias));
    x.set16(f::STATE, 0x0204);
    x.set8(f::SUB3, 0);
    if !x.in_arena(0) && matches!(x.u8(f::SUB2), 0x16 | 0x18 | 0x1a | 0x1c | 0x1e) {
        x.set8(f::SUB2, 0x14);
    }
}

/// 0x57924: end of a move: variant 0 gets back on its feet; the others, if the target is in a range box, chain an
/// attack of series `0x5799c`.
pub(self) fn after_strike<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Obj::VARIANT) == 0 || x.u16(f::TARGET) == 0 {
        return set_state_long(x, STANCE);
    }
    let close = lib::target_in_box(x, -0x30, 0x30, 8) || lib::target_in_box(x, -0xd0, 0x70, 8);
    if !close {
        return set_state_long(x, STANCE);
    }
    let row = ((var::RANK.get(x.b) >> 1) << 5) as u32;
    let r = rand_and(x, 0x1f);
    let mv = x.data8(0x5799c + row + r);
    x.set8(f::SUB2, mv);
    x.set16(f::STATE, 0x0208);
    x.set8(f::SUB3, 0);
}

// --- state 0: creation --------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    if matches!(var::SCENE.get(x.b), 6 | 0xc) {
        x.set8(0x2a, 1);
        x.set8(0x2b, 0x19);
    }
    x.add8(f::STATE, 2);
    x.set8(f::BOX_SET, 0xc);
    lib::initial_hp(x);
    for (off, v) in [(0x70, 0), (0x7e, 8), (0x7f, 0), (0x8e, 0), (0x78, 0xff)] {
        x.set8(off, v);
    }
    lib::set_immunities(x);
    // profils d'IA : 68 69 6a 6c 6d 72 73 74 75 76 81 80
    let params: [u8; 12] = match x.u8(GROUP) {
        0 => [1, 1, 1, 1, 1, 1, 1, 1, 0x14, 1, 0, 1],
        1 => [2, 2, 1, 2, 2, 2, 2, 2, 0x18, 2, 1, 2],
        2 => [2, 2, 1, 2, 3, 3, 3, 2, 0x1c, 3, 2, 3],
        _ => [2, 2, 1, 3, 4, 4, 4, 3, 0x20, 3, 3, 4],
    };
    for (off, v) in [0x68u32, 0x69, 0x6a, 0x6c, 0x6d, 0x72, 0x73, 0x74, 0x75, 0x76, 0x81, 0x80].into_iter().zip(params) {
        x.set8(off, v);
    }
    if !pick_target(x) {
        return set_state_long(x, IDLE);
    }
    lib::draw_stance(x);
    lib::draw_6b(x);
    // starting state, look and animation according to the variant
    let (state, look_v, anim, active) = match x.u8(Obj::VARIANT) {
        0 => (STANCE, 0xc, 0xc, false),
        1 => (0x0200_0e00, 0x16, 0x16, false),
        2 => (0x0200_0a00, 0x19, 0x19, false),
        3 => (0x0200_0400, 6, 6, false),
        4 => (0x0200_0600, 0, 0, true),
        5 => (0x0200_0800, 0, 0, true),
        6 => (0x0200_0000, 0x17, 0x17, false),
        7 => (0x0200_0c00, 0x1a, 0x1a, false),
        8 => (0x0200_0200, 0, 0, false),
        9 => (0x0200_1000, 0x17, 0x17, false),
        10 => (0x0200_1200, 0xc, 0xc, false),
        11 => (0x0200_1400, 0xc, 0xc, false),
        12 => (0x0200_0600, 0, 0, true),
        13 => (0x0200_0800, 0, 0, true),
        _ => return,
    };
    set_state_long(x, state);
    if active {
        x.set8(0, 2);
    }
    look(x, look_v);
    x.set8(VULNERABLE, 1);
    mv(x, anim);
}

// --- state 2: active -----------------------------------------------------------------------------

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        if freeze < 0 {
            x.set8(f::FREEZE, 0);
        }
        if !paused(x) {
            match x.u8(f::SUB) {
                0 => arrival::run(x),
                2 => stance::run(x),
                4 => attacks::run(x),
                6 => moves::run_b(x),
                8 => moves::run_c(x),
                10 => moves::idle(x),
                _ => {}
            }
        }
    }
    if x.u8(f::SUB) != 0 && x.u8(f::SUB2) != 0xe {
        zone_and_terrain(x);
    }
    x.reinsert();
}

handlers! {
    0x54914 "enemy_0" run,
}
