//! Boss of type 7 (0x859f8): the first black warrior ("DarkWarrior 1"), who also appears, motionless, in the
//! attract demonstration. See `docs/families/boss.md`.
//!
//! Same skeleton as boss 8 (the second black warrior, `boss_8`): `+4` state, `+5` group, `+6` and `+7`
//! steps, states set in one go as longs, reactions to hits through the common enemy routines (table of
//! longs 0x86c3a), slow motion one frame in three when weakened.
//!
//! | `+5` | File | Role |
//! |---|---|---|
//! | 0 | `fight` | arrival (fall, landing); attract pose (`+6` = 2) |
//! | 2 | `fight` | combat: walk (`+6` = 0, 8), about-face (2), stop (4), approach (6) |
//! | 4 | `attacks` | nine attacks (`+6` = 0 to 16) |
//! | 6, 8 | `moves` | dodges (disappearance and reappearance, backward leap, short walk) |
//! | 10, 12, 14 | here | wait with no player, return to combat, getting up |
//! | state 4 | `hit` | reactions to hits, fall, death |
//!
//! Animations: 0x17809c (movements), 0x1780ba (hits).

use crate::families::cx::Bench;
use crate::data::RomAddr;
use crate::objects::Obj;
use crate::record::record;
use super::boss_common::*;
use super::boss_common::warrior::Warrior;
use super::cx::handlers;

mod aids;
mod attacks;
mod fight;
mod hit;
mod moves;

/// Fields of this boss beyond those of `Enemy`, `Boss` and `Warrior` (`record!`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Boss7(pub Obj);

impl Boss7 {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Boss7 {
        /// Volleys still to launch (rain of projectiles).
        bursts, set_bursts, BURSTS: u8 @ 0xaa;
        /// Position of the target at the start of the plunging hit.
        target_x, set_target_x, TARGET_X: u16 @ 0xb0;
        target_y, set_target_y, TARGET_Y: u16 @ 0xb2;
    }
}
use crate::objects::pools;
use crate::objects::Current;
use crate::machine::Machine;

const SET_BODY: RomAddr = RomAddr(0x17809c);
const SET_STRIKE: RomAddr = RomAddr(0x1780ba);

/// "Waiting for a player" state (group 10).
const WAIT: u32 = 0x020a_0000;
/// Combat state (group 2, walk).
const FIGHT: u32 = 0x0202_0000;

fn body<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_BODY.0, n as u16);
}

fn strike<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_STRIKE.0, n as u16);
}

/// The leftovers D2 to D5 (re-read by the following handlers); D0, D1 and D6 are re-read by no one.
fn remainder<'a, R: Bench>(x: &'a mut Cx<'_, R>, n: usize) -> Option<&'a mut u32> {
    match n {
        2 => Some(x.c.d2()),
        3 => Some(x.c.d3()),
        4 => Some(x.c.d4()),
        5 => Some(x.c.d5()),
        _ => None,
    }
}

/// Low-order word of a data register (kept if it is a leftover).
fn setw<R: Bench>(x: &mut Cx<'_, R>, n: usize, v: u16) {
    if let Some(r) = remainder(x, n) {
        *r = *r & 0xffff_0000 | v as u32;
    }
}

/// Low-order byte of a data register (kept if it is a leftover).
fn setb<R: Bench>(x: &mut Cx<'_, R>, n: usize, v: u8) {
    if let Some(r) = remainder(x, n) {
        *r = *r & !0xff | v as u32;
    }
}

/// Animation key `kind` (`+0xa2`, copied to `+0xa3`).
fn key<R: Bench>(x: &mut Cx<'_, R>, kind: u8) {
    x.set8(Warrior::ANIM_KIND, kind);
    x.set8(Warrior::ANIM_LAST, kind);
}

/// Countdown of `+0xa0`: true when it has elapsed (gone below zero).
fn timer_done<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    sub16(x, Warrior::TIMER, 1).mi()
}

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    loop {
        x.tick_counters();
        if x.u16(f::TARGET) == 0 {
            lib::pick_target(x);
        }
        match x.u8(f::STATE) {
            0 => init(x),
            2 => active(x),
            4 => hit::run(x),
            _ => x.free(&pools::BOSS),
        }
        if x.u8(Enemy::ACTIVE) == 0 {
            return;
        }
        // slow motion: the frame is replayed one time in three (counter +0xba: 3, 2, 1, 0)
        if x.u8(Warrior::SLOW) != 0 {
            let f = sub8(x, Warrior::SLOW_TICK, 1);
            if f.eq() {
                x.set8(Warrior::REPLAY, 1);
                continue;
            }
            if f.mi() {
                x.set8(Warrior::SLOW_TICK, 3);
                x.set8(Warrior::REPLAY, 0);
                return x.reinsert();
            }
        }
        break;
    }
    hp_threshold(x);
    if x.u8(Warrior::NO_THRESHOLD) == 0 {
        let weak = x.u16(Warrior::WEAK_HP);
        let hp = x.u16(f::HP);
        if !Fl::sub16(hp, weak).gt() {
            x.set8(Warrior::SLOW, 1);
        }
    }
    x.reinsert();
}

// --- state 0: creation --------------------------------------------------------------------------

