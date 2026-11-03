//! "Mud Golem" enemy (ENEMY type 11, 0x67854). See `docs/families/enemy.md`.
//!
//! A slow colossus whose code is a heap of moves: punches (animations 0x15eef4), jumps, lateral leaps, walk. When it
//! stands (`+5` = 0 or 2), it is driven by the keys of the fourth player (`A5+3`, debug mode); the AI proper starts
//! from substates 4 (attacks), 6 and 8 (fallback), chosen by routine 0x690ce from rows of 32 bytes (0x69146).
//!
//! | `+5` | File | Role (h) |
//! |---|---|---|
//! | 0, 2 | `control` | key-driven walk, jump |
//! | 4 | `attacks` | attacks (`+6` = 0 to 20) and their steps `+7` |
//! | 6, 8 | `moves` | leaps, dodges, screen exit |
//! | state 4 | `hit` | reactions to hits, fall, flight |
//!
//! Animations 0x15eed8 (movements) and 0x15eef4 (attacks). Many moves share three common steps: `next_anim` (0x6935c),
//! `wait` (0x69346, wait of `+0x77` frames) and `turn` (0x6899a, turn-around).

use crate::families::cx::Bench;
use crate::record::record;
use crate::data::RomAddr;
use crate::objects::Current;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

mod attacks;
mod control;
mod hit;
mod moves;

/// Fields of the family beyond the common ones of `Enemy` (`record!`). A meaning guessed from the code alone is
/// marked (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Golem(pub Obj);

impl Golem {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Golem {
        hit_flag, set_hit_flag, HIT_FLAG: u8 @ 0x8f;
        timer, set_timer, TIMER: u8 @ 0xa2;
        timer2, set_timer2, TIMER2: u16 @ 0xa4;
        /// Orientation of the target as seen by the boxes (0, 2) or box number.
        side, set_side, SIDE: u8 @ 0xa6;
        /// Set when a player grabs the golem; not read by the family (h).
        grabbed, set_grabbed, GRABBED: u8 @ 0xa7;
        /// Set before a squat (0x100, or 0xff with `SQUAT_SIDE`); not read by the family (h).
        squat_mode, set_squat_mode, SQUAT_MODE: u16 @ 0xa8;
        /// Side drawn before a squat (0 or 2), 1 at the end of a freeze; not read by the family (h).
        squat_side, set_squat_side, SQUAT_SIDE: u8 @ 0xa9;
    }
}
use crate::objects::pools;

const SET_MOVE: RomAddr = RomAddr(0x15eed8);
const SET_STRIKE: RomAddr = RomAddr(0x15eef4);
const STANCE: u32 = 0x0202_0000;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        st::DEAD => x.free(&pools::ENEMY),
        _ => {}
    }
}

// --- aides --------------------------------------------------------------------------------------

/// Starts animation `n` of the movement set.
fn mv<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_MOVE.0, n as u16);
}

/// Starts animation `n` of the attack set.
fn strike<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_STRIKE.0, n as u16);
}

/// Sets the attack type and its strength.
fn set_strike<R: Bench>(x: &mut Cx<'_, R>, kind: u8, power: u8) {
    x.set8(Enemy::STRIKE_KIND, kind);
    x.set8(Enemy::STRIKE_POWER, power);
}

/// 0x6935c: next step, animation 0.
fn next_anim<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    mv(x, 0);
}

/// 0x69346: waits `+0x77` frames (countdown), then next step.
fn wait<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.u8(Enemy::AI_BYTE);
    if n != 0 {
        let m = n.wrapping_sub(1);
        x.set8(Enemy::AI_BYTE, m);
        if (m as i8) > 0 {
            return;
        }
    }
    x.add8(f::SUB3, 2);
}

/// 0x6899a: turn-around according to `+0xa6` (0 or 1: nothing, 2: animation 1 then 3: waits for the end and turns
/// around).
fn turn<R: Bench>(x: &mut Cx<'_, R>) {
    let side = x.u8(Golem::SIDE) as i8;
    if side < 2 {
        return x.add8(f::SUB3, 2);
    }
    if side == 2 {
        x.add8(Golem::SIDE, 1);
        return mv(x, 1);
    }
    if (x.u8(Enemy::ANIM_END) as i8) < 0 {
        x.add8(f::SUB3, 2);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        return mv(x, 0);
    }
    x.anim_step();
}

