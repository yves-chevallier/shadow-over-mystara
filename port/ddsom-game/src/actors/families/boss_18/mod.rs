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

use crate::actors::families::cx::Bench;
use crate::world::data::RomAddr;
use crate::world::objects::Obj;
use crate::world::record::record;
use crate::actors::player::Player;
use super::eshl_b::shot32::Shot32;
use super::eshl_b::shot33::Shot33;
use super::eshl_b::ShotB;
use super::boss_common::*;
use super::cx::handlers;

mod attacks;
mod decide;
mod fight;
mod hit;
mod intro;

/// Fields of this boss beyond those of `Enemy` and `Boss` (`record!`). A meaning guessed from the code alone is
/// marked (h).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Boss18(pub Obj);

impl Boss18 {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Boss18 {
        /// Number of attacks 4 allowed (`+0xae`), dropped objects still in play (`+0xaf`), linked object (`+0xb0`).
        budget_4, set_budget_4, BUDGET_4: u8 @ 0xae;
        spawned, set_spawned, SPAWNED: u8 @ 0xaf;
        child, set_child, CHILD: u16 @ 0xb0;
        /// Attacks remaining of each kind.
        budget_a, set_budget_a, BUDGET_A: u8 @ 0xb2;
        budget_b, set_budget_b, BUDGET_B: u8 @ 0xb3;
        budget_c, set_budget_c, BUDGET_C: u8 @ 0xb4;
        budget_d, set_budget_d, BUDGET_D: u8 @ 0xb5;
        budget_e, set_budget_e, BUDGET_E: u8 @ 0xb6;
        budget_f, set_budget_f, BUDGET_F: u8 @ 0xb7;
        /// Pursuit timer (word `+0xb8`) and step between two aimings (`+0xba`, `+0xbb`).
        chase_time, set_chase_time, CHASE_TIME: u16 @ 0xb8;
        aim_tick, set_aim_tick, AIM_TICK: u8 @ 0xba;
        aim_on, set_aim_on, AIM_ON: u8 @ 0xbb;
    }
}
use crate::world::objects::pools;
use crate::world::objects::Current;

const SET_BODY: RomAddr = RomAddr(0x181ae0);
const SET_STRIKE: RomAddr = RomAddr(0x181af2);
const SET_HURT: RomAddr = RomAddr(0x181b10);

fn body<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_BODY.0, n);
}

fn strike<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_STRIKE.0, n);
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
        x.set8(Boss::SHADOW_B, 0);
    }
    if x.u8(Boss::INVULNERABLE) != 0 {
        x.set8(Enemy::FRAME_TAG, 0);
    }
    if x.u8(Enemy::ACTIVE) != 0 {
        x.reinsert();
    }
}

// --- state 0: creation --------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    var::BOSS_FIGHT.set(x.b, 1);
    var::STAGE_CLOCK.set(x.b, 0x0459_5900);
    lib::pick_target_any(x);
    x.set8(Boss::PHASES, 2);
    x.set8(f::BOX_SET, 0x32);
    lib::initial_hp_boss(x);
    var::SCENE_LEVEL.set(x.b, 0);
    lib::spread_index(x);
    lib::clear_popup(x);
    lib::empty_popups(x);
    lib::clear_all_popups(x);
    x.set8(Boss::DEATH_CREDIT, 1);
    x.set8(Boss::DEATH_VALUE, 0xff);
    for (off, v) in [(Enemy::AI_BYTE_ROW, 0), (Enemy::AI_STANCE_ROW, 0), (Enemy::AI_WANDER_ROW, 0), (Enemy::AI_BOX_ANIM, 0), (Enemy::AI_ROW_8E, 0)] {
        x.set8(off, v);
    }
    for (off, v) in [(Enemy::AI_DECISION_A, 1), (Enemy::AI_DECISION_B, 1), (Enemy::AI_WAIT_MASKS, 6), (Enemy::AI_WAIT_ROW, 6), (Enemy::AI_BOX_CHANCE, 7), (Enemy::AI_SHOT, 0xff), (Enemy::AI_68, 8), (Enemy::AI_69, 1), (Enemy::VULNERABLE, 0)] {
        x.set8(off, v);
    }
    x.set8(Boss::ANIM_KIND, 0);
    x.set8(Boss::ANIM_LAST, 0xff);
    for (off, v) in [(Boss18::BUDGET_A, 6), (Boss18::BUDGET_B, 0xc), (Boss18::BUDGET_C, 3), (Boss18::BUDGET_D, 6), (Boss18::BUDGET_E, 5), (Boss18::BUDGET_F, 4)] {
        x.set8(off, v);
    }
    for (off, v) in [(Enemy::EVENT_KIND, 0x26), (Enemy::EVENT_LOW, 0), (Enemy::LOOT_ROW, 0x22), (Enemy::LOOT, 0x1c)] {
        x.set8(off, v);
    }
    x.set32(f::IMMUNITY, 0xffff_ffff);
    if var::GAME_ON.get(x.b) != 0 {
        x.set8(Enemy::ACTIVE, 2);
        x.set_bit(Enemy::DISPLAY, 1, true);
        let px = var::CAMERA1_X.get(x.b).wrapping_add(0xc0);
        let py = var::CAMERA1_Y.get(x.b).wrapping_add(0x69);
        x.set16(Obj::X, px);
        x.set16(Obj::Y, py);
        x.set16(Boss::TIMER, 0x3c);
        if let Some(e) = alloc_retry(x, &pools::EFFCT) {
            e.set_u8(x.b, Obj::ACTIVE, 1);
            e.set_u8(x.b, Obj::KIND, 0x26);
            e.set_u16(x.b, Enemy::CREATOR, x.o.0 as u16);
        }
    } else {
        x.set_bit(Enemy::DISPLAY, 1, false);
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
            x.set8(Boss::ANIM_LAST, 0xff);
            body(x, 8);
        }
        2 => {
            lib::step(x);
        }
        _ => {}
    }
}

handlers! {
    BOSS_18 0x975ec "boss_18" run,
}
