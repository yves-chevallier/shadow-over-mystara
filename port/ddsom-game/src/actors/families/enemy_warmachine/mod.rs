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

use crate::actors::families::cx::Bench;
use crate::world::objects::Obj;
use crate::world::record::record;
use crate::world::data::RomAddr;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use super::enemy_lib as lib;

mod arrival;
mod attacks;
mod decide;
pub(crate) mod hit;
mod stance;

/// Fields of the family beyond the common ones of `Enemy` (`record!`). A meaning guessed from the code alone is
/// marked (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct WarMachine(pub Obj);

impl WarMachine {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl WarMachine {
        /// Current and previous look.
        look, set_look, LOOK: u8 @ 0xa0;
        look_prev, set_look_prev, LOOK_PREV: u8 @ 0xa1;
        /// State to resume after a turn-around (long).
        resume_state, set_resume_state, RESUME_STATE: u32 @ 0xa2;
        timer, set_timer, TIMER: u16 @ 0xa6;
        timer2, set_timer2, TIMER2: u16 @ 0xa8;
        phase, set_phase, PHASE: u8 @ 0xaa;
        flag, set_flag, FLAG: u8 @ 0xad;
        count, set_count, COUNT: u8 @ 0xae;
        /// Non-zero: the charge aims at position `WarMachine::GOAL_X` instead of the edge of the screen.
        aimed, set_aimed, AIMED: u8 @ 0xb2;
        goal_x, set_goal_x, GOAL_X: u16 @ 0xbc;
    }
}
use crate::world::objects::pools;
use crate::world::objects::Current;

const SET_MOVE: RomAddr = RomAddr(0x171a52);
const SET_WEAPON: RomAddr = RomAddr(0x171a7e);
const SET_FLY_A: RomAddr = RomAddr(0x171b76);
const SET_FLY_B: RomAddr = RomAddr(0x171c1a);

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
    if x.u8(WarMachine::FLAG) != 0 {
        x.set8(Enemy::FRAME_TAG, 0);
    }
    if x.u8(Enemy::ACTIVE) != 0 {
        x.reinsert();
    }
}

// --- aides --------------------------------------------------------------------------------------

/// Starts animation `n` of the movement set.
fn mv<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_MOVE.0, n);
}

/// Starts animation `n` of the weapon set.
fn weapon<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_WEAPON.0, n);
}

/// Look (`+0xa0`) and its twin (`+0xa1`).
fn look<R: Bench>(x: &mut Cx<'_, R>, v: u8) {
    x.set8(WarMachine::LOOK, v);
    x.set8(WarMachine::LOOK_PREV, v);
}

fn set_speed<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.select_speed(&crate::world::objects::motion::SPEEDS_B, n as u16);
}

/// Spark (0x1172e) number `n`.
fn spark<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::debris(x, n);
}

/// Camera X (single screen or second screen).
fn camera_x<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    if var::CAMERA_SELECT.get(x.b) != 0 {
        var::CAMERA2_X.get(x.b)
    } else {
        var::CAMERA1_X.get(x.b)
    }
}

/// Orientation: `forward` gives 0x18 when the orientation is zero, otherwise 8.
fn heading<R: Bench>(x: &mut Cx<'_, R>) -> u8 {
    if x.u8(f::FLIP) != 0 {
        8
    } else {
        0x18
    }
}

/// The state "stubs": small routines that set the next state (0x791a0-0x792d6); the AI decisions end up there by jump.
mod go {
    use super::*;

    pub fn arrive<R: Bench>(x: &mut Cx<'_, R>, s6: u32) {
        x.set8(WarMachine::FLAG, 1);
        set_state_long(x, 0x0200_0000 | s6 << 8);
    }
    /// 0x791d0.
    pub fn stand<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(Enemy::VULNERABLE, 1);
        set_state_long(x, 0x0202_0000);
    }
    /// 0x791e0: turn-around (state to resume saved).
    pub fn turn<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(Enemy::VULNERABLE, 0);
        let state = x.u32(f::STATE);
        x.set32(WarMachine::RESUME_STATE, state);
        set_state_long(x, 0x0202_0200);
    }
    /// 0x791f4: shot A (shot type 0x34, or 0x36 depending on the profile).
    pub fn fire_a<R: Bench>(x: &mut Cx<'_, R>) {
        let kind = if x.u8(Enemy::GROUP) != 0 { 0x36 } else { 0x34 };
        x.set8(Enemy::AI_POINT_SET, kind);
        x.set8(Enemy::VULNERABLE, 1);
        set_state_long(x, 0x0202_0400);
    }
    /// 0x79216: shot B (0x35 or 0x37).
    pub fn fire_b<R: Bench>(x: &mut Cx<'_, R>) {
        let kind = if x.u8(Enemy::GROUP) != 0 { 0x37 } else { 0x35 };
        x.set8(Enemy::AI_POINT_SET, kind);
        x.set8(Enemy::VULNERABLE, 1);
        set_state_long(x, 0x0202_0600);
    }
    /// 0x79238 (`aimed` = 0) and 0x7924a (`aimed` = 1): charge.
    pub fn charge<R: Bench>(x: &mut Cx<'_, R>, aimed: u8) {
        x.set8(Enemy::VULNERABLE, 0);
        x.set8(WarMachine::AIMED, aimed);
        set_state_long(x, 0x0202_0800);
    }
    /// 0x7925e to 0x792be: move of substate 4 (`s6` = 0, 2, 4, 6, 8, 0xa, 0xc); `flag`: sets `+0xad` (stubs 0x79288 and
    /// 0x792aa).
    pub fn attack<R: Bench>(x: &mut Cx<'_, R>, s6: u32, flag: bool) {
        if flag {
            x.set8(WarMachine::FLAG, 1);
        }
        x.set8(Enemy::VULNERABLE, 0);
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
    x.set8(Enemy::HP_TIERS, 2);
    x.set8(f::BOX_SET, 0x20);
    lib::initial_hp(x);
    lib::spread_index(x);
    x.set8(Enemy::ACTIVE, 1);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set_bit(Enemy::DISPLAY, 1, true);
    for (off, v) in [
        (Enemy::AI_BYTE_ROW, 0),
        (Enemy::AI_STANCE_ROW, 0),
        (Enemy::AI_WANDER_ROW, 2),
        (Enemy::AI_BOX_ANIM, 0),
        (Enemy::AI_DECISION_A, 1),
        (Enemy::AI_DECISION_B, 1),
        (Enemy::AI_WAIT_MASKS, 6),
        (Enemy::AI_WAIT_ROW, 1),
        (Enemy::AI_BOX_CHANCE, 7),
        (Enemy::AI_SHOT, 0xff),
        (Enemy::AI_ROW_8E, 3),
        (Enemy::AI_68, 0xb),
        (Enemy::AI_69, 1),
        (Enemy::VULNERABLE, 0),
        (WarMachine::LOOK, 0),
        (WarMachine::LOOK_PREV, 0xff),
        (Enemy::EVENT_KIND, 0xc),
        (Enemy::EVENT_LOW, 0),
        (Enemy::LOOT_ROW, 0x1e),
        (Enemy::LOOT, 0x19),
    ] {
        x.set8(off, v);
    }
    pick_target(x);
    mv(x, 4);
    // the starting state depends on the variant: 0, 2 or 4 for `+6`
    match x.u8(Enemy::VARIANT) {
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
    ENEMY_27 0x77d12 "enemy_27" run,
    // get up after a common reaction (table 0x3e000 of `reactions.rs`)
    ENEMY_27_GETUP 0x78c3c "enemy_27_getup" hit::recover,
}
