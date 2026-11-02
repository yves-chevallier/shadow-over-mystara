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

use crate::families::cx::Bench;
use super::boss_common::*;
use super::cx::handlers;
use crate::mem::A5;

mod aids;
mod attacks;
mod fight;
mod hit;
mod moves;

mod g {
    pub const REACTION: u32 = 0x49;
    pub const STUN: u32 = 0x56;
    pub const VULNERABLE: u32 = 0x67;
    pub const SHOT: u32 = 0x79;
    pub const HITS: u32 = 0x84;
    pub const TIMER: u32 = 0xa0;
    pub const ANIM_KIND: u32 = 0xa2;
    pub const ANIM_LAST: u32 = 0xa3;
    /// Substate and state to resume (`+0xa5`, long `+0xa6`).
    pub const RESUME_SUB: u32 = 0xa5;
    pub const RESUME_STATE: u32 = 0xa6;
    pub const GRABBED: u32 = 0xaa;
    pub const PHASE_TIMER: u32 = 0xab;
    pub const VOLLEY: u32 = 0xac;
    pub const PHASES: u32 = 0xad;
    pub const HIT_ACK: u32 = 0xae;
    pub const PICKED: u32 = 0xaf;
    pub const SPAWN_TIME: u32 = 0xb4;
    pub const WEAK_HP: u32 = 0xb6;
    pub const SPAWNED: u32 = 0xb8;
    /// Slow motion: active (`+0xb9`), step (`+0xba`), counter of the third attack (`+0xbb`).
    pub const SLOW: u32 = 0xb9;
    pub const SLOW_TICK: u32 = 0xba;
    pub const THIRD: u32 = 0xbb;
    pub const IDLE_TIME: u32 = 0xbc;
    pub const IDLE_WAIT: u32 = 0xbe;
}
use g::*;
use crate::objects::pools;
use crate::objects::Current;
use crate::machine::Machine;

const SET_BODY: u32 = 0x1790f6;
const SET_STRIKE: u32 = 0x179116;

const WAIT: u32 = 0x020a_0000;

fn body<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_BODY, n as u16);
}

fn strike<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_STRIKE, n as u16);
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

/// Jump through a relative table (`move.w (tbl,PC,D0w),D1w ; jmp (tbl,PC,D1w)`) indexed by the object's byte `idx`:
/// returns the arrival address.
fn dispatch<R: Bench>(x: &mut Cx<'_, R>, table: u32, idx: u32) -> u32 {
    let i = x.u8(idx) as u32;
    let off = x.rom16(table + i);
    table.wrapping_add(off as i16 as i32 as u32)
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
        if x.u8(0) == 0 {
            return;
        }
        // slow motion: the frame is replayed one time in three (counter +0xba: 2, 1, 0)
        if x.u8(SLOW) != 0 {
            let f = sub8(x, SLOW_TICK, 1);
            if f.eq() {
                x.set8(0x46, 1);
                continue;
            }
            if f.mi() {
                x.set8(SLOW_TICK, 2);
                x.set8(0x46, 0);
                return x.reinsert();
            }
        }
        break;
    }
    aids::phase(x);
    if x.u8(0x43) == 0 {
        let weak = x.u16(WEAK_HP);
        let hp = x.u16(f::HP);
        if !Fl::sub16(hp, weak).gt() {
            x.set8(SLOW, 1);
        }
    }
    x.reinsert();
}

