//! Variants 8 to 24 of SET type 15 (see `finale.rs`). The role names are guesses.

use crate::actors::families::cx::Bench;
use super::*;
use crate::hw::mem::{rd16, rd32, rd8, wr32};
use crate::world::objects::pools;
use crate::actors::families::set::tables as set_tables;
use crate::actors::families::set::vars;
use crate::world::objects::Obj;
use crate::actors::families::set::fields::SetObj;
use crate::actors::families::set_c::fields::ScreenEffect;
use crate::actors::families::set_c::fields::RefillObj;
use crate::actors::families::set_c::fields::Conductor;

/// Players: four records of 0x200 bytes.
const PLAYERS: u32 = crate::world::ram::PLAYERS.base;
const PLAYER_STRIDE: u32 = 0x200;
/// Pause handler of the linked objects (word A5+0x14: activity bits of the background tasks).
const TASK_FLAGS: u32 = 0x14;

fn players() -> impl Iterator<Item = u32> {
    (0..4).map(|i| PLAYERS + i * PLAYER_STRIDE)
}

/// Player in game: active and main state (byte `+4`) below 4.
fn present<R: Bench>(x: &mut Cx<'_, R>, p: u32) -> bool {
    rd8(x.b, p) != 0 && (rd8(x.b, p + 4) as i8) < 4
}

/// Releases the object on the next pass: `+4 = 6` and the next three bytes at zero.
fn retire<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(Obj::STATE, 0x0600_0000);
}

/// RAM address stored on 16 bits (`movea.w`: sign extension).
fn adr16(v: u16) -> u32 {
    v as i16 as i32 as u32
}

/// Creates a SET of type 15 (variant `variant`): routine 0xce00a. Returns the object or `None`.
fn spawn_finale<R: Bench>(x: &mut Cx<'_, R>, variant: u32) -> Option<Obj> {
    crate::actors::families::creators::set_15_pure(x.b, variant as u8).ok()
}

/// Creates a SOBJ of type 6 (routine 0xddeaa), variant `variant`.
fn spawn_sobj<R: Bench>(x: &mut Cx<'_, R>, variant: u32) -> Option<Obj> {
    crate::actors::effects_aids::scenery_6(x.b, variant as u8).ok()
}

/// Creates an effect (type 0x1f of 0xb1a7a): variant, value of `+0x2f`.
fn spawn_effct<R: Bench>(x: &mut Cx<'_, R>, variant: u32, extra: u32) -> Option<Obj> {
    crate::actors::families::effects_creators::countdown(x.b, variant as u8, extra as u8)
}

/// Four-component shake (0xce24c): background and second plane camera, with copy of the
/// "previous" positions. `table`: 8-byte records, one per step of `+0x18` & 3.
fn shake4<R: Bench>(x: &mut Cx<'_, R>, table: u32) {
    let at = table + 8 * (x.u8(RefillObj::STEP) & 3) as u32;
    x.add8(RefillObj::STEP, 1);
    for (i, var) in [a5::CAM_X, a5::CAM_Y, a5::BG_X, a5::BG_Y].into_iter().enumerate() {
        let d = set_tables::word(x.b, at + 2 * i as u32);
        let v = x.a5_u16(var).wrapping_add(d);
        x.set_a5_u16(var, v);
    }
    copy_cameras(x);
}

fn copy_cameras<R: Bench>(x: &mut Cx<'_, R>) {
    for (from, to) in [(a5::CAM_X, a5::CAM_X_PREV), (a5::CAM_Y, a5::CAM_Y_PREV), (a5::BG_X, a5::BG_X_PREV), (a5::BG_Y, a5::BG_Y_PREV)] {
        let v = x.a5_u16(from);
        x.set_a5_u16(to, v);
    }
}

fn mask_tasks<R: Bench>(x: &mut Cx<'_, R>, keep: u16) {
    let v = x.a5_u16(TASK_FLAGS) & keep;
    x.set_a5_u16(TASK_FLAGS, v);
}

// --- variants 8 and 23: HP refill ----------------------------------------------------------

