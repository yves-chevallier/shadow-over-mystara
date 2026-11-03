//! Rank (dynamic difficulty, hypothesis): a level from 0 to 31 that rises with play time and
//! drops when a player is hit; enemies read it (`A5+0x7f92`) to choose their
//! attacks and damage.
//!
//! Structure at `A5 + 0x7f92` (words):
//!
//! | offset | content |
//! |---|---|
//! | +0x00 | current rank (0..=31) |
//! | +0x02 | maximum rank (setting) |
//! | +0x04 | minimum rank (setting) |
//! | +0x06 | initial rank in game (setting) |
//! | +0x0a | frames between two rises (setting) |
//! | +0x0c | frames between two rises outside the game (0x384 by default) |
//! | +0x0e | bytes: rank drop when a player is hit, according to `A5+0x35` |
//! | +0x12 | countdown before the next rise (0xFFFF: stopped) |
//!
//! The settings (+0x02..=+0x11) are copied from a 16-byte row of the table `rank/presets` (0x1f1d8), chosen
//! by the byte `A5+0x2a`.

use cps2::Cps2Bus;

use crate::ram::{Byte, Long, Word};
use crate::regs;

pub(crate) const BASE: u32 = crate::ai::RANK.0;
pub(crate) const CURRENT: u32 = 0x00;
pub(crate) const MAX: u32 = 0x02;
pub(crate) const MIN: u32 = 0x04;
pub(crate) const INITIAL: u32 = 0x06;
pub(crate) const RAMP_DELAY: u32 = 0x0a;
const IDLE_DELAY: u32 = 0x0c;
pub(crate) const DROP_TABLE: u32 = 0x0e;
pub(crate) const TIMER: u32 = 0x12;

/// A5 variables: game in progress, game mode (number of players), settings row.
pub(crate) const GAME_ACTIVE: u32 = 0x30;
pub(crate) const PLAYERS_MODE: u32 = 0x35;
pub(crate) const PRESET_ROW: u32 = 0x2a;

pub(crate) fn get(b: &mut Cps2Bus, off: u32) -> u16 {
    Word(BASE + off).get(b)
}
pub(crate) fn set(b: &mut Cps2Bus, off: u32, v: u16) {
    Word(BASE + off).set(b, v)
}

/// Copies the settings of row `row` of the table `rank/presets`.
pub fn load_preset(b: &mut Cps2Bus, row: u8) {
    let presets = crate::tables::engine().get("rank/presets");
    for i in 0..4 {
        let v = presets.at32(b, 16 * row as u32 + 4 * i);
        Long(BASE + MAX + 4 * i).set(b, v);
    }
}

/// 0x1f238: the settings of row `A5+0x2a`, then `reset`.
pub fn load_current_preset(b: &mut Cps2Bus) {
    let row = Byte::a5(PRESET_ROW).get(b);
    load_preset(b, row);
    reset(b);
}

/// Resets the countdown and the rank to their starting value.
pub fn reset(b: &mut Cps2Bus) {
    if !Byte::a5(GAME_ACTIVE).is_set(b) {
        set(b, IDLE_DELAY, 0x384);
        set(b, INITIAL, 0xe);
    }
    let delay = get(b, IDLE_DELAY);
    set(b, TIMER, delay);
    let initial = get(b, INITIAL);
    set(b, CURRENT, initial);
}

/// One step per frame: the rank rises by one notch every `RAMP_DELAY` frames up to the maximum.
pub fn tick(b: &mut Cps2Bus) {
    if !Byte::a5(GAME_ACTIVE).is_set(b) || !Byte::a5(PLAYERS_MODE).is_set(b) {
        return;
    }
    let timer = get(b, TIMER);
    if timer != 0 {
        set(b, TIMER, timer - 1);
        return;
    }
    let delay = get(b, RAMP_DELAY);
    set(b, TIMER, delay);
    let (rank, max) = (get(b, CURRENT), get(b, MAX));
    let next = if rank == max {
        set(b, TIMER, 0xffff);
        rank
    } else if (rank as i16 + 1) < max as i16 {
        rank + 1
    } else {
        set(b, TIMER, 0xffff);
        max
    };
    set(b, CURRENT, next & 0x1f);
}

/// Rises by one notch (at most to the maximum) and restarts the countdown.
pub fn raise(b: &mut Cps2Bus) {
    let delay = get(b, RAMP_DELAY);
    set(b, TIMER, delay);
    let next = (get(b, CURRENT).wrapping_add(1) as i16).min(get(b, MAX) as i16);
    set(b, CURRENT, next as u16 & 0x1f);
}

/// Drops according to the game mode (at most down to the minimum) and restarts the countdown.
pub fn lower(b: &mut Cps2Bus) {
    let delay = get(b, RAMP_DELAY);
    set(b, TIMER, delay);
    let mode = Byte::a5(PLAYERS_MODE).get(b) as u32;
    let drop = Byte(BASE + DROP_TABLE + mode).get(b) as u16;
    let next = (get(b, CURRENT).wrapping_sub(drop) as i16).max(get(b, MIN) as i16);
    set(b, CURRENT, next as u16 & 0x1f);
}

// ---- adaptation to the 68000 registers ----

/// Registers left behind by the original routines (see `diag::dead_regs`, label `rank`).
pub const REGS_RANK: u32 = regs::d(0) | regs::d(1) | regs::a(0) | regs::a(1) | regs::a(4) | regs::CCR;
