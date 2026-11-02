//! "WarMachine" enemy (ENEMY type 27, 0x77d12). See `docs/families/enemy.md`.
//!
//! Rolling siege machine: it rushes at the player (substate 2, `+6` = 8), fires projectiles (substate 4, moves `+6` = 0
//! to 12), jumps, and its hit points make it go through thresholds (`lib::hp_threshold`). Like the Gargoyle and the
//! Ogre it relies on the library `enemy_lib` (0x103d6-0x105b6). The AI decisions go through small routines that read
//! byte tables then word tables (relative jump address) and end on one of the state "stubs" (`go::*`, 0x791a0-0x792d6).
//!
//! | `+5` | File | Role (h) |
//! |---|---|---|
//! | 0 | `arrival` | arrival: rolls in from the edge (`+6` = 0), waits for a player (2 and 4) |
//! | 2 | `stance` | rolls toward the player, chooses a shot (`decide`), turn-around, charge |
//! | 4 | `attacks` | shots and jumps (`+6` = 0 to 12) |
//! | 6, 8, 10 | `attacks` | spins in place as long as no player is alive |
//! | state 4 | `hit` | reactions, death in five stages, flight |

use crate::families::cx::Bench;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use super::enemy_lib as lib;

mod arrival;
mod attacks;
mod decide;
pub(crate) mod hit;
mod stance;

pub(self) mod g {
    pub const GROUP: u32 = 0x2f;
    pub const VULNERABLE: u32 = 0x67;
    pub const AI_WANDER: u32 = 0x6b;
    pub const AI_WAIT: u32 = 0x6e;
    pub const AI_CHARGE: u32 = 0x71;
    pub const AI_SHOT: u32 = 0x78;
    pub const SHOT_KIND: u32 = 0x79;
    /// Current and previous look.
    pub const LOOK: u32 = 0xa0;
    pub const LOOK_PREV: u32 = 0xa1;
    /// State to resume after a turn-around (long).
    pub const RESUME_STATE: u32 = 0xa2;
    pub const TIMER: u32 = 0xa6;
    pub const TIMER2: u32 = 0xa8;
    pub const PHASE: u32 = 0xaa;
    pub const FLAG: u32 = 0xad;
    pub const COUNT: u32 = 0xae;
    /// Non-zero: the charge aims at position `GOAL_X` instead of the edge of the screen.
    pub const AIMED: u32 = 0xb2;
    pub const GOAL_X: u32 = 0xbc;
}
use g::*;
use crate::objects::pools;
use crate::objects::Current;

pub(self) const SET_MOVE: u32 = 0x171a52;
pub(self) const SET_WEAPON: u32 = 0x171a7e;
pub(self) const SET_FLY_A: u32 = 0x171b76;
pub(self) const SET_FLY_B: u32 = 0x171c1a;

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
    if x.u8(FLAG) != 0 {
        x.set8(0x38, 0);
    }
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

// --- aides --------------------------------------------------------------------------------------

/// Starts animation `n` of the movement set.
pub(self) fn mv<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_MOVE, n);
}

/// Starts animation `n` of the weapon set.
pub(self) fn weapon<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_WEAPON, n);
}

/// Look (`+0xa0`) and its twin (`+0xa1`).
pub(self) fn look<R: Bench>(x: &mut Cx<'_, R>, v: u8) {
    x.set8(LOOK, v);
    x.set8(LOOK_PREV, v);
}

pub(self) fn set_speed<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
}

/// Spark (0x1172e) number `n`.
pub(self) fn spark<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::debris(x, n);
}

/// Camera X (single screen or second screen).
pub(self) fn camera_x<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    if var::CAMERA_SELECT.get(x.b) != 0 {
        var::CAMERA2_X.get(x.b)
    } else {
        var::CAMERA1_X.get(x.b)
    }
}

/// Orientation: `forward` gives 0x18 when the orientation is zero, otherwise 8.
pub(self) fn heading<R: Bench>(x: &mut Cx<'_, R>) -> u8 {
    if x.u8(f::FLIP) != 0 {
        8
    } else {
        0x18
    }
}

/// The state "stubs": small routines that set the next state (0x791a0-0x792d6); the AI decisions end up there by jump.
pub(self) mod go {
    use super::*;