// --- state 0: creation --------------------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    x.set_a5_u8(0x50, 1);
    x.set_a5_u32(0x38, 0x0459_5900);
    lib::clear_popup(x);
    lib::empty_popups(x);
    lib::clear_all_popups(x);
    lib::sound(x, crate::sound::CMD_23);
    lib::create_set_15(x);
    lib::pick_target(x);
    lib::draw_stance(x);
    lib::draw_6b(x);
    x.set8(0x2e, 0x28);
    lib::initial_hp_boss(x);
    for (off, v) in [(0x70, 0), (0x7e, 0x4f), (0x7f, 0), (0x8e, 3), (0x78, 0xff)] {
        x.set8(off, v);
    }
    var::SCENE_LEVEL.set(x.b, 0);
    for (off, v) in [(0x68, 8), (0x69, 8), (0x6a, 0), (0x6c, 1), (0x6d, 2), (0x72, 1), (0x73, 1), (0x74, 4), (0x75, 0x40), (0x76, 1), (0x81, 0x22), (0x80, 0x2a)] {
        x.set8(off, v);
    }
    x.set16(SPAWN_TIME, 0xf0);
    x.set16(IDLE_WAIT, 0x50);
    x.set16(IDLE_TIME, 0xf0);
    x.set8(0, 2);
    x.set_bit(0x1, 1, true);
    let px = var::SCROLL_X.get(x.b).wrapping_add(0xc0);
    let py = var::SCROLL_Y.get(x.b).wrapping_add(0x69);
    x.set16(0x8, px);
    x.set16(0x10, py);
    x.add16(0xc, 0x50);
    x.set16(TIMER, 0x3c);
    crate::families::effects_creators::linked(x.b, x.o, crate::families::effects_creators::kind::VICTORY, None);
    // "weakened" hit points (slow-motion threshold) according to the number of players
    let hp = x.u16(0x60);
    let (mut d1, d2) = (hp >> 1, hp >> 2);
    // D2.w (the quarter) stays in the register (`dispatch::Leftovers`)
    setw(x, 2, d2);
    let players = var::PLAYERS_ALIVE.get(x.b);
    if Fl::sub8(players, 1).gt() {
        if Fl::sub8(players, 2).ne() {
            x.set8(PHASES, 3);
            d1 = d1.wrapping_add(d1);
        } else {
            x.set8(PHASES, 2);
            d1 = d1.wrapping_add(d2);
        }
    } else {
        x.set8(PHASES, 1);
    }
    x.set16(WEAK_HP, d1);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(0x93, 1);
    x.set8(0x9a, 0xff);
    body(x, 0);
}

// --- state 2: active -----------------------------------------------------------------------------

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(f::TARGET) == 0 {
        lib::pick_target(x);
    }
    if tst8(x, SPAWNED).eq() && sub16(x, SPAWN_TIME, 1).mi() {
            if let Some(e) = x.alloc(&pools::SET) {
                e.set_u8(x.b, 0, 1);
                e.set_u8(x.b, 3, 0x72);
                e.set_u8(x.b, 2, 2);
                x.set8(SPAWNED, 1);
            }
        }
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else if var::PAUSE.get(x.b) == 0 {
        let t = dispatch(x, 0x881aa, f::SUB);
        match t {
            0x882ae => fight::intro(x),
            0x8839a => fight::fight(x),
            0x887ac => attacks::run(x),
            0x88f36 => moves::group6(x),
            0x88f78 => moves::group8(x),
            0x891e2 => fight::wait(x),
            0x882a4 => x.set32(f::STATE, 0x0202_0000),
            0x8921e => fight::stand_up(x),
            t => unreachable!("boss 8: group {t:x}"),
        }
        if (x.u8(PHASE_TIMER) as i8) > 0 {
            sub8(x, PHASE_TIMER, 1);
            x.set8(0x38, 0);
        }
        x.set8(0xa4, 0);
        if sub16(x, IDLE_TIME, 1).le() {
            idle_reset(x);
        } else if (x.u16(IDLE_WAIT) as i16) < 0 && x.u16(0xc) == 0 {
            idle_attack(x);
            idle_reset(x);
        }
    }
    lib::keep_in_zone(x);
    lib::collide_terrain(x);
}

fn idle_reset<R: Bench>(x: &mut Cx<'_, R>) {
    x.set16(IDLE_WAIT, 0x3c);
    x.set16(IDLE_TIME, 0x12c);
}

/// Too long without fighting: the boss straightens up and attacks (state 2, group 4, step drawn from 0x88294).
fn idle_attack<R: Bench>(x: &mut Cx<'_, R>) {
    x.set_bit(0x1, 1, false);
    x.set16(HITS, 0);
    x.set8(0x86, 0);
    x.set8(REACTION, 0);
    x.set8(0, 1);
    x.set8(PHASE_TIMER, 0xa);
    x.set32(f::STATE, 0x0204_0000);
    let rng = x.random();
    let i = rng as u32 & 0xf;
    let step = x.rom8(0x88294 + i);
    x.set8(f::SUB2, step);
}

handlers! {
    0x87fc0 "boss_8" run,
}
