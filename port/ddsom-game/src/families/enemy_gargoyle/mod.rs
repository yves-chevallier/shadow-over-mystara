//! "Gargoyle" enemy (ENEMY type 8, 0x63fb0). See `docs/families/enemy.md`.
//!
//! Flying creature that lands to attack (variants `+3` = 0 to 2: arrival at ground level, in flight or on the ground).
//! It relies on the common library `enemy_lib` (animation sets with attack type `+0x44`).
//!
//! | `+5` | File | Role (h) |
//! |---|---|---|
//! | 0 | `arrival` | arrival in flight (`+6` = 0 variant 0, 2 variant 1) |
//! | 2 | `stance` | movements (`+6` = 0 to 10): pursuit, turn-around, dives, circular flights |
//! | 4 | `attacks` | attacks (`+6` = 0 to 12) |
//! | 6, 8, 10 | `attacks` | rest, end-of-fight shot |
//! | 12, 14 | `attacks` | grab of a player by the creature |
//! | state 4 | `hit` | reactions to hits (table `0x6518a` by code `+0x49`), fall, death |
//!
//! `+0x50` is the target (player), `+0x54` the possible grabbed victim. Animations 0x162ebc (flight), 0x162eee
//! (attacks), 0x163014 and 0x163098 (flight against or with the orientation).

use crate::families::cx::Bench;
use crate::record::record;
use crate::data::RomAddr;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use super::enemy_lib as lib;
use crate::objects::Obj;

mod arrival;
pub(crate) mod attacks;
mod hit;
mod stance;

/// Fields of the family beyond the common ones of `Enemy` (`record!`). A meaning guessed from the code alone is
/// marked (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Gargoyle(pub Obj);

impl Gargoyle {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Gargoyle {
        hit_flags, set_hit_flags, HIT_FLAGS: u8 @ 0x57;
        look, set_look, LOOK: u8 @ 0xa0;
        look_prev, set_look_prev, LOOK_PREV: u8 @ 0xa1;
        saved_state, set_saved_state, SAVED_STATE: u32 @ 0xa2;
        timer, set_timer, TIMER: u16 @ 0xa6;
        timer2, set_timer2, TIMER2: u16 @ 0xa8;
        phase, set_phase, PHASE: u8 @ 0xaa;
        invulnerable, set_invulnerable, INVULNERABLE: u8 @ 0xad;
        grabbed_once, set_grabbed_once, GRABBED_ONCE: u8 @ 0xaf;
        side_turn, set_side_turn, SIDE_TURN: u8 @ 0xb2;
        cooldown, set_cooldown, COOLDOWN: u16 @ 0xb4;
        hit_code_saved, set_hit_code_saved, HIT_CODE_SAVED: u8 @ 0xb7;
        state_saved, set_state_saved, STATE_SAVED: u32 @ 0xb8;
        goal_x, set_goal_x, GOAL_X: u16 @ 0xbc;
        goal_depth, set_goal_depth, GOAL_DEPTH: u16 @ 0xbe;
    }
}
use crate::objects::pools;
use crate::objects::Current;

const SET_FLY: RomAddr = RomAddr(0x162ebc);
const SET_STRIKE: RomAddr = RomAddr(0x162eee);
/// Flight sets depending on whether or not the speed follows the orientation (`lib::moving_forward`).
const SET_FLY_AGAINST: RomAddr = RomAddr(0x163014);
const SET_FLY_ALONG: RomAddr = RomAddr(0x163098);

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
    if x.u8(Gargoyle::INVULNERABLE) != 0 {
        x.set8(Enemy::FRAME_TAG, 0);
    }
    if x.u8(Enemy::ACTIVE) != 0 {
        x.reinsert();
    }
}

// --- aides --------------------------------------------------------------------------------------

fn play<R: Bench>(x: &mut Cx<'_, R>, set: u32, n: u32) {
    lib::start(x, set, n);
}

fn fly<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    play(x, SET_FLY.0, n);
}

fn strike<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    play(x, SET_STRIKE.0, n);
}