/// 0x689e6: saves in `+0xa6` whether the target is behind (2) or in front (0); true if behind.
fn target_behind<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let mut d1 = 0u32;
    if x.u16(f::TARGET) != 0 {
        let target = load_target(x);
        let mine = x.u16(Obj::X);
        let theirs = target.u16(x.b, Obj::X);
        if mine != theirs {
            let want = if mine < theirs { 0x20 } else { 0 };
            if want as u8 != x.u8(f::FLIP) {
                d1 = 2;
            }
        }
    }
    x.set8(Golem::SIDE, d1 as u8);
    d1 != 0
}

/// 0x68910: received hit flag `+0x8f`: returns true when the state must continue normally.
fn hit_gate<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let v = x.u8(Golem::HIT_FLAG);
    if v == 0 {
        return true;
    }
    match v {
        1 => {
            if x.u16(Obj::Z) != 0 {
                return true;
            }
            x.set8(Golem::HIT_FLAG, 0);
            x.set8(Enemy::REACTION, 0x58);
            if x.u32(f::STATE) == 0x0404_0200 {
                x.set16(Enemy::ANIM_END, 0xffff);
                set_state_long(x, 0x0406_0200);
            } else {
                set_state_long(x, 0x0400_0000);
            }
            false
        }
        2 => {
            set_state_long(x, 0x0400_0000);
            x.set8(Golem::HIT_FLAG, 0);
            let code = if x.u16(Obj::Z) == 0 { 0xc } else { 0x38 };
            x.set8(Enemy::REACTION, code);
            false
        }
        3 => {
            if x.u16(Obj::Z) != 0 {
                return true;
            }
            x.set8(Golem::HIT_FLAG, 0);
            set_state_long(x, 0x0206_0a00);
            false
        }
        _ => false,
    }
}

/// 0x690ce: chooses the next move (`+5` = 8) according to the distance of the target, or gets back on its feet.
fn choose<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::AI_SHOT, 0xff);
    if x.u16(f::TARGET) == 0 {
        return set_state_long(x, STANCE);
    }
    load_target(x);
    let side = match lib::target_in_front(x, -0x8, 0x68, 8) {
        lib::InBox::No => lib::target_in_front(x, -0xa0, 0x30, 8),
        near => near,
    };
    if !side.found() {
        return set_state_long(x, STANCE);
    }
    x.set8(Golem::SIDE, side as u8);
    let b = rank_byte(x, "golem/next_moves");
    x.set8(f::SUB2, b);
    x.set16(f::STATE, 0x0208);
    x.set8(f::SUB3, 0);
}

/// 0x67ea4: restarts animation 2 taking over the time and the end flag of the previous animation (at most eight frames
/// ahead), marks the strike and saves the position.
fn restart_walk<R: Bench>(x: &mut Cx<'_, R>) {
    let time = x.u16(Obj::ANIM_TIMER);
    let end = x.u16(Enemy::ANIM_END);
    mv(x, 2);
    let mut keep = time;
    let mut found = false;
    for _ in 0..8 {
        if x.u16(Enemy::ANIM_END) == end {
            found = true;
            break;
        }
        x.set16(Enemy::ANIM_END, 1);
        x.anim_step();
    }
    if !found {
        keep = x.u16(Obj::ANIM_TIMER);
    }
    x.set16(Obj::ANIM_TIMER, keep);
    x.set32(Obj::SPEED_TABLE, 0x1c7336);
    x.set8(Obj::SPEED_INDEX, 8);
    if x.u8(f::FLIP) != 0 {
        x.add8(Obj::SPEED_INDEX, 0x10);
    }
    let at = x.u16(Obj::X);
    x.set16(Golem::TIMER2, at);
}

// --- state 0: creation --------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    lib::initial_hp(x);
    let (bx, by) = if var::CAMERA_SELECT.get(x.b) != 0 { (0x556, 0x55a) } else { (0x506, 0x50a) };
    let cam_x = x.a5_u16(bx);
    let cam_y = x.a5_u16(by);
    x.set16(Obj::X, cam_x.wrapping_add(0xc0));
    x.set32(Obj::Z, 0);
    x.set16(Obj::Y, cam_y.wrapping_add(0x50));
    x.set8(f::FLIP, 0);
    set_state_long(x, STANCE);
    mv(x, 0);
    x.reinsert();
}

// --- state 2: active -----------------------------------------------------------------------------

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else if !paused(x) {
        match x.u8(f::SUB) {
            0 | 2 => control::run(x),
            4 => attacks::run(x),
            6 => moves::run_c(x),
            8 => moves::run_d(x),
            _ => {}
        }
    }
    zone_and_terrain(x);
    x.reinsert();
}

handlers! {
    ENEMY_11 0x67854 "enemy_11" run,
}
