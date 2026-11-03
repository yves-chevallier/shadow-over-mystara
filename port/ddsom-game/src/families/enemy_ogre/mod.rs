//! "Ogre" enemy (ENEMY type 18, 0x703dc). See `docs/families/enemy.md`.
//!
//! Two kinds of objects share this handler: the ogre (variants `+3` = 0 to 2, or 0xff in scenes 65 and 66) and its
//! thrown "arms" (type 0x2a, outside this handler). Like the Gargoyle it relies on the library `enemy_lib`; the
//! negative variant cannot be grabbed (`tst.b (3,A0) ; bmi` skips the grab test).
//!
//! | `+5` | File | Role (h) |
//! |---|---|---|
//! | 0 | `arrival` | arrival (`+6` = 0 entrance fall, 2 appearance on the ground, 4 wait for a player, 6 hidden
//! appearance) |
//! | 2 | `stance` | movements (`+6` = 0 to 10) |
//! | 4 | `attacks` | attacks (`+6` = 0 to 8) |
//! | 6, 8, 10 | `attacks` | rest |
//! | state 4 | `hit` | reactions (table `0x715d4`), recoil, fall, death, flight |
//!
//! Animations 0x177874 and 0x17788c (movements and attacks), 0x1778a4 (death).

use crate::families::cx::Bench;
use crate::record::record;
use crate::data::RomAddr;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use super::enemy_lib as lib;
use crate::objects::Obj;

mod arrival;
mod attacks;
pub(crate) mod hit;
mod stance;

/// Fields of the family beyond the common ones of `Enemy` (`record!`). A meaning guessed from the code alone is
/// marked (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Ogre(pub Obj);

impl Ogre {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Ogre {
        look, set_look, LOOK: u8 @ 0xa0;
        look_prev, set_look_prev, LOOK_PREV: u8 @ 0xa1;
        saved_state, set_saved_state, SAVED_STATE: u32 @ 0xa2;
        /// Rounds of wait left before a new arrival (byte of `SAVED_STATE`) (h).
        wait_rounds, set_wait_rounds, WAIT_ROUNDS: u8 @ 0xa4;
        timer, set_timer, TIMER: u16 @ 0xa6;
        timer2, set_timer2, TIMER2: u16 @ 0xa8;
        phase, set_phase, PHASE: u8 @ 0xaa;
        invulnerable, set_invulnerable, INVULNERABLE: u8 @ 0xad;
        arm, set_arm, ARM: u16 @ 0xb0;
        /// Step to resume after a wait (low byte of `ARM`) (h).
        resume_step, set_resume_step, RESUME_STEP: u8 @ 0xb1;
        side_turn, set_side_turn, SIDE_TURN: u8 @ 0xb2;
        flash, set_flash, FLASH: u8 @ 0xb3;
        goal_x, set_goal_x, GOAL_X: u16 @ 0xbc;
    }
}

/// The arm of the ogre (object `Ogre::ARM`): the signals that the ogre sends it (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct OgreArm(pub Obj);

impl OgreArm {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl OgreArm {
        /// Set when the ogre rises or lands: the arm follows (h).
        raise, set_raise, RAISE: u8 @ 0x58;
        /// Set at the end of the wind-up of a strike: the arm swings (h).
        swing, set_swing, SWING: u8 @ 0x59;
        /// Set by the arm at the end of its swing (h).
        done, set_done, DONE: u8 @ 0x5a;
    }
}
use crate::objects::pools;
use crate::objects::Current;

const SET_A: RomAddr = RomAddr(0x177874);
const SET_B: RomAddr = RomAddr(0x17788c);
const SET_DEATH: RomAddr = RomAddr(0x1778a4);
const SET_FLY_AGAINST: RomAddr = RomAddr(0x177992);
const SET_FLY_ALONG: RomAddr = RomAddr(0x177a36);

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    arm_dies_with_ogre(x);
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        st::DEAD => x.free(&pools::ENEMY),
        _ => {}
    }
    if x.u8(Ogre::INVULNERABLE) != 0 {
        x.set8(Enemy::FRAME_TAG, 0);
    }
    if x.u8(Enemy::ACTIVE) != 0 {
        x.reinsert();
    }
}

