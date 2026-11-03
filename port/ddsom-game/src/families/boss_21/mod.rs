//! Boss of type 21 (0x9947a), stage 44. See `docs/families/boss.md`.
//!
//! Cousin of boss 17 (`boss_17`): same skeleton (`+4` state, `+5` group, `+6` and `+7` steps, states set in one go
//! as longs), same AI libraries, same slow motion (`+0xbb`); the ROM tables, the attacks and the death are its own.
//! It is linked to a host (`+0x46`): when the host dies (state 0x0406), the boss leaves the stage.
//!
//! | `+5` | File | Role |
//! |---|---|---|
//! | 0 | `intro` | arrival from the right (`+6` = 0), then entry into combat (2) |
//! | 2 | `fight` | combat: wait, about-face, walk toward a player (`+6` = 4, 6), charge (8, 10) |
//! | 4 | `attacks` | nine attacks (`+6` = 0 to 16) |
//! | 6, 8, 10 | here | wait while no player is alive |
//! | state 4 | `hit` | reactions to hits (common ROM table), stun, death |
//!
//! Animations: 0x1836ec (movements), 0x18371c (hits), 0x18373a (knockback). `+0xa0` is the current set
//! (0x30 and 0x31: sequences with no shadow or zone test) and `+0xa1` the current animation.

use crate::families::cx::Bench;
use crate::data::RomAddr;
use crate::record::record;
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
pub struct Boss21(pub Obj);

impl Boss21 {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Boss21 {
        /// Counter of hits taken during the stun.
        hits, set_hits, HITS: u16 @ 0x84;
        /// Number of dropped objects still in play.
        spawned, set_spawned, SPAWNED: u8 @ 0xaf;
        /// Linked object (word): shot.
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
const SET_BODY: RomAddr = RomAddr(0x1836ec);
const SET_STRIKE: RomAddr = RomAddr(0x18371c);
const SET_HURT: RomAddr = RomAddr(0x18373a);

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
        if x.u32(f::STATE) & 0xffff_ff00 != decide::WAIT && x.u8(Boss21::SLOW) != 0 {
            if sub8(x, Boss21::SLOW_COUNT, 1).pl() {
                continue;
            }
            if sub16(x, Boss21::SLOW_TIME, 1).gt() {
                let tick = x.u8(Boss21::SLOW);
                x.set8(Boss21::SLOW_COUNT, tick);
                slow_tick(x);
            } else {
                x.set8(Boss21::SLOW, 0);
                decide::pick_strike_limit(x);
            }
        }
        break;
    }
    // the dead host takes the boss down with it (state 0x0406 = death)
    if decide::host_dead(x) {
        x.set8(Enemy::ACTIVE, 2);
        x.set32(f::IMMUNITY, 0xffff_ffff);
    }
    x.reinsert();
}

// --- state 0: creation --------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    pick_target(x);
    x.set8(Boss::PHASES, 4);
    x.set8(f::BOX_SET, 0x31);
    lib::initial_hp_boss(x);
    lib::spread_index(x);
    decide::pick_strike_limit(x);
    x.set8(Enemy::ACTIVE, 2);
    x.set_bit(Enemy::DISPLAY, 1, true);
    for (off, v) in [(Enemy::AI_BYTE_ROW, 0), (Enemy::AI_STANCE_ROW, 0), (Enemy::AI_WANDER_ROW, 0), (Enemy::AI_BOX_ANIM, 0)] {
        x.set8(off, v);
    }
    for (off, v) in [(Enemy::AI_DECISION_A, 1), (Enemy::AI_DECISION_B, 1), (Enemy::AI_WAIT_MASKS, 6), (Enemy::AI_WAIT_ROW, 6), (Enemy::AI_BOX_CHANCE, 7), (Enemy::AI_SHOT, 0xff), (Enemy::AI_ROW_8E, 3), (Enemy::AI_68, 0xb), (Enemy::AI_69, 1), (Enemy::VULNERABLE, 0)] {
        x.set8(off, v);
    }
    x.set16(f::IMMUNITY, 0xffff);
    x.set8(Boss::ANIM_KIND, 0);
    x.set8(Boss::ANIM_LAST, 0xff);
    x.set8(Boss21::BUDGET_A, 8);
    x.set8(Boss21::BUDGET_B, 8);
    x.set8(Boss21::BUDGET_C, 3);
    for (off, v) in [(Enemy::EVENT_KIND, 0x4d), (Enemy::EVENT_LOW, 0), (Enemy::LOOT_ROW, 0), (Enemy::LOOT, 0x18)] {
        x.set8(off, v);
    }
    decide::start_intro(x);
    body(x, 0);
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

/// `+5` = 6, 8, 10 (0x9a36a): waits for players to come back.
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

// --- effects and slow motion ---------------------------------------------------------------------------

/// An effect (EFFCT) linked to the boss (`+0x42`): active byte, type, variant, orientation of the boss.
/// `retry`: insists as long as the pool is full.
fn effect<R: Bench>(x: &mut Cx<'_, R>, retry: bool, kind: u8, variant: Option<u8>, flip: bool) -> Option<Obj> {
    let e = if retry { alloc_retry(x, &pools::EFFCT)? } else { x.alloc(&pools::EFFCT)? };
    e.set_u8(x.b, Obj::ACTIVE, 1);
    e.set_u8(x.b, Obj::KIND, kind);
    if let Some(v) = variant {
        e.set_u8(x.b, Obj::VARIANT, v);
    }
    if flip {
        let side = x.u8(f::FLIP);
        e.set_u8(x.b, f::FLIP, side);
    }
    e.set_u16(x.b, Enemy::CREATOR, x.o.0 as u16);
    Some(e)
}

/// 0x9ae52: a dust cloud (effect 0x32, variant 4).
fn dust<R: Bench>(x: &mut Cx<'_, R>) {
    effect(x, false, 0x32, Some(4), true);
}

/// 0x9ae4c: same, except during the slow motion.
fn dust_unless_slow<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Boss21::SLOW) == 0 {
        dust(x);
    }
}

/// 0x9ae2a: slow-motion step: the dust falls every 11 steps (mode 1) or 7 steps (mode 2).
fn slow_tick<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Boss21::SLOW) == 0 {
        return;
    }
    x.add8(Boss21::SLOW_TICK, 1);
    let period = if x.u8(Boss21::SLOW) == 1 { 0xb } else { 7 };
    if (period as i8) > x.u8(Boss21::SLOW_TICK) as i8 {
        return;
    }
    x.set8(Boss21::SLOW_TICK, 0);
    dust(x);
}

handlers! {
    BOSS_21 0x9947a "boss_21" run,
    // getting up after a common reaction (table 0x3e000 of `reactions.rs`)
    BOSS_21_GETUP 0x9a6d4 "boss_21_getup" hit::recovered,
}
