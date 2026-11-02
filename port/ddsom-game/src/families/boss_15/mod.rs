//! Boss of types 15 and 20 (0x91530, a single routine for both), stage 46. See `docs/families/boss.md`.
//!
//! Red dragon (type 15 "Red Dragon" in the debug names; type 20 is a smaller variant never seen in the scenarios):
//! from the same family as the dragon of `boss_19` (same target draws, same range probes) with its own tables, a
//! three-phase flight scenery and an "armor" (the first hits of each 64-frame period do not interrupt it). Enemy
//! skeleton (`+4` state, `+5` group, `+6` and `+7` steps), states set as longs.
//!
//! | `+5` | File | Role |
//! |---|---|---|
//! | 0 | `intro` | arrival: falls from the sky on the right in three phases |
//! | 2 | `fight` | combat: walk (`+6` = 0), jump toward a player (2), approach (4) |
//! | 4 | `attacks` | eleven attacks (`+6` = 0 to 18), most of them in flight |
//! | 6, 8, 10 | here | standing, then waiting while no player is alive |
//! | 12 | here | grab of a player (drained down to 1 HP) |
//! | 14 | here | wait |
//! | state 4 | `hit` | reaction (a single routine), knockback, getting up, death |
//!
//! Animations: three sets per type (`+0xa0` = 0, 2 or 4, `set_anim`), `+0xa1` is the current animation.

use crate::families::cx::Bench;
use super::boss_common::*;
use super::cx::handlers;

pub(crate) mod attacks;
mod fight;
mod hit;
mod intro;

mod g {
    pub const TYPE: u32 = 0x2;
    pub const STUN: u32 = 0x56;
    pub const VULNERABLE: u32 = 0x67;
    pub const WALK_STEPS: u32 = 0x6b;
    pub const WALK_DELAY: u32 = 0x71;
    pub const SHOT: u32 = 0x79;
    /// Foreground: the foreground display list (`+0x43`).
    pub const FOREGROUND: u32 = 0x43;
    pub const SHAKE: u32 = 0x90;
    pub const ANIM_SET: u32 = 0xa0;
    pub const ANIM_N: u32 = 0xa1;
    pub const HURT_BOX: u32 = 0xa2;
    pub const HOVER: u32 = 0xa4;
    pub const HOVER_STATE: u32 = 0xa5;
    pub const TIMER: u32 = 0xa6;
    /// Position saved at the end of the previous frame (X, altitude, depth): a hit does not move the dragon.
    pub const KEPT_X: u32 = 0xa8;
    pub const KEPT_Z: u32 = 0xaa;
    pub const KEPT_Y: u32 = 0xac;
    pub const SHIELD: u32 = 0xae;
    pub const SPECIALS: u32 = 0xaf;
    /// Active state of the previous frame (long) and its hit box (`+0xb9`).
    pub const LAST_STATE: u32 = 0xb0;
    pub const LAST_BOX: u32 = 0xb9;
    /// Armor: period (`+0xb4`, 64 frames), hits still ignored (`+0xb5`), push timer (`+0xb8`).
    pub const ARMOR_TICK: u32 = 0xb4;
    pub const ARMOR: u32 = 0xb5;
    pub const COOLDOWN: u32 = 0xb7;
    pub const PUSHED: u32 = 0xb8;
}
use g::*;
use crate::objects::pools;
use crate::objects::Current;

const TYPE_BIG: u8 = 0xf;
const TYPE_SMALL: u8 = 0x14;

const STAND: u32 = 0x0202_0000;
const TURN: u32 = 0x0202_0200;
const CHASE: u32 = 0x0202_0400;
const WAIT: u32 = 0x020a_0000;
const ATTACK_4: u32 = 0x0204_0400;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        _ => x.free(&pools::BOSS),
    }
}

// --- animations ----------------------------------------------------------------------------------

/// Starts animation `n` of set `set` (0, 2 or 4) of the type, unless it is already playing (0x93182, 0x9319c,
/// 0x931b6).
fn set_anim<R: Bench>(x: &mut Cx<'_, R>, set: u8, n: u32) {
    let small = x.u8(TYPE) == TYPE_SMALL;
    let base = match (set, small) {
        (0, false) => 0x17e03a,
        (0, true) => 0x18295e,
        (2, false) => 0x17e048,
        (2, true) => 0x18296c,
        (_, false) => 0x17e096,
        (_, true) => 0x1829ba,
    };
    if set == x.u8(ANIM_SET) && n as u8 == x.u8(ANIM_N) {
        return;
    }
    x.set8(ANIM_SET, set);
    x.set8(ANIM_N, n as u8);
    x.anim_start(base, n as u16);
}

