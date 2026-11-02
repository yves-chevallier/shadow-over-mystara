//! Boss of type 18 (0x975ec), stage 26: the Lich (h). See `docs/families/boss.md`.
//!
//! From the family of bosses 17 and 21 (enemy skeleton `+4` state, `+5` group, `+6` and `+7` steps, states set in one
//! go as longs, two-level draws in the ROM tables); its particularity is the hit state: most of the hit codes
//! `+0x49` have a reaction of their own (nine routines: knockback, fall, jump, whirlwind...).
//!
//! | `+5` | File | Role |
//! |---|---|---|
//! | 0 | `intro` | arrival cutscene (delays and sounds, then combat) |
//! | 2 | `fight` | combat: wait, about-face, walk (`+6` = 4, 6), charge (8), recovery (10) |
//! | 4 | `attacks` | eleven attacks (`+6` = 0 to 20) |
//! | 6, 8, 10 | here | wait while no player is alive |
//! | state 4 | `hit` | reactions to hits (table of nine routines), death |
//!
//! Animations: 0x181ae0 (movements), 0x181af2 (hits), 0x181b10 (reactions).

use crate::families::cx::Bench;
use super::boss_common::*;
use super::cx::handlers;

mod attacks;
mod decide;
mod fight;
mod hit;
mod intro;

mod g {
    pub const REACTION: u32 = 0x49;
    pub const STUN: u32 = 0x56;
    pub const VULNERABLE: u32 = 0x67;
    pub const WALK_STEPS: u32 = 0x6b;
    pub const APPROACH: u32 = 0x6e;
    pub const WALK_DELAY: u32 = 0x71;
    pub const SHOT: u32 = 0x79;
    pub const SHAKE: u32 = 0x90;
    pub const TIMER: u32 = 0xa6;
    pub const COUNT: u32 = 0xa8;
    pub const ANIM_KIND: u32 = 0xa0;
    pub const ANIM_LAST: u32 = 0xa1;
    pub const SAVED_STATE: u32 = 0xa2;
    pub const PHASE: u32 = 0xaa;
    pub const PHASES: u32 = 0xab;
    pub const INVULNERABLE: u32 = 0xad;
    /// Number of attacks 4 allowed (`+0xae`), dropped objects still in play (`+0xaf`), linked object (`+0xb0`).
    pub const BUDGET_4: u32 = 0xae;
    pub const SPAWNED: u32 = 0xaf;
    pub const CHILD: u32 = 0xb0;
    /// Attacks remaining of each kind.
    pub const BUDGET_A: u32 = 0xb2;
    pub const BUDGET_B: u32 = 0xb3;
    pub const BUDGET_C: u32 = 0xb4;
    pub const BUDGET_D: u32 = 0xb5;
    pub const BUDGET_E: u32 = 0xb6;
    pub const BUDGET_F: u32 = 0xb7;
    /// Pursuit timer (word `+0xb8`) and step between two aimings (`+0xba`, `+0xbb`).
    pub const CHASE_TIME: u32 = 0xb8;
    pub const AIM_TICK: u32 = 0xba;
    pub const AIM_ON: u32 = 0xbb;
}
use g::*;
use crate::objects::pools;
use crate::objects::Current;

const SET_BODY: u32 = 0x181ae0;
const SET_STRIKE: u32 = 0x181af2;
const SET_HURT: u32 = 0x181b10;

fn body<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_BODY, n);
}

fn strike<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_STRIKE, n);
}

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        _ => x.free(&pools::BOSS),
    }
    if var::GAME_ON.get(x.b) == 0 {
        x.set8(0x3a, 0);
    }
    if x.u8(INVULNERABLE) != 0 {
        x.set8(0x38, 0);
    }
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

// --- state 0: creation --------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    x.set_a5_u8(0x50, 1);
    x.set_a5_u32(0x38, 0x0459_5900);
    lib::pick_target_any(x);
    x.set8(PHASES, 2);
    x.set8(f::BOX_SET, 0x32);
    lib::initial_hp_boss(x);
    var::SCENE_LEVEL.set(x.b, 0);
    lib::spread_index(x);
    lib::clear_popup(x);
    lib::empty_popups(x);
    lib::clear_all_popups(x);
    x.set8(0x93, 1);
    x.set8(0x9a, 0xff);
    for off in [0x76, 0x70, 0x6a, 0x75, 0x8e] {
        x.set8(off, 0);
    }
    for (off, v) in [(0x72, 1), (0x73, 1), (0x6c, 6), (0x6d, 6), (0x74, 7), (0x78, 0xff), (0x68, 8), (0x69, 1), (VULNERABLE, 0)] {
        x.set8(off, v);
    }
    x.set8(ANIM_KIND, 0);
    x.set8(ANIM_LAST, 0xff);
    for (off, v) in [(BUDGET_A, 6), (BUDGET_B, 0xc), (BUDGET_C, 3), (BUDGET_D, 6), (BUDGET_E, 5), (BUDGET_F, 4)] {
        x.set8(off, v);
    }
    for (off, v) in [(0x7e, 0x26), (0x7f, 0), (0x80, 0x22), (0x81, 0x1c)] {
        x.set8(off, v);
    }
    x.set32(f::IMMUNITY, 0xffff_ffff);
    if var::GAME_ON.get(x.b) != 0 {
        x.set8(0, 2);
        x.set_bit(0x1, 1, true);
        let px = var::CAMERA1_X.get(x.b).wrapping_add(0xc0);
        let py = var::CAMERA1_Y.get(x.b).wrapping_add(0x69);
        x.set16(0x8, px);
        x.set16(0x10, py);
        x.set16(TIMER, 0x3c);
        if let Some(e) = alloc_retry(x, &pools::EFFCT) {
            e.set_u8(x.b, 0, 1);
            e.set_u8(x.b, 2, 0x26);
            e.set_u16(x.b, 0x42, x.o.0 as u16);
        }
    } else {
        x.set_bit(0x1, 1, false);
        decide::wait(x);
    }
    body(x, 2);
}

// --- state 2: active -----------------------------------------------------------------------------

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else if var::PAUSE.get(x.b) == 0 {
        match x.u8(f::SUB) {
            0 => intro::run(x),
            2 => fight::run(x),
            4 => attacks::run(x),
            _ => wait_for_players(x),
        }
    }
    lib::keep_in_zone(x);
    lib::collide_terrain(x);
}

/// `+5` = 6, 8, 10 (0x98640): waits for players to come back.
fn wait_for_players<R: Bench>(x: &mut Cx<'_, R>) {
    if var::GAME_ON.get(x.b) != 0 && (var::PLAYERS_ALIVE.get(x.b) as i8) > 0 {
        return decide::chase(x);
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(ANIM_LAST, 0xff);
            body(x, 8);
        }
        2 => {
            lib::step(x);
        }
        _ => {}
    }
}

handlers! {
    0x975ec "boss_18" run,
}
