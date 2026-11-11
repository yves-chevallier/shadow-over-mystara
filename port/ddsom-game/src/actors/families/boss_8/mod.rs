//! Boss of type 8 (0x87fc0), stage 64: the second black warrior ("DarkWarrior 2"). See `docs/families/boss.md`.
//!
//! Same skeleton as bosses 17, 18 and 21 (`+4` state, `+5` group, `+6` and `+7` steps, states set in one go as
//! longs, draws in ROM tables) but its reactions to hits are the common enemy routines
//! (table of longs 0x89330) and it goes into "slow motion" when weakened.
//!
//! | `+5` | File | Role |
//! |---|---|---|
//! | 0 | `fight` | arrival cutscene (delays, jump, effects) |
//! | 2 | `fight` | combat: walk (`+6` = 0, 8), about-face (2), stop (4), approach (6) |
//! | 4 | `attacks` | ten attacks (`+6` = 0 to 18) and choice of the next one |
//! | 6, 8 | `moves` | dodges (sidestep, jump, retreat) |
//! | 10, 12, 14 | here | wait with no player, standing, getting up |
//! | state 4 | `hit` | reactions to hits, death |
//!
//! Animations: 0x1790f6 (movements), 0x179116 (hits).

use crate::actors::families::cx::Bench;
use crate::world::data::RomAddr;
use crate::world::objects::Obj;
use crate::world::record::record;
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
pub struct Boss8(pub Obj);

impl Boss8 {
    pub fn at(self, off: u32) -> u32 {
        self.0.at(off)
    }
}

record! {
    impl Boss8 {
        /// A player is grabbed by the boss (h).
        grabbed, set_grabbed, GRABBED: u8 @ 0xaa;
        /// Cleared every frame, read by no code of the port: the byte of the body shape of boss 3 (`Boss3::BODY_SHAPE`),
        /// which `boss_common::push_out_of_boss` reads only for type 3 (h).
        shape, set_shape, SHAPE: u8 @ 0xa4;
    }
}
use crate::world::objects::pools;
use crate::world::objects::Current;
use crate::world::machine::Machine;

const SET_BODY: RomAddr = RomAddr(0x1790f6);
const SET_STRIKE: RomAddr = RomAddr(0x179116);

const WAIT: u32 = 0x020a_0000;

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

/// Low-order word of a data register (`move.w`, `andi.w`...), kept if it is a leftover.
fn setw<R: Bench>(x: &mut Cx<'_, R>, n: usize, v: u16) {
    if let Some(r) = remainder(x, n) {
        *r = *r & 0xffff_0000 | v as u32;
    }
}