/// 0x71dde: an object with a negative variant (the "arm") dies with the object `+0x46` it accompanies.
fn arm_dies_with_ogre<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u8(Obj::VARIANT) as i8) >= 0 || x.u16(f::STATE) == 0x0406 {
        return;
    }
    let owner = x.linked(Enemy::HOST);
    if (owner.u16(x.b, f::HP) as i16) > 0 {
        return;
    }
    set_state_long(x, 0x0406_0000);
    x.set8(Enemy::ACTIVE, 2);
}

// --- aides --------------------------------------------------------------------------------------

fn play<R: Bench>(x: &mut Cx<'_, R>, set: u32, n: u32) {
    lib::start(x, set, n);
}

fn mv<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    play(x, SET_A.0, n);
}

fn act<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    play(x, SET_B.0, n);
}

fn look<R: Bench>(x: &mut Cx<'_, R>, v: u8) {
    x.set8(Ogre::LOOK, v);
    x.set8(Ogre::LOOK_PREV, v);
}

fn set_speed<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
}

fn spark<R: Bench>(x: &mut Cx<'_, R>, d5: u32) {
    lib::debris(x, d5);
}

fn camera_x<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA2_X.get(x.b) } else { var::CAMERA1_X.get(x.b) }
}

fn count_down<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> bool {
    let n = x.u16(off).wrapping_sub(1);
    x.set16(off, n);
    (n as i16) > 0
}

/// Orientation to the right (0x18) or the left (8) according to `flip`.
fn angle_by_flip<R: Bench>(x: &mut Cx<'_, R>) {
    let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
    x.set8(Obj::SPEED_INDEX, angle);
}

/// The ogre can be grabbed by a player (not the negative variants): it then abandons the move.
fn held_by_player<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if (x.u8(Obj::VARIANT) as i8) < 0 || !grabbed(x) {
        return false;
    }
    go::held(x);
    true
}

/// No player alive any more: retreat; true if the state has changed.
fn nobody_left<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if lib::players_alive(x) {
        return false;
    }
    go::retire(x);
    true
}

/// 0x71d98: vertical undulation in flight according to the animation key (table of longs `bob_steps`).
fn bob<R: Bench>(x: &mut Cx<'_, R>) {
    let idx = ((x.u8(f::KEY_FRAME) & 0x7f) as u32) << 2;
    let mut d = tables::table("ogre/bob_steps").at32(x.b, idx) as i32;
    if (x.u16(Ogre::TIMER2) as i16) <= 0x24 {
        d = d.wrapping_neg();
    }
    x.add32(Obj::Z, d as u32);
    if !count_down(x, Ogre::TIMER2) {
        x.set16(Ogre::TIMER2, 0x48);
    }
}

/// Spark of the animation key: bit 7 of `+0x33` → effect 0x11a36, alternated by `+0xb3`.
fn flash_key<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u8(f::KEY_FRAME) as i8) < 0 {
        let k = x.u8(f::KEY_FRAME) & 0x7f;
        x.set8(f::KEY_FRAME, k);
        let f = x.u8(Ogre::FLASH) ^ 1;
        x.set8(Ogre::FLASH, f);
        lib::ground_debris(x, f);
    }
}

// --- state transitions: the next move ----------------------------------------------------------

mod go {
    use super::*;

