//! Boss of type 17 (0x952b4), stages 13 and 44 (h: "Tel'Arin" in the atlas's debug names). See
//! `docs/families/boss.md`.
//!
//! Fighter with three variants (`+3`): 0 is the boss of stage 13 (stage arrival, combat, death and ending
//! cutscene), 1 and 2 are side appearances (stage 44). Enemy skeleton (`+4` state, `+5` group, `+6` and `+7` steps);
//! active states (`+5`):
//!
//! | `+5` | File | Role |
//! |---|---|---|
//! | 0 | `intro` | cutscenes: arrival (`+6` = 0 and 2), death and end of stage (4), variants 1 and 2 (6, 8) |
//! | 2 | `fight` | combat: walk toward a player (`+6` = 4, 6), charge (8, 10), everything is decided by `decide` |
//! | 4 | `attacks` | nine attacks (`+6` = 0 to 16) |
//! | 6, 8, 10 | here | wait while no player is alive |
//! | state 4 | `hit` | reactions to hits, death |
//!
//! Animations: 0x18076e (movements), 0x1807a2 (hits), 0x1807c0 (knockback). `+0xa0` is the current set
//! (0x30 to 0x34: special sequences, with no shadow or zone test) and `+0xa1` the current animation.

use crate::families::cx::Bench;
use crate::data::RomAddr;
use crate::record::record;
use crate::player::Player;
use super::eshl_b::shot32::Shot32;
use super::eshl_b::shot33::Shot33;
use super::eshl_b::ShotB;
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
pub struct Boss17(pub Obj);

impl Boss17 {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Boss17 {
        /// Number of dropped objects still in play.
        spawned, set_spawned, SPAWNED: u8 @ 0xaf;
        /// Linked object (word): shot, core or message.
        child, set_child, CHILD: u16 @ 0xb0;
        /// Attacks remaining of each kind (`0xb2`, `0xb3`, `0xb4`).
        budget_a, set_budget_a, BUDGET_A: u8 @ 0xb2;
        budget_b, set_budget_b, BUDGET_B: u8 @ 0xb3;
        budget_c, set_budget_c, BUDGET_C: u8 @ 0xb4;
        /// Slow motion (`0xbb`: 0 normal, 1 or 2 mode) and its counters.
        slow_tick, set_slow_tick, SLOW_TICK: u8 @ 0xb5;
        strikes, set_strikes, STRIKES: u8 @ 0xb6;
        strike_limit, set_strike_limit, STRIKE_LIMIT: u8 @ 0xb7;
        slow_time, set_slow_time, SLOW_TIME: u16 @ 0xb8;
        slow_count, set_slow_count, SLOW_COUNT: u8 @ 0xba;
        slow, set_slow, SLOW: u8 @ 0xbb;
        /// Y position aimed at by a charge (word; `Boss::MARK_X` for X).
        mark_y, set_mark_y, MARK_Y: u16 @ 0xbe;
    }
}
use crate::objects::pools;
use crate::objects::Current;

/// Jeux d'animations.
const SET_BODY: RomAddr = RomAddr(0x18076e);
const SET_STRIKE: RomAddr = RomAddr(0x1807a2);
const SET_HURT: RomAddr = RomAddr(0x1807c0);

/// Starts animation `n` of the movements / hits set via the AI (`0x103d6`).
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
    loop {
        x.tick_counters();
        match x.u8(f::STATE) {
            st::INIT => init(x),
            st::ACTIVE => active(x),
            st::HIT => hit::run(x),
            _ => x.free(&pools::BOSS),
        }
        if x.u8(Boss::INVULNERABLE) != 0 {
            x.set8(Enemy::FRAME_TAG, 0);
        }
        if x.u8(Enemy::ACTIVE) == 0 {
            return;
        }
        // slow motion: as long as `+0xba` stays positive, the boss replays its logic within the same frame
        if x.u32(f::STATE) & 0xffff_ff00 != decide::WAIT && x.u8(Boss17::SLOW) != 0 {
            let t = sub8(x, Boss17::SLOW_COUNT, 1);
            if t.pl() {
                continue;
            }
            if sub16(x, Boss17::SLOW_TIME, 1).gt() {
                let tick = x.u8(Boss17::SLOW);
                x.set8(Boss17::SLOW_COUNT, tick);
                attacks::slow_tick(x);
            } else {
                x.set8(Boss17::SLOW, 0);
                decide::pick_strike_limit(x);
            }
        }
        break;
    }
    if cmp8(x, Boss::ANIM_KIND, 0x30).eq() {
        x.reinsert_foreground();
    } else {
        x.reinsert();
    }
}

// --- state 0: creation --------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    var::BOSS_FIGHT.set(x.b, 1);
    var::STAGE_CLOCK.set(x.b, 0x0459_5900);
    pick_target(x);
    x.set8(Boss::PHASES, 4);
    x.set8(f::BOX_SET, 0x31);
    lib::initial_hp_boss(x);
    lib::spread_index(x);
    var::SCENE_LEVEL.set(x.b, 0);
    lib::clear_popup(x);
    lib::empty_popups(x);
    lib::clear_all_popups(x);
    decide::pick_strike_limit(x);
    x.set8(Enemy::ACTIVE, 2);
    x.set_bit(Enemy::DISPLAY, 1, true);
    if x.u8(Obj::VARIANT) != 0 {
        x.set8(Boss::DEATH_CREDIT, 1);
        x.set8(Boss::DEATH_VALUE, 0xff);
    }
    for off in [Enemy::AI_BYTE_ROW, Enemy::AI_STANCE_ROW, Enemy::AI_WANDER_ROW, Enemy::AI_BOX_ANIM] {
        x.set8(off, 0);
    }
    for (off, v) in [(Enemy::AI_DECISION_A, 1), (Enemy::AI_DECISION_B, 1), (Enemy::AI_WAIT_MASKS, 6), (Enemy::AI_WAIT_ROW, 6), (Enemy::AI_BOX_CHANCE, 7), (Enemy::AI_SHOT, 0xff), (Enemy::AI_ROW_8E, 3), (Enemy::AI_68, 0xb), (Enemy::AI_69, 1), (Enemy::VULNERABLE, 0)] {
        x.set8(off, v);
    }
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(Boss::ANIM_KIND, 0);
    x.set8(Boss::ANIM_LAST, 0xff);
    x.set8(Boss17::BUDGET_A, 0xc);
    x.set8(Boss17::BUDGET_B, 0xc);
    x.set8(Boss17::BUDGET_C, 3);
    body(x, 0);
    match x.u8(Obj::VARIANT) {
        0 => decide::start_intro(x),
        1 => decide::start_variant1(x),
        _ => decide::start_variant2(x),
    }
}

// --- state 2: active -----------------------------------------------------------------------------

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = tst8(x, f::FREEZE);
    if freeze.gt() {
        sub8(x, f::FREEZE, 1);
    } else if var::PAUSE.get(x.b) == 0 {
        match x.u8(f::SUB) {
            0 => intro::run(x),
            2 => fight::run(x),
            4 => attacks::run(x),
            _ => wait_for_players(x),
        }
    }
    // the special sequences (set 0x30 and beyond) have neither play area nor shadow
    if !cmp8(x, Boss::ANIM_KIND, 0x30).ge() {
        lib::keep_in_zone(x);
        lib::collide_terrain(x);
    }
}

/// `+5` = 6, 8, 10 (0x96958): waits for players to come back.
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

handlers! {
    0x952b4 "boss_17" run,
    // getting up after a common reaction (table 0x3e000 of `reactions.rs`)
    0x96ce0 "boss_17_getup" hit::recovered_tail,
}