fn look<R: Bench>(x: &mut Cx<'_, R>, v: u8) {
    x.set8(Gargoyle::LOOK, v);
    x.set8(Gargoyle::LOOK_PREV, v);
}

/// Speed table `n` (0x1b64) for angle `+0x2d`.
fn set_speed<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
}

/// Scenery chip (0x1172e) at impact point `d5`.
fn spark<R: Bench>(x: &mut Cx<'_, R>, d5: u32) {
    lib::debris(x, d5);
}

fn camera_x<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA2_X.get(x.b) } else { var::CAMERA1_X.get(x.b) }
}

/// Countdown of a word timer `+off`: true as long as it stays positive.
fn count_down<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> bool {
    let n = x.u16(off).wrapping_sub(1);
    x.set16(off, n);
    (n as i16) > 0
}

/// 0x65a9e: turns toward the target.
fn face_target<R: Bench>(x: &mut Cx<'_, R>) {
    let target = x.linked(f::TARGET);
    let d1 = target.u16(x.b, Obj::X);
    let flip = if (d1 as i16) < (x.u16(Obj::X) as i16) { 0 } else { 0x20 };
    x.set8(f::FLIP, flip as u8);
}

/// 0x65a30: vertical undulation in flight according to the animation key (table of longs `bob_steps`).
fn bob<R: Bench>(x: &mut Cx<'_, R>) {
    let idx = (x.u8(f::KEY_FRAME) as u32) << 2;
    let mut d = tables::table("gargoyle/bob_steps").at32(x.b, idx) as i32;
    if (x.u16(Gargoyle::TIMER2) as i16) <= 0x24 {
        d = d.wrapping_neg();
    }
    x.add32(Obj::Z, d as u32);
    if !count_down(x, Gargoyle::TIMER2) {
        x.set16(Gargoyle::TIMER2, 0x28);
    }
}

/// 0x65a72: progressive climb (table of longs `rise_steps`).
fn rise<R: Bench>(x: &mut Cx<'_, R>) {
    let idx = (x.u8(f::KEY_FRAME) as u32) << 2;
    let d = tables::table("gargoyle/rise_steps").at32(x.b, idx);
    x.add32(Obj::Z, d);
}

// --- state transitions: the next move ----------------------------------------------------------

mod go {
    use super::*;

    /// Substate 2 (movements): `+6` = 2, saves the current state in order to come back to it.
    pub fn turn_around<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(Enemy::VULNERABLE, 0);
        let s = x.u32(f::STATE);
        x.set32(Gargoyle::SAVED_STATE, s);
        set_state_long(x, 0x0202_0200);
    }
    pub fn wander<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(Enemy::VULNERABLE, 0);
        x.set8(Enemy::AI_POINT_SET, 0x3b);
        set_state_long(x, 0x0202_0400);
    }
    pub fn wander_far<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(Enemy::VULNERABLE, 0);
        x.set8(Enemy::AI_POINT_SET, 0x3c);
        set_state_long(x, 0x0202_0600);
    }
    pub fn dive<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(Enemy::VULNERABLE, 0);
        x.set8(Gargoyle::SIDE_TURN, 0);
        set_state_long(x, 0x0202_0800);
    }
    pub fn circle<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(Gargoyle::INVULNERABLE, 1);
        x.set8(Enemy::VULNERABLE, 0);
        set_state_long(x, 0x0202_0a00);
    }
    /// Substate 4 (attacks), `+6` = 0; `grab`: the creature has already grabbed its victim.
    pub fn strike_0<R: Bench>(x: &mut Cx<'_, R>, grab: bool) {
        if grab {
            x.set8(Gargoyle::HIT_FLAGS, 0);
        }
        x.set8(Gargoyle::GRABBED_ONCE, grab as u8);
        x.set8(Enemy::VULNERABLE, 0);
        set_state_long(x, 0x0204_0000);
    }
    pub fn strike_2<R: Bench>(x: &mut Cx<'_, R>, grab: bool) {
        if grab {
            x.set8(Gargoyle::HIT_FLAGS, 0);
        }
        x.set8(Gargoyle::GRABBED_ONCE, grab as u8);
        x.set8(Enemy::VULNERABLE, 1);
        set_state_long(x, 0x0204_0200);
    }
    pub fn strike_4<R: Bench>(x: &mut Cx<'_, R>, grab: bool) {
        x.set8(Gargoyle::GRABBED_ONCE, grab as u8);
        x.set8(Enemy::VULNERABLE, 0);
        set_state_long(x, 0x0204_0400);
    }
    pub fn strike_6<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(Enemy::VULNERABLE, 0);
        set_state_long(x, 0x0204_0600);
    }
    pub fn strike_8<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(Gargoyle::INVULNERABLE, 1);
        x.set8(Enemy::VULNERABLE, 0);
        set_state_long(x, 0x0204_0800);
    }
    pub fn strike_10<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(Enemy::VULNERABLE, 0);
        set_state_long(x, 0x0204_0a00);
    }
    pub fn strike_12<R: Bench>(x: &mut Cx<'_, R>) {
        x.set8(Enemy::VULNERABLE, 0);
        set_state_long(x, 0x0204_0c00);
    }
    /// No player alive any more: the creature withdraws (substate 10).
    pub fn retire<R: Bench>(x: &mut Cx<'_, R>) {
        set_state_long(x, 0x020a_0000);
    }
    /// Grabbed by a player: hit state 8.
    pub fn held<R: Bench>(x: &mut Cx<'_, R>) {
        set_state_long(x, 0x0408_0000);
    }
}