/// Prepares the list (player, reserve): `+0x30 + 4 i` = player's address, `+0x32 + 4 i` = HP given back
/// automatically; the base step depends on the score `A5+0x38`, increased (variant 23) by the player's `+0x64`.
fn refill_setup<R: Bench>(x: &mut Cx<'_, R>, with_player_bonus: bool) {
    let score = x.a5_u16(vars::LEVEL_SCORE) as i16;
    let mut step: u16 = 0x14;
    for (limit, add) in [(0x100, 8), (0x130, 4), (0x200, 8), (0x230, 8)] {
        if score <= limit {
            break;
        }
        step = step.wrapping_add(add);
    }
    let mut slot = 0x30;
    for p in players() {
        if present(x, p) {
            x.set16(slot, p as u16);
            if with_player_bonus {
                step = step.wrapping_add(rd16(x.b, p + 0x64));
            }
            x.set16(slot + 2, step);
            slot += 4;
        }
    }
}

/// One refill round: each player whose HP (`+0x62`) are below the maximum (`+0x60`)
/// gains one as long as their reserve is non-empty, then up to 1.25 times half the maximum. True
/// if a player received an HP.
fn refill_step<R: Bench>(x: &mut Cx<'_, R>, variant_b: bool) -> bool {
    let mut slot = 0x30;
    let mut gave = 0u8;
    let count = (x.a5_u8(vars::PLAYERS_ALIVE) as u16).wrapping_sub(1) as u32 + 1;
    for _ in 0..count {
        let p = adr16(x.u16(slot));
        let max = rd16(x.b, p + 0x60) as i16;
        let cur = rd16(x.b, p + 0x62) as i16;
        if max > cur {
            if x.u16(slot + 2) != 0 {
                gave = gave.wrapping_add(1);
                wr16(x.b, p + 0x62, cur.wrapping_add(1) as u16);
                let left = x.u16(slot + 2).wrapping_sub(1);
                x.set16(slot + 2, left);
            } else {
                let half = (max as u16) >> 1;
                let bonus = if variant_b && x.u8(RefillObj::DELAY) == 0 { 0 } else { half >> 2 };
                if (half.wrapping_add(bonus) as i16) > cur {
                    gave = gave.wrapping_add(1);
                    wr16(x.b, p + 0x62, cur.wrapping_add(1) as u16);
                }
            }
        }
        slot += 4;
    }
    gave != 0
}

/// Variant 8 (0xcf3a8): end of level, the HP climb back up.
pub fn hp_refill_a<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            next_state(x);
            refill_setup(x, false);
        }
        2 => {
            if !refill_step(x, false) {
                next_state(x);
            }
        }
        _ => {}
    }
}

/// Variant 23 (0xd0932): same with the bonus specific to each player.
pub fn hp_refill_b<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            next_state(x);
            refill_setup(x, true);
        }
        2 => {
            if !refill_step(x, true) {
                next_state(x);
            }
        }
        _ => {}
    }
}

// --- variants 9 and 11: effects attached to the players ------------------------------------------------------

/// Picks, for an effect with no player, a free character number in the `taken` mask:
/// examines the pairs (0,1), (2,3)...; the first entirely free pair gives its first, otherwise
/// the first free number. The result is marked taken.
fn free_character(taken: &mut u32) -> Option<u32> {
    let bit = |t: u32, n: u32| t >> (n & 31) & 1 != 0;
    let mut n = 0;
    for _ in 0..6 {
        if bit(*taken, n) || bit(*taken, n + 1) {
            n += 2;
        } else {
            *taken |= 1 << n;
            return Some(n);
        }
    }
    let mut n = 0;
    for _ in 0..12 {
        for _ in 0..2 {
            if !bit(*taken, n) {
                *taken |= 1 << n;
                return Some(n);
            }
            n += 1;
        }
    }
    None
}