/// Low-order byte of a data register, kept if it is a leftover.
fn setb<R: Bench>(x: &mut Cx<'_, R>, n: usize, v: u8) {
    if let Some(r) = remainder(x, n) {
        *r = *r & !0xff | v as u32;
    }
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
            st::INIT => init(x),
            st::ACTIVE => active(x),
            st::HIT => hit::run(x),
            _ => x.free(&pools::BOSS),
        }
        if x.u8(Enemy::ACTIVE) == 0 {
            return;
        }
        // slow motion: the frame is replayed one time in three (counter +0xba: 2, 1, 0)
        if x.u8(Warrior::SLOW) != 0 {
            let f = sub8(x, Warrior::SLOW_TICK, 1);
            if f.eq() {
                x.set8(Warrior::REPLAY, 1);
                continue;
            }
            if f.mi() {
                x.set8(Warrior::SLOW_TICK, 2);
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

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    var::BOSS_FIGHT.set(x.b, 1);
    var::STAGE_CLOCK.set(x.b, 0x0459_5900);
    lib::clear_popup(x);
    lib::empty_popups(x);
    lib::clear_all_popups(x);
    lib::sound(x, crate::hw::sound::CMD_23);
    lib::create_set_15(x);
    lib::pick_target(x);
    lib::draw_stance(x);
    lib::draw_6b(x);
    x.set8(Enemy::BOX_SET, 0x28);
    lib::initial_hp_boss(x);
    for (off, v) in [(Enemy::AI_STANCE_ROW, 0), (Enemy::EVENT_KIND, 0x4f), (Enemy::EVENT_LOW, 0), (Enemy::AI_ROW_8E, 3), (Enemy::AI_SHOT, 0xff)] {
        x.set8(off, v);
    }
    var::SCENE_LEVEL.set(x.b, 0);
    for (off, v) in [(Enemy::AI_68, 8), (Enemy::AI_69, 8), (Enemy::AI_WANDER_ROW, 0), (Enemy::AI_WAIT_MASKS, 1), (Enemy::AI_WAIT_ROW, 2), (Enemy::AI_DECISION_A, 1), (Enemy::AI_DECISION_B, 1), (Enemy::AI_BOX_CHANCE, 4), (Enemy::AI_BOX_ANIM, 0x40), (Enemy::AI_BYTE_ROW, 1), (Enemy::LOOT, 0x22), (Enemy::LOOT_ROW, 0x2a)] {
        x.set8(off, v);
    }
    x.set16(Warrior::SPAWN_TIME, 0xf0);
    x.set16(Warrior::IDLE_WAIT, 0x50);
    x.set16(Warrior::IDLE_TIME, 0xf0);
    x.set8(Enemy::ACTIVE, 2);
    x.set_bit(Enemy::DISPLAY, 1, true);
    let px = var::SCROLL_X.get(x.b).wrapping_add(0xc0);
    let py = var::SCROLL_Y.get(x.b).wrapping_add(0x69);
    x.set16(Obj::X, px);
    x.set16(Obj::Y, py);
    x.add16(Obj::Z, 0x50);
    x.set16(Warrior::TIMER, 0x3c);
    crate::actors::families::effects_creators::linked(x.b, x.o, crate::actors::families::effects_creators::kind::VICTORY, None);
    // "weakened" hit points (slow-motion threshold) according to the number of players
    let hp = x.u16(Enemy::HP_MAX);
    let (mut d1, d2) = (hp >> 1, hp >> 2);
    // D2.w (the quarter) stays in the register (`dispatch::Leftovers`)
    setw(x, 2, d2);
    let players = var::PLAYERS_ALIVE.get(x.b);
    if Fl::sub8(players, 1).gt() {
        if Fl::sub8(players, 2).ne() {
            x.set8(Warrior::PHASES, 3);
            d1 = d1.wrapping_add(d1);
        } else {
            x.set8(Warrior::PHASES, 2);
            d1 = d1.wrapping_add(d2);
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

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(f::TARGET) == 0 {
        lib::pick_target(x);
    }
    if tst8(x, Warrior::SPAWNED).eq() && sub16(x, Warrior::SPAWN_TIME, 1).mi() {
            if let Some(e) = x.alloc(&pools::SET) {
                e.set_u8(x.b, Enemy::ACTIVE, 1);
                e.set_u8(x.b, Enemy::VARIANT, 0x72);
                e.set_u8(x.b, Enemy::KIND, 2);
                x.set8(Warrior::SPAWNED, 1);
            }
        }
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else if var::PAUSE.get(x.b) == 0 {
        // jump table 0x881aa: eight groups
        match x.u8(f::SUB) {
            0 => fight::intro(x),
            2 => fight::fight(x),
            4 => attacks::run(x),
            6 => moves::group6(x),
            8 => moves::group8(x),
            0xa => fight::wait(x),
            0xc => x.set32(f::STATE, 0x0202_0000),
            0xe => fight::stand_up(x),
            g => bad_index(g),
        }
        if (x.u8(Warrior::PHASE_TIMER) as i8) > 0 {
            sub8(x, Warrior::PHASE_TIMER, 1);
            x.set8(Enemy::FRAME_TAG, 0);
        }
        x.set8(Boss8::SHAPE, 0);
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
    x.set16(Warrior::IDLE_TIME, 0x12c);
}

/// Too long without fighting: the boss straightens up and attacks (state 2, group 4, step drawn from 0x88294).
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
    let step = byte(x, "boss_8/idle_attacks", i);
    x.set8(f::SUB2, step);
}

handlers! {
    BOSS_8 0x87fc0 "boss_8" run,
}