    pub fn arrive<R: Bench>(x: &mut Cx<'_, R>, s6: u32) {
        x.set8(FLAG, 1);
        set_state_long(x, 0x0200_0000 | s6 << 8);
    }
    /// 0x791d0.
    pub fn stand<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(VULNERABLE, 1);
        set_state_long(x, 0x0202_0000);
    }
    /// 0x791e0: turn-around (state to resume saved).
    pub fn turn<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(VULNERABLE, 0);
        let state = x.u32(f::STATE);
        x.set32(RESUME_STATE, state);
        set_state_long(x, 0x0202_0200);
    }
    /// 0x791f4: shot A (shot type 0x34, or 0x36 depending on the profile).
    pub fn fire_a<R: Bench>(x: &mut Cx<'_, R>) {
        let kind = if x.u8(GROUP) != 0 { 0x36 } else { 0x34 };
        x.set8(SHOT_KIND, kind);
        x.set8(VULNERABLE, 1);
        set_state_long(x, 0x0202_0400);
    }
    /// 0x79216: shot B (0x35 or 0x37).
    pub fn fire_b<R: Bench>(x: &mut Cx<'_, R>) {
        let kind = if x.u8(GROUP) != 0 { 0x37 } else { 0x35 };
        x.set8(SHOT_KIND, kind);
        x.set8(VULNERABLE, 1);
        set_state_long(x, 0x0202_0600);
    }
    /// 0x79238 (`aimed` = 0) and 0x7924a (`aimed` = 1): charge.
    pub fn charge<R: Bench>(x: &mut Cx<'_, R>, aimed: u8) {
        x.set8(VULNERABLE, 0);
        x.set8(AIMED, aimed);
        set_state_long(x, 0x0202_0800);
    }
    /// 0x7925e to 0x792be: move of substate 4 (`s6` = 0, 2, 4, 6, 8, 0xa, 0xc); `flag`: sets `+0xad` (stubs 0x79288 and
    /// 0x792aa).
    pub fn attack<R: Bench>(x: &mut Cx<'_, R>, s6: u32, flag: bool) {
        if flag {
            x.set8(FLAG, 1);
        }
        x.set8(VULNERABLE, 0);
        set_state_long(x, 0x0204_0000 | s6 << 8);
    }
    /// 0x792cc.
    pub fn idle<R: Bench>(x: &mut Cx<'_, R>) {
        set_state_long(x, 0x020a_0000);
    }
    /// 0x792d6.
    pub fn held<R: Bench>(x: &mut Cx<'_, R>) {
        set_state_long(x, 0x0408_0000);
    }
}

// --- state 0: creation --------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    x.set8(0xab, 2);
    x.set8(f::BOX_SET, 0x20);
    lib::initial_hp(x);
    lib::spread_index(x);
    x.set8(0, 1);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set_bit(1, 1, true);
    for (off, v) in [
        (0x76, 0),
        (0x70, 0),
        (0x6a, 2),
        (0x75, 0),
        (0x72, 1),
        (0x73, 1),
        (0x6c, 6),
        (0x6d, 1),
        (0x74, 7),
        (AI_SHOT, 0xff),
        (0x8e, 3),
        (0x68, 0xb),
        (0x69, 1),
        (VULNERABLE, 0),
        (LOOK, 0),
        (LOOK_PREV, 0xff),
        (0x7e, 0xc),
        (0x7f, 0),
        (0x80, 0x1e),
        (0x81, 0x19),
    ] {
        x.set8(off, v);
    }
    pick_target(x);
    mv(x, 4);
    // the starting state depends on the variant: 0, 2 or 4 for `+6`
    match x.u8(3) {
        0 => go::arrive(x, 0),
        1 => go::arrive(x, 2),
        2 => go::arrive(x, 4),
        _ => {}
    }
}

// --- state 2: active -----------------------------------------------------------------------------

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u8(f::FREEZE) as i8) > 0 {
        let n = x.u8(f::FREEZE).wrapping_sub(1);
        x.set8(f::FREEZE, n);
    } else if !paused(x) {
        match x.u8(f::SUB) {
            0 => arrival::run(x),
            2 => stance::run(x),
            4 => attacks::run(x),
            6 | 8 | 10 => attacks::spin(x),
            _ => {}
        }
        if x.u8(f::SUB) == 0 {
            return;
        }
    }
    zone_and_terrain(x);
}

handlers! {
    0x77d12 "enemy_27" #[without] run,
    // get up after a common reaction (table 0x3e000 of `reactions.rs`)
    0x78c3c "enemy_27_releve" hit::recover,
}