fn rest<R: Bench>(x: &mut Cx<'_, R>) {
    set_anim(x, 0, 0);
}

fn step<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.anim_step()
}

// --- state changes ---------------------------------------------------------------------------

/// 0x92ff4: standing, at rest.
fn stand<R: Bench>(x: &mut Cx<'_, R>) {
    set_abs8(x, 0xff8043, 0xff);
    x.set8(HURT_BOX, 0);
    x.set32(f::STATE, STAND);
    rest(x);
}

/// 0x9300c: walks towards a player (only through sub-state 0xa of state 4, never set: docs/code-mort.md).
fn chase<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(HURT_BOX, 0);
    x.set32(f::STATE, CHASE);
    rest(x);
}

/// 0x9301e: rest or walk, at random (the small dragon: only the about-face).
fn wander<R: Bench>(x: &mut Cx<'_, R>) {
    let mut r = (x.random() as u32) as u16 & 3;
    if x.u8(TYPE) != TYPE_BIG {
        r &= 1;
    }
    x.set8(HURT_BOX, 0);
    if r as u8 == 0 {
        x.set32(f::STATE, TURN);
    } else {
        x.set16(TIMER, 0x100);
        x.set32(f::STATE, CHASE);
    }
    rest(x);
}

/// 0x91748: waits for a player.
fn wait<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, WAIT);
}

// --- state 0: creation --------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PAUSE.get(x.b) != 0 {
        return;
    }
    if !super::small::enemy_pair_created(x.b, x.o) {
        return;
    }
    x.add8(f::STATE, 2);
    x.set8(f::SUB, 0);
    x.set16(f::SUB2, 0);
    x.set_a5_u32(0x38, 0x0459_5900);
    x.set_a5_u8(0x50, 1);
    x.set8(0x93, 1);
    x.set8(0x9a, 0xff);
    let big = x.u8(TYPE) == TYPE_BIG;
    x.set8(0x7e, if big { 0x50 } else { 0x29 });
    lib::initial_hp_boss(x);
    lib::create_set_14(x);
    lib::create_set_15(x);
    x.set8(0x78, 0xff);
    x.set8(f::BOX_SET, 0x2f);
    x.set8(0x82, 1);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(0, 2);
    for (off, v) in [(0x76, 5), (0x8e, 3), (0x6f, 2), (0x68, 5), (0x69, 5), (0x6c, 4), (0x6d, 8), (0x6a, 3)] {
        x.set8(off, v);
    }
    var::SCENE_LEVEL.set(x.b, 0);
    lib::draw_6b(x);
    x.set8(0x70, 0);
    lib::draw_stance(x);
    x.set8(0x81, 0);
    x.set8(0x80, if big { 0x28 } else { 0x2c });
    x.set8(FOREGROUND, 0);
    x.set8(0xb6, 0);
    x.set8(SHIELD, 0);
    x.set8(HURT_BOX, 0);
    x.set8(SPECIALS, if big { 2 } else { 3 });
    let (px, pz, py) = (x.u16(0x8), x.u16(0xc), x.u16(0x10));
    x.set16(KEPT_X, px);
    x.set16(KEPT_Z, pz);
    x.set16(KEPT_Y, py);
    x.set16(ANIM_SET, 0xffff);
    rest(x);
    lib::empty_popups(x);
    lib::clear_popup(x);
    lib::clear_all_popups(x);
}

// --- state 2: active -----------------------------------------------------------------------------

/// Active state loop (0x91684); the hit state calls it again to resume the combat.
fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(FOREGROUND) != 0 {
        x.set8(FOREGROUND, 0);
        x.unlink();
    }
    if var::PAUSE.get(x.b) == 0 {
        if tst8(x, f::FREEZE).gt() {
            sub8(x, f::FREEZE, 1);
        } else {
            if tst8(x, PUSHED).gt() && sub8(x, PUSHED, 1).le() {
                x.set32(0x58, 0);
            }
            x.set8(0x42, 0);
            if sub8(x, ARMOR_TICK, 1).le() {
                x.set8(ARMOR_TICK, 0x40);
                x.set8(ARMOR, 4);
            }
            restore_position(x);
            match x.u8(f::SUB) {
                0 => intro::run(x),
                2 => fight::run(x),
                4 => attacks::run(x),
                6 | 8 => {
                    stand(x);
                    idle(x);
                }
                10 => idle(x),
                12 => grab(x),
                _ => {}
            }
            let s = x.u32(f::STATE);
            x.set32(LAST_STATE, s);
            let b = x.u8(HURT_BOX);
            x.set8(LAST_BOX, b);
            keep_position(x);
            hover(x);
            if tst8(x, SHIELD).gt() {
                sub8(x, SHIELD, 1);
                x.set8(0x38, 0);
            }
        }
    }
    finish(x);
}

