//! Boss of type 6 (0x8430e), stage 24: the Manticore (h). See `docs/families/boss.md`.
//!
//! Same template as boss 17 (`boss_17`): `+4` state (0 creation, 2 active, 4 hit, 6 release), `+5`
//! group, `+6` and `+7` steps; state changes are set in one go (long `+4..+7`).
//!
//! | `+5` | File | Role |
//! |---|---|---|
//! | 0 | `intro` | arrival: leap from the right, roar, wait for the camera |
//! | 2 | `fight` | combat: wait, about-face, walk, charge (`+6` = 8), shot in a leap (10) |
//! | 4 | `attacks` | five attacks (`+6` = 0 to 8) |
//! | 6 to 12 | here | wait while no player is alive |
//! | 14 | here | resumption after a hit, with no phase |
//! | state 4 | `hit` | reactions to hits, death |
//!
//! Animations: 0x177874 (ground), 0x17788c (flight), 0x1778a4 (knockback). Non-zero `+3` variants are
//! parts linked to the host `+0x46`: they die with it (`guard`).

use crate::families::cx::Bench;
use super::boss_common::*;
use super::cx::handlers;
use crate::objects::Obj;

mod attacks;
mod decide;
mod fight;
pub(crate) mod hit;
mod intro;

mod g {
    pub const STUN: u32 = 0x56;
    pub const VULNERABLE: u32 = 0x67;
    pub const WALK_STEPS: u32 = 0x6b;
    pub const APPROACH: u32 = 0x6e;
    pub const WALK_DELAY: u32 = 0x71;
    pub const SHOT: u32 = 0x79;
    pub const TIMER: u32 = 0xa6;
    pub const COUNT: u32 = 0xa8;
    pub const ANIM_KIND: u32 = 0xa0;
    pub const ANIM_LAST: u32 = 0xa1;
    pub const SAVED_STATE: u32 = 0xa2;
    pub const PHASE: u32 = 0xaa;
    pub const INVULNERABLE: u32 = 0xad;
    /// Charge: 0 toward the camera, 1 toward the target's saved position.
    pub const CHARGE_AT_TARGET: u32 = 0xb2;
    /// Linked shot (word) and toggle of the wing-beat animation.
    pub const CHILD: u32 = 0xb0;
    pub const FLAP: u32 = 0xb3;
    pub const MARK_X: u32 = 0xbc;
}
use g::*;
use crate::objects::pools;
use crate::objects::Current;

const SET_BODY: u32 = 0x177874;
const SET_FLY: u32 = 0x17788c;
const SET_HURT: u32 = 0x1778a4;

fn body<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_BODY, n);
}

fn fly<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_FLY, n);
}

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    guard(x);
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        _ => x.free(&pools::BOSS),
    }
    if x.u8(INVULNERABLE) != 0 {
        x.set8(0x38, 0);
    }
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

/// 0x859b2: a linked part (non-zero variant) falls with its host (`+0x46`).
fn guard<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Obj::VARIANT) == 0 || x.u16(f::STATE) == 0x0406 {
        return;
    }
    let host = x.linked(f::HOST);
    if host.u16(x.b, f::HP) as i16 > 0 {
        return;
    }
    x.set32(f::STATE, 0x0406_0000);
    x.set8(0, 2);
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    x.set_a5_u8(0x50, 1);
    x.set_a5_u32(0x38, 0x0459_5900);
    pick_target(x);
    x.set8(0xab, 2);
    x.set8(f::BOX_SET, 0x26);
    lib::initial_hp_boss(x);
    var::SCENE_LEVEL.set(x.b, 0);
    lib::spread_index(x);
    lib::set_immunities(x);
    lib::clear_popup(x);
    lib::empty_popups(x);
    lib::clear_all_popups(x);
    for (off, v) in [(0x76, 0), (0x70, 0), (0x6a, 2), (0x75, 0)] {
        x.set8(off, v);
    }
    x.set_bit(0x1, 1, true);
    x.set8(0x93, 1);
    x.set8(0x9a, 0xff);
    for (off, v) in [(0x72, 1), (0x73, 1), (0x6c, 6), (0x6d, 1), (0x74, 7), (0x78, 0xff), (0x8e, 3), (0x68, 8), (0x69, 1), (VULNERABLE, 0)] {
        x.set8(off, v);
    }
    x.set8(ANIM_KIND, 0);
    x.set8(ANIM_LAST, 0xff);
    for (off, v) in [(0x7e, 0x4a), (0x7f, 0), (0x80, 0x21), (0x81, 0x1b)] {
        x.set8(off, v);
    }
    body(x, 0);
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else if var::PAUSE.get(x.b) == 0 {
        match x.u8(f::SUB) {
            0 => intro::run(x),
            2 => fight::run(x),
            4 => attacks::run(x),
            6 | 8 | 10 | 12 => wait_for_players(x),
            _ => recovered(x),
        }
    }
    lib::keep_in_zone(x);
    lib::collide_terrain(x);
}

/// `+5` = 6 to 12 (0x851d6): waits for players to come back.
fn wait_for_players<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PLAYERS_ALIVE.get(x.b) as i8 > 0 {
        return decide::chase(x);
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(ANIM_LAST, 0xff);
            body(x, 0);
        }
        2 => {
            lib::step(x);
        }
        _ => {}
    }
}

/// `+5` = 14 (0x8520e): resumption (the only `+6` state is the end of getting up).
fn recovered<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        hit::recovered_tail(x);
    }
}

handlers! {
    0x8430e "boss_6" run,
    // getting up after a common reaction (table 0x3e000 of `reactions.rs`)
    0x85490 "boss_6_getup" hit::recovered_tail,
}