/// 0x85a80: in game, the boss falls from the top of the screen (group 0); in the attract mode, it takes the pose
/// (group 0, step 2).
fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if var::GAME_ON.get(x.b) == 0 {
        x.set32(f::STATE, 0x0200_0200);
    } else {
        x.add8(f::STATE, 2);
        x.add16(Obj::Z, 0x50);
        lib::sound(x, crate::sound::CMD_23);
    }
    var::BOSS_FIGHT.set(x.b, 1);
    lib::clear_popup(x);
    lib::empty_popups(x);
    lib::clear_all_popups(x);
    lib::create_set_15(x);
    lib::pick_target(x);
    lib::draw_stance(x);
    lib::draw_6b(x);
    x.set8(Enemy::BOX_SET, 0x27);
    lib::initial_hp_boss(x);
    lib::create_set_14(x);
    for (off, v) in [(Enemy::AI_STANCE_ROW, 0), (Enemy::EVENT_KIND, 0x23), (Enemy::EVENT_LOW, 0), (Enemy::AI_ROW_8E, 3), (Enemy::AI_SHOT, 0xff)] {
        x.set8(off, v);
    }
    var::SCENE_LEVEL.set(x.b, 0);
    for (off, v) in [(Enemy::AI_68, 8), (Enemy::AI_69, 8), (Enemy::AI_WANDER_ROW, 0), (Enemy::AI_WAIT_MASKS, 1), (Enemy::AI_WAIT_ROW, 2), (Enemy::AI_DECISION_A, 1), (Enemy::AI_DECISION_B, 1), (Enemy::AI_BOX_CHANCE, 4), (Enemy::AI_BOX_ANIM, 0x40), (Enemy::AI_BYTE_ROW, 1), (Enemy::LOOT, 0x13), (Enemy::LOOT_ROW, 0x18)] {
        x.set8(off, v);
    }
    x.set16(Warrior::SPAWN_TIME, 0xf0);
    x.set16(Warrior::IDLE_WAIT, 0x3c);
    x.set16(Warrior::IDLE_TIME, 0xf0);
    x.set8(Enemy::ACTIVE, 2);
    x.set_bit(Enemy::DISPLAY, 1, true);
    // "weakened" hit points (slow-motion threshold) according to the number of players
    let hp = x.u16(Enemy::HP_MAX);
    let (mut d1, d2) = (hp >> 1, hp >> 2);
    setw(x, 2, d2);
    let players = var::PLAYERS_ALIVE.get(x.b);
    if Fl::sub8(players, 1).gt() {
        if players == 2 {
            x.set8(Warrior::PHASES, 2);
            d1 = d1.wrapping_add(d2);
        } else {
            x.set8(Warrior::PHASES, 3);
            d1 = d1.wrapping_add(d1);
        }
    } else {
        x.set8(Warrior::PHASES, 1);
    }
    x.set16(Warrior::WEAK_HP, d1);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(Boss::DEATH_CREDIT, 1);
    x.set8(Boss::DEATH_VALUE, 0xff);
    body(x, 0);
}

// --- state 2: active -----------------------------------------------------------------------------

/// 0x85bd0: outside the game (attract), the boss keeps the pose; otherwise one group per frame, an auxiliary
/// creature (SET 0x3f) after 0xf0 frames, and a forced attack if it stays too long without fighting.
fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if var::GAME_ON.get(x.b) == 0 {
        x.set32(f::STATE, 0x0200_0200);
        x.set8(Enemy::ACTIVE, 1);
        x.set_bit(Enemy::DISPLAY, 1, false);
        lib::set_immunities(x);
        return;
    }
    if x.u16(f::TARGET) == 0 {
        lib::pick_target(x);
    }
    if x.u8(Warrior::SPAWNED) == 0 && sub16(x, Warrior::SPAWN_TIME, 1).mi() {
        if let Some(e) = x.alloc(&pools::SET) {
            e.set_u8(x.b, Enemy::ACTIVE, 1);
            e.set_u8(x.b, Enemy::VARIANT, 0x3f);
            e.set_u8(x.b, Enemy::KIND, 2);
            x.set8(Warrior::SPAWNED, 1);
        }
    }
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else if var::PAUSE.get(x.b) == 0 {
        match x.u8(f::SUB) {
            0 => fight::intro(x),
            2 => fight::fight(x),
            4 => attacks::run(x),
            6 => moves::group6(x),
            8 => moves::group8(x),
            0xa => fight::wait(x),
            0xc => x.set32(f::STATE, FIGHT),
            0xe => fight::stand_up(x),
            // the table has only these eight entries (`+5` even, from 0 to 14; see docs/code-mort.md)
            g => bad_index(g),
        }
        if (x.u8(Warrior::PHASE_TIMER) as i8) > 0 {
            sub8(x, Warrior::PHASE_TIMER, 1);
            x.set8(Enemy::FRAME_TAG, 0);
        }
        if sub16(x, Warrior::IDLE_TIME, 1).le() {
            idle_reset(x);
        } else if (x.u16(Warrior::IDLE_WAIT) as i16) < 0 && x.u16(Obj::Z) == 0 {
            idle_attack(x);
            idle_reset(x);
        }
    }
    lib::keep_in_zone(x);
    lib::collide_terrain(x);
}

fn idle_reset<R: Bench>(x: &mut Cx<'_, R>) {
    x.set16(Warrior::IDLE_WAIT, 0x3c);
    x.set16(Warrior::IDLE_TIME, 0xf0);
}

/// Too long without fighting: the boss straightens up and attacks (state 2, group 4, attack drawn from 0x85cd8).
fn idle_attack<R: Bench>(x: &mut Cx<'_, R>) {
    x.set_bit(Enemy::DISPLAY, 1, false);
    x.set16(Warrior::HITS, 0);
    x.set8(Enemy::DOWNED, 0);
    x.set8(Enemy::REACTION, 0);
    x.set8(Enemy::ACTIVE, 1);
    x.set8(Warrior::PHASE_TIMER, 0xa);
    x.set32(f::STATE, 0x0204_0000);
    let rng = x.random();
    let i = rng as u32 & 0xf;
    let attack = byte(x, "boss_7/idle_attacks", i);
    x.set8(f::SUB2, attack);
}

handlers! {
    BOSS_7 0x859f8 "boss_7" run,
}
