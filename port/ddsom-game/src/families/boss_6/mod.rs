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
use crate::data::RomAddr;
use crate::record::record;
use super::boss_common::*;
use super::cx::handlers;
use crate::objects::Obj;

mod attacks;
mod decide;
mod fight;
pub(crate) mod hit;
mod intro;

/// Fields of this boss beyond those of `Enemy` and `Boss` (`record!`). A meaning guessed from the code alone is
/// marked (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Boss6(pub Obj);

impl Boss6 {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Boss6 {
        approach, set_approach, APPROACH: u8 @ 0x6e;
        count, set_count, COUNT: u16 @ 0xa8;
        /// Linked shot (word) and toggle of the wing-beat animation.
        child, set_child, CHILD: u8 @ 0xb0;
        /// Charge: 0 toward the camera, 1 toward the target's saved position.
        charge_at_target, set_charge_at_target, CHARGE_AT_TARGET: u8 @ 0xb2;
        flap, set_flap, FLAP: u8 @ 0xb3;
    }
}

/// The ESHL 0x2a that the boss carries (`Boss6::CHILD`; `eshl_c::shot42`): the boss arms it, triggers its salvo and
/// waits for its end.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Salvo(pub Obj);

impl Salvo {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Salvo {
        /// Set by the boss: the shot starts its wind-up.
        armed, set_armed, ARMED: u8 @ 0x58;
        /// Set by the boss: the shot fires its burst.
        trigger, set_trigger, TRIGGER: u8 @ 0x59;
        /// Set by the shot at the end of its salvo.
        done, set_done, DONE: u8 @ 0x5a;
    }
}
use crate::objects::pools;
use crate::objects::Current;

const SET_BODY: RomAddr = RomAddr(0x177874);
const SET_FLY: RomAddr = RomAddr(0x17788c);
const SET_HURT: RomAddr = RomAddr(0x1778a4);

fn body<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_BODY.0, n);
}

fn fly<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_FLY.0, n);
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
    if x.u8(Boss::INVULNERABLE) != 0 {
        x.set8(Enemy::FRAME_TAG, 0);
    }
    if x.u8(Enemy::ACTIVE) != 0 {
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
    x.set8(Enemy::ACTIVE, 2);
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    var::BOSS_FIGHT.set(x.b, 1);
    var::STAGE_CLOCK.set(x.b, 0x0459_5900);
    pick_target(x);
    x.set8(Boss::PHASES, 2);
    x.set8(f::BOX_SET, 0x26);
    lib::initial_hp_boss(x);
    var::SCENE_LEVEL.set(x.b, 0);
    lib::spread_index(x);
    lib::set_immunities(x);
    lib::clear_popup(x);
    lib::empty_popups(x);
    lib::clear_all_popups(x);
    for (off, v) in [(Enemy::AI_BYTE_ROW, 0), (Enemy::AI_STANCE_ROW, 0), (Enemy::AI_WANDER_ROW, 2), (Enemy::AI_BOX_ANIM, 0)] {
        x.set8(off, v);
    }
    x.set_bit(Enemy::DISPLAY, 1, true);
    x.set8(Boss::DEATH_CREDIT, 1);
    x.set8(Boss::DEATH_VALUE, 0xff);
    for (off, v) in [(Enemy::AI_DECISION_A, 1), (Enemy::AI_DECISION_B, 1), (Enemy::AI_WAIT_MASKS, 6), (Enemy::AI_WAIT_ROW, 1), (Enemy::AI_BOX_CHANCE, 7), (Enemy::AI_SHOT, 0xff), (Enemy::AI_ROW_8E, 3), (Enemy::AI_68, 8), (Enemy::AI_69, 1), (Enemy::VULNERABLE, 0)] {
        x.set8(off, v);
    }
    x.set8(Boss::ANIM_KIND, 0);
    x.set8(Boss::ANIM_LAST, 0xff);
    for (off, v) in [(Enemy::EVENT_KIND, 0x4a), (Enemy::EVENT_LOW, 0), (Enemy::LOOT_ROW, 0x21), (Enemy::LOOT, 0x1b)] {
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
            x.set8(Boss::ANIM_LAST, 0xff);
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
    BOSS_6 0x8430e "boss_6" run,
    // getting up after a common reaction (table 0x3e000 of `reactions.rs`)
    BOSS_6_GETUP 0x85490 "boss_6_getup" hit::recovered_tail,
}