/// Mask of the characters present (variant `+3` of each player in game).
fn characters_present<R: Bench>(x: &mut Cx<'_, R>) -> u32 {
    let mut mask = 0;
    for p in players() {
        if present(x, p) {
            mask |= 1 << (rd8(x.b, p + 3) & 31);
        }
    }
    mask
}

/// Variant 9 (0xcf476): launches six effects (one per player, then per absent character), notifies the
/// conductor (`+8 = 1`), waits for the end, then hands control back to it (`+8 = 0xff`).
pub fn spawn_player_effects<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            next_state(x);
            x.set8(RefillObj::PHASE, 0);
            player_effects(x);
        }
        2 => match x.u8(SetObj::SUBSTATE) {
            0 => {
                if x.a5_u8(vars::FADE_STEPS) != 0 {
                    return;
                }
                handover_step(x, 0)
            }
            2 => handover_step2(x),
            4 => handover_step3(x),
            _ => {}
        },
        _ => {}
    }
}

fn player_effects<R: Bench>(x: &mut Cx<'_, R>) {
    let mut taken = characters_present(x);
    let mut a6 = PLAYERS;
    for d1 in (0..=5u32).rev() {
        let here = d1 as i8 > 1 && present(x, a6);
        let Some(fx) = spawn_effct(x, 1, d1) else { break };
        if here {
            fx.set_u16(x.b, RefillObj::TARGET, a6 as u16);
            fx.set_u16(x.b, RefillObj::CREATOR, x.o.0 as u16);
        } else if let Some(n) = free_character(&mut taken) {
            fx.set_u16(x.b, RefillObj::TARGET, n as u16);
            fx.set_u16(x.b, RefillObj::CREATOR, x.o.0 as u16);
        }
        a6 += PLAYER_STRIDE;
    }
    x.set16(RefillObj::DELAY, 0x3c);
    let boss = conductor(x);
    boss.set_u8(x.b, Obj::X, 1);
}

/// First phase: after the delay `+0x42`, creates the bonus screen (type 15 variant 0x17).
fn handover_step<R: Bench>(x: &mut Cx<'_, R>, mark: u8) {
    x.add16(RefillObj::DELAY, 0xffff);
    if x.u16(RefillObj::DELAY) != 0 {
        return;
    }
    if let Some(o) = spawn_finale(x, 0x17) {
        o.set_u8(x.b, ScreenEffect::ORDER, mark);
    }
    next_substate(x);
    x.set8(RefillObj::PHASE, 1);
    x.set8(RefillObj::MARK, 6);
    x.set_a5_u8(vars::BONUS_SCREEN, 1);
}

fn handover_step2<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u8(RefillObj::MARK) as i8) >= 0 {
        return;
    }
    x.set_a5_u8(vars::BONUS_SCREEN, 0);
    next_substate(x);
    x.set16(RefillObj::DELAY, 1);
}

fn handover_step3<R: Bench>(x: &mut Cx<'_, R>) {
    x.add16(RefillObj::DELAY, 0xffff);
    if x.u16(RefillObj::DELAY) != 0 {
        return;
    }
    let boss = conductor(x);
    boss.set_u8(x.b, Obj::X, 0xff);
    x.set8(RefillObj::PHASE, 2);
    next_state(x);
}

/// Variant 11 (0xcf6e8): like 9 for state 0; then it waits for the conductor
/// to go to 2 before launching the bonus screen (`+0x42 = 1`).
pub fn handover_effects<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => spawn_player_effects(x),
        2 => match x.u8(SetObj::SUBSTATE) {
            0 => {
                if conductor(x).u8(x.b, Conductor::SIGNAL) != 2 {
                    return;
                }
                handover_step(x, 1)
            }
            2 => handover_step2(x),
            4 => handover_step3(x),
            _ => {}
        },
        _ => {}
    }
}


/// Variants 10 to 17: shakes, markers, fireworks, lightning, ranking and palette fade.
mod show;
/// Variants 18 and 19: end-of-level bonuses and countdown.
mod rewards;
/// Variants 20 to 24: credits, flicker, name entry and results.
mod screens;

pub use rewards::*;
pub use screens::*;
pub use show::*;