/// 0x9171e: end of the frame: the foreground receives the dragon when `+0x43` is raised.
fn finish<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0xb6, 0);
    if x.u8(FOREGROUND) != 0 {
        x.unlink();
        x.reinsert_foreground();
    } else {
        x.reinsert();
    }
}

/// The saved positions take over the dragon's place at the start of the frame (a hit does not move it).
fn restore_position<R: Bench>(x: &mut Cx<'_, R>) {
    let (px, pz, py) = (x.u16(KEPT_X), x.u16(KEPT_Z), x.u16(KEPT_Y));
    x.set16(0x8, px);
    x.set16(0xc, pz);
    x.set16(0x10, py);
}

fn keep_position<R: Bench>(x: &mut Cx<'_, R>) {
    let (px, pz, py) = (x.u16(0x8), x.u16(0xc), x.u16(0x10));
    x.set16(KEPT_X, px);
    x.set16(KEPT_Z, pz);
    x.set16(KEPT_Y, py);
}

/// 0x93066: vertical sway, when `+0xa4` is raised: the altitude follows a sine table (0x1c67b6).
fn hover<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB) == 0 {
        return;
    }
    if x.u8(HOVER) == 0 {
        x.set8(HOVER_STATE, 0);
        return;
    }
    x.set8(HOVER, 0);
    let phase = x.u8(HOVER_STATE).wrapping_add(1);
    x.set8(HOVER_STATE, phase);
    let i = (phase as u32) & 0x7c;
    let w = x.data16(0x1c67b6 + 2 + i);
    x.add32(0xc, (((w as u32) << 16) as i32 >> 8) as u32);
}

/// `+5` = 6, 8, 10 (0x927fa): waits for a player to come back.
fn idle<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PLAYERS_ALIVE.get(x.b) != 0 {
        x.set32(f::STATE, STAND);
        return;
    }
    x.set8(HOVER, 1);
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        return rest(x);
    }
    step(x);
    fight::steer(x);
}

/// `+5` = 12 (0x9282c): grab of a player: they lose HP then are released.
fn grab<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u16(f::TARGET) as i16) >= 0 {
        return stand(x);
    }
    let p = x.linked(f::TARGET);
    if p.u8(x.b, 0) == 0 || p.u8(x.b, 0x188) == 0 {
        return stand(x);
    }
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(TIMER, 0x20);
            let hp = p.u16(x.b, f::HP).wrapping_sub(0x10);
            p.set_u16(x.b, f::HP, hp);
            if (hp as i16) <= 0 {
                p.set_u16(x.b, f::HP, 1);
            }
            set_anim(x, 2, 4);
            lib::hold_seized(x);
        }
        2 => {
            if (x.u8(TIMER) as i8) > 0 {
                if !sub8(x, TIMER, 1).gt() {
                    x.add8(0x7, 2);
                    x.set8(TIMER, 0x10);
                }
                if (x.u16(0xc) as i16) > 0 {
                    x.set16(0xc, 0);
                }
            }
            grab_step(x);
        }
        4 => grab_step(x),
        _ => {}
    }
}

fn grab_step<R: Bench>(x: &mut Cx<'_, R>) {
    if !step_anim(x) {
        return lib::hold_seized(x);
    }
    x.set16(ANIM_SET, 0xffff);
    set_anim(x, 2, 4);
    if !sub8(x, TIMER, 1).gt() {
        return release(x);
    }
    if (x.u16(f::TARGET) as i16) >= 0 {
        return stand(x);
    }
    let p = x.linked(f::TARGET);
    if (p.u16(x.b, f::HP) as i16) <= 1 {
        return release(x);
    }
    let hp = p.u16(x.b, f::HP).wrapping_sub(1);
    p.set_u16(x.b, f::HP, hp);
    lib::sound(x, crate::sound::CMD_341_441);
    lib::hold_seized(x);
}

fn release<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x45, 1);
    let player = x.linked(f::TARGET);
    lib::release_player(x, player);
    set_anim(x, 2, 5);
    stand(x);
}

/// `jsr 0x1a56`: one script step, returns true at the end (carry raised).
fn step_anim<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.anim_step()
}

handlers! {
    0x91530 "boss_15" run,
}