    /// 0x71cac : debout.
    pub fn stand<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(Enemy::VULNERABLE, 1);
        set_state_long(x, 0x0202_0000);
    }
    /// 0x71cbc: turn-around, saving the current state.
    pub fn turn_around<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(Enemy::VULNERABLE, 0);
        let s = x.u32(f::STATE);
        x.set32(Ogre::SAVED_STATE, s);
        set_state_long(x, 0x0202_0200);
    }
    pub fn wander<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(Enemy::VULNERABLE, 1);
        x.set8(Enemy::AI_POINT_SET, 0x2c);
        set_state_long(x, 0x0202_0400);
    }
    pub fn wander_far<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(Enemy::VULNERABLE, 1);
        x.set8(Enemy::AI_POINT_SET, 0x2d);
        set_state_long(x, 0x0202_0600);
    }
    pub fn dive<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(Enemy::VULNERABLE, 0);
        x.set8(Ogre::SIDE_TURN, 0);
        set_state_long(x, 0x0202_0800);
    }
    pub fn dive_side<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(Enemy::VULNERABLE, 0);
        x.set8(Ogre::SIDE_TURN, 1);
        set_state_long(x, 0x0202_0800);
    }
    pub fn leap<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(Enemy::VULNERABLE, 0);
        set_state_long(x, 0x0202_0a00);
    }
    pub fn strike_0<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(Enemy::VULNERABLE, 1);
        set_state_long(x, 0x0204_0000);
    }
    pub fn strike_2<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(Enemy::VULNERABLE, 0);
        set_state_long(x, 0x0204_0200);
    }
    pub fn strike_4<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(Enemy::VULNERABLE, 0);
        set_state_long(x, 0x0204_0400);
    }
    pub fn strike_6<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(Ogre::INVULNERABLE, 1);
        x.set8(Enemy::VULNERABLE, 0);
        set_state_long(x, 0x0204_0600);
    }
    pub fn strike_8<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(Ogre::INVULNERABLE, 1);
        x.set8(Enemy::VULNERABLE, 0);
        set_state_long(x, 0x0204_0800);
    }
    pub fn retire<R: Bench>(x: &mut Cx<'_, R>) {
        set_state_long(x, 0x020a_0000);
    }
    pub fn held<R: Bench>(x: &mut Cx<'_, R>) {
        set_state_long(x, 0x0408_0000);
    }
}

// --- state 0: creation --------------------------------------------------------------------------

/// 0x71e04: initial HP; the negative variant computes them like a type 6 (0x453c) then resumes type 0x12.
fn initial_hp<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u8(Obj::VARIANT) as i8) >= 0 {
        return lib::initial_hp(x);
    }
    x.set8(Enemy::KIND, 6);
    lib::initial_hp_boss(x);
    x.set8(Enemy::KIND, 0x12);
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    x.set8(Enemy::HP_TIERS, 2);
    x.set8(f::BOX_SET, 0x26);
    initial_hp(x);
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
        (Enemy::AI_68, 8),
        (Enemy::AI_69, 1),
        (Enemy::VULNERABLE, 0),
        (Ogre::LOOK, 0),
        (Ogre::LOOK_PREV, 0xff),
        (Enemy::EVENT_KIND, 0x17),
        (Enemy::EVENT_LOW, 0),
        (Enemy::LOOT_ROW, 0x21),
        (Enemy::LOOT, 0x1b),
    ] {
        x.set8(off, v);
    }
    pick_target(x);
    mv(x, 0);
    x.set8(Ogre::INVULNERABLE, 1);
    let state = match x.u8(Obj::VARIANT) as i8 {
        v if v < 0 => 0x0200_0000,
        0 => 0x0200_0200,
        1 => 0x0200_0400,
        2 => 0x0200_0600,
        _ => return,
    };
    set_state_long(x, state);
}

// --- state 2: active -----------------------------------------------------------------------------

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::FREEZE) as i8 > 0 {
        x.add8(f::FREEZE, 0xff);
        zone_and_terrain(x);
        return;
    }
    if !paused(x) {
        match x.u8(f::SUB) {
            0 => arrival::run(x),
            2 => stance::run(x),
            4 => attacks::run(x),
            6 | 8 | 10 => attacks::rest(x),
            _ => {}
        }
        if x.u8(f::SUB) == 0 {
            return;
        }
    }
    zone_and_terrain(x);
}

handlers! {
    ENEMY_18 0x703dc "enemy_18" run,
    // get up after a common reaction (table 0x3e000 of `reactions.rs`)
    ENEMY_18_GETUP 0x717ea "enemy_18_getup" hit::regroup,
}
