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
use crate::objects::Current;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

mod attacks;
mod control;
mod hit;
mod moves;

pub(self) mod g {
    pub const GROUP: u32 = 0x2f;
    /// Time (word) of the current animation.
    pub const ANIM_TIME: u32 = 0x30;
    /// End flag (word) of the current animation.
    pub const ANIM_END: u32 = 0x32;
    pub const STRIKE_KIND: u32 = 0x44;
    pub const STRIKE_POWER: u32 = 0x45;
    pub const AI_WAIT: u32 = 0x77;
    pub const AI_SHOT: u32 = 0x78;
    pub const HIT_FLAG: u32 = 0x8f;
    pub const TIMER: u32 = 0xa2;
    pub const TIMER2: u32 = 0xa4;
    /// Orientation of the target as seen by the boxes (0, 2) or box number.
    pub const SIDE: u32 = 0xa6;
}
use g::*;
use crate::objects::pools;

pub(self) const SET_MOVE: u32 = 0x15eed8;
pub(self) const SET_STRIKE: u32 = 0x15eef4;
pub(self) const STANCE: u32 = 0x0202_0000;

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
pub(self) fn mv<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_MOVE, n as u16);
}

/// Starts animation `n` of the attack set.
pub(self) fn strike<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_STRIKE, n as u16);
}

/// Sets the attack type and its strength.
pub(self) fn set_strike<R: Bench>(x: &mut Cx<'_, R>, kind: u8, power: u8) {
    x.set8(STRIKE_KIND, kind);
    x.set8(STRIKE_POWER, power);
}

/// 0x6935c: next step, animation 0.
pub(self) fn next_anim<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    mv(x, 0);
}

/// 0x69346: waits `+0x77` frames (countdown), then next step.
pub(self) fn wait<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.u8(AI_WAIT);
    if n != 0 {
        let m = n.wrapping_sub(1);
        x.set8(AI_WAIT, m);
        if (m as i8) > 0 {
            return;
        }
    }
    x.add8(f::SUB3, 2);
}

/// 0x6899a: turn-around according to `+0xa6` (0 or 1: nothing, 2: animation 1 then 3: waits for the end and turns
/// around).
pub(self) fn turn<R: Bench>(x: &mut Cx<'_, R>) {
    let side = x.u8(SIDE) as i8;
    if side < 2 {
        return x.add8(f::SUB3, 2);
    }
    if side == 2 {
        x.add8(SIDE, 1);
        return mv(x, 1);
    }
    if (x.u8(ANIM_END) as i8) < 0 {
        x.add8(f::SUB3, 2);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        return mv(x, 0);
    }
    x.anim_step();
}

/// 0x689e6: saves in `+0xa6` whether the target is behind (2) or in front (0); true if behind.
pub(self) fn target_behind<R: Bench>(x: &mut Cx<'_, R>) -> bool {
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
    x.set8(SIDE, d1 as u8);
    d1 != 0
}

/// 0x68910: received hit flag `+0x8f`: returns true when the state must continue normally.
pub(self) fn hit_gate<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let v = x.u8(HIT_FLAG);
    if v == 0 {
        return true;
    }
    match v {
        1 => {
            if x.u16(Obj::Z) != 0 {
                return true;
            }
            x.set8(HIT_FLAG, 0);
            x.set8(0x49, 0x58);
            if x.u32(f::STATE) == 0x0404_0200 {
                x.set16(ANIM_END, 0xffff);
                set_state_long(x, 0x0406_0200);
            } else {
                set_state_long(x, 0x0400_0000);
            }
            false
        }
        2 => {
            set_state_long(x, 0x0400_0000);
            x.set8(HIT_FLAG, 0);
            let code = if x.u16(Obj::Z) == 0 { 0xc } else { 0x38 };
            x.set8(0x49, code);
            false
        }
        3 => {
            if x.u16(Obj::Z) != 0 {
                return true;
            }
            x.set8(HIT_FLAG, 0);
            set_state_long(x, 0x0206_0a00);
            false
        }
        _ => false,
    }
}

/// Byte drawn at random from the difficulty row of `table`.
pub(self) fn row_byte<R: Bench>(x: &mut Cx<'_, R>, table: u32) -> u8 {
    let row = ((var::RANK.get(x.b) >> 1) << 5) as u32;
    let at = table.wrapping_add(row as u16 as i16 as i32 as u32);
    let r = rand_and(x, 0x1f);
    x.data8(at + r)
}

/// 0x690ce: chooses the next move (`+5` = 8) according to the distance of the target, or gets back on its feet.
pub(self) fn choose<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(AI_SHOT, 0xff);
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
    x.set8(SIDE, side as u8);
    let b = row_byte(x, 0x69146);
    x.set8(f::SUB2, b);
    x.set16(f::STATE, 0x0208);
    x.set8(f::SUB3, 0);
}

/// 0x67ea4: restarts animation 2 taking over the time and the end flag of the previous animation (at most eight frames
/// ahead), marks the strike and saves the position.
pub(self) fn restart_walk<R: Bench>(x: &mut Cx<'_, R>) {
    let time = x.u16(ANIM_TIME);
    let end = x.u16(ANIM_END);
    mv(x, 2);
    let mut keep = time;
    let mut found = false;
    for _ in 0..8 {
        if x.u16(ANIM_END) == end {
            found = true;
            break;
        }
        x.set16(ANIM_END, 1);
        x.anim_step();
    }
    if !found {
        keep = x.u16(ANIM_TIME);
    }
    x.set16(ANIM_TIME, keep);
    x.set32(0x4c, 0x1c7336);
    x.set8(Obj::SPEED_INDEX, 8);
    if x.u8(f::FLIP) != 0 {
        x.add8(Obj::SPEED_INDEX, 0x10);
    }
    let at = x.u16(Obj::X);
    x.set16(TIMER2, at);
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
    0x67854 "enemy_11" run,
}