// --- state 0: creation --------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    x.set8(Enemy::HP_TIERS, 2);
    x.set8(f::BOX_SET, 0x14);
    lib::initial_hp(x);
    lib::spread_index(x);
    x.set8(Enemy::ACTIVE, 1);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set_bit(Enemy::DISPLAY, 1, true);
    for (off, v) in [
        (Enemy::AI_BYTE_ROW, 0),
        (Enemy::AI_STANCE_ROW, 0),
        (Enemy::AI_WANDER_ROW, 0),
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
        (Gargoyle::LOOK, 0),
        (Gargoyle::LOOK_PREV, 0xff),
        (Enemy::EVENT_KIND, 0x11),
        (Enemy::EVENT_LOW, 0),
        (Enemy::LOOT_ROW, 0x11),
        (Enemy::LOOT, 0x10),
    ] {
        x.set8(off, v);
    }
    pick_target(x);
    fly(x, 0);
    match x.u8(Obj::VARIANT) {
        0 => {
            x.set8(Gargoyle::INVULNERABLE, 1);
            set_state_long(x, 0x0200_0000);
        }
        1 => {
            x.set8(Gargoyle::INVULNERABLE, 1);
            set_state_long(x, 0x0200_0200);
        }
        2 => set_state_long(x, 0x0200_0400),
        _ => {}
    }
}

// --- state 2: active -----------------------------------------------------------------------------

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::FREEZE) as i8 > 0 {
        x.add8(f::FREEZE, 0xff);
    } else if !paused(x) {
        match x.u8(f::SUB) {
            0 => arrival::run(x),
            2 => stance::run(x),
            4 => attacks::run(x),
            6 | 8 | 10 => attacks::rest(x),
            12 => attacks::grab(x),
            14 => attacks::done(x),
            _ => {}
        }
    }
    if x.u8(f::SUB) != 0 {
        zone_and_terrain(x);
    }
    let s = x.u32(f::STATE);
    x.set32(Gargoyle::STATE_SAVED, s);
    cooldown(x);
}

/// 0x65ab6: countdown of the rest period (`+0xb4`); at zero, flag `+0x58` drops.
fn cooldown<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u16(Gargoyle::COOLDOWN) as i16) <= 0 {
        return;
    }
    if !count_down(x, Gargoyle::COOLDOWN) {
        x.set8(Enemy::HIT_LOCK, 0);
    }
}

handlers! {
    0x63fb0 "enemy_8" run,
    // get up after a common reaction (table 0x3e000 of `reactions.rs`)
    0x653b2 "enemy_8_getup" attacks::regroup,
}
