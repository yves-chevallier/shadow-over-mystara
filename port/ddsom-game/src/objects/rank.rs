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
//! The settings (+0x02..=+0x11) are copied from a 16-byte row of the table 0x1f1d8, chosen
//! by the byte `A5+0x2a`.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use super::motion::{cmp, lsl_word};
use crate::mem::{rd16, rd32, rd8, rts, set_low_byte, set_low_word, wr16, wr32, A5};
use crate::{regs, Native};

const BASE: u32 = A5 + 0x7f92;
const CURRENT: u32 = 0x00;
const MAX: u32 = 0x02;
const MIN: u32 = 0x04;
const INITIAL: u32 = 0x06;
const RAMP_DELAY: u32 = 0x0a;
const IDLE_DELAY: u32 = 0x0c;
const DROP_TABLE: u32 = 0x0e;
const TIMER: u32 = 0x12;

/// A5 variables: game in progress, game mode (number of players), settings row.
const GAME_ACTIVE: u32 = 0x30;
const PLAYERS_MODE: u32 = 0x35;
const PRESET_ROW: u32 = 0x2a;
const PRESETS: u32 = 0x1f1d8;

fn get(b: &mut Cps2Bus, off: u32) -> u16 {
    rd16(b, BASE + off)
}
fn set(b: &mut Cps2Bus, off: u32, v: u16) {
    wr16(b, BASE + off, v)
}

/// Copies the settings of row `row` of the table.
pub fn load_preset(b: &mut Cps2Bus, row: u8) {
    let src = PRESETS + 16 * row as u32;
    for i in 0..4 {
        let v = rd32(b, src + 4 * i);
        wr32(b, BASE + MAX + 4 * i, v);
    }
}

/// 0x1f238: the settings of row `A5+0x2a`, then `reset`.
pub fn load_current_preset(b: &mut Cps2Bus) {
    let row = rd8(b, A5 + PRESET_ROW);
    load_preset(b, row);
    reset(b);
}

/// Resets the countdown and the rank to their starting value.
pub fn reset(b: &mut Cps2Bus) {
    if rd8(b, A5 + GAME_ACTIVE) == 0 {
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
    if rd8(b, A5 + GAME_ACTIVE) == 0 || rd8(b, A5 + PLAYERS_MODE) == 0 {
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
    let mode = rd8(b, A5 + PLAYERS_MODE) as u32;
    let drop = rd8(b, BASE + DROP_TABLE + mode) as u16;
    let next = (get(b, CURRENT).wrapping_sub(drop) as i16).max(get(b, MIN) as i16);
    set(b, CURRENT, next as u16 & 0x1f);
}

// ---- adaptation to the 68000 registers ----

/// Registers left behind by the original routines (see `diag::dead_regs`, label `rank`).
pub const REGS_RANK: u32 = regs::d(0) | regs::d(1) | regs::a(0) | regs::a(1) | regs::a(4) | regs::CCR;

/// `reset` with the original's registers (A0 = structure).
#[track_caller]
pub fn reset_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    reset_regs(c, b);
    crate::diag::dead_regs(c, "rank", REGS_RANK);
}

/// `load_preset` of row `A5+0x2a` then `reset`, with the original's registers.
#[track_caller]
pub fn load_preset_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    load_preset_regs(c, b);
    crate::diag::dead_regs(c, "rank", REGS_RANK);
}

/// `tick` with the original's registers (D0 = rank, A0 = structure).
#[track_caller]
pub fn tick_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    tick_regs(c, b);
    crate::diag::dead_regs(c, "rank", REGS_RANK);
}

/// `raise` with the original's registers (D0 = rank).
#[track_caller]
pub fn raise_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    raise_regs(c, b);
    crate::diag::dead_regs(c, "rank", REGS_RANK);
}

/// `lower` with the original's registers (D0 = rank, D1 = drop, A1 = drop table).
#[track_caller]
pub fn lower_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    lower_regs(c, b);
    crate::diag::dead_regs(c, "rank", REGS_RANK);
}

/// 0x1f258: `reset`. Leaves A0 = structure.
fn reset_regs(c: &mut Cpu, b: &mut Cps2Bus) {
    c.a[0] = BASE;
    let active = rd8(b, A5 + GAME_ACTIVE);
    c.flags_logic(active as u32, Size::Byte);
    reset(b);
    let initial = get(b, INITIAL);
    c.flags_logic(initial as u32, Size::Word);
}

/// 0x1f1b8: loads the settings row `A5+0x2a`, then `reset`.
fn load_preset_regs(c: &mut Cpu, b: &mut Cps2Bus) {
    let row = rd8(b, A5 + PRESET_ROW);
    let off = lsl_word(c, row as u16, 4);
    c.d[0] = off as u32;
    load_preset(b, row);
    c.a[4] = PRESETS + 16 * row as u32 + 16;
    reset_regs(c, b);
}

/// 0x1f27c: `tick`, operations and flags of the original (D0 = rank, A0 = structure).
fn tick_regs(c: &mut Cpu, b: &mut Cps2Bus) {
    let active = rd8(b, A5 + GAME_ACTIVE);
    c.flags_logic(active as u32, Size::Byte);
    if active == 0 {
        return;
    }
    let mode = rd8(b, A5 + PLAYERS_MODE);
    c.flags_logic(mode as u32, Size::Byte);
    if mode == 0 {
        return;
    }
    c.a[0] = BASE;
    let timer = get(b, TIMER);
    c.flags_logic(timer as u32, Size::Word);
    if timer != 0 {
        let left = c.addsub(true, 1, timer as u32, false, Size::Word, false);
        set(b, TIMER, left as u16);
        return;
    }
    let delay = get(b, RAMP_DELAY);
    set(b, TIMER, delay);
    c.flags_logic(delay as u32, Size::Word);
    let (rank, max) = (get(b, CURRENT), get(b, MAX));
    set_low_word(&mut c.d[0], rank);
    c.flags_logic(rank as u32, Size::Word);
    cmp(c, max as u32, rank as u32, Size::Word);
    let mut d0 = rank;
    if !c.cond(7 /* eq */) {
        d0 = c.addsub(false, 1, d0 as u32, false, Size::Word, false) as u16;
        cmp(c, max as u32, d0 as u32, Size::Word);
        if !c.cond(13 /* lt */) {
            d0 = max;
            c.flags_logic(d0 as u32, Size::Word);
            set(b, TIMER, 0xffff);
            c.flags_logic(0xffff, Size::Word);
        }
    } else {
        set(b, TIMER, 0xffff);
        c.flags_logic(0xffff, Size::Word);
    }
    set_low_word(&mut c.d[0], d0);
    set(b, CURRENT, d0);
    c.flags_logic(d0 as u32, Size::Word);
    set(b, CURRENT, d0 & 0x1f);
    c.flags_logic((d0 & 0x1f) as u32, Size::Word);
}

/// 0x1f2c0: `raise`.
fn raise_regs(c: &mut Cpu, b: &mut Cps2Bus) {
    let delay = get(b, RAMP_DELAY);
    set(b, TIMER, delay);
    c.flags_logic(delay as u32, Size::Word);
    let mut d0 = get(b, CURRENT);
    c.flags_logic(d0 as u32, Size::Word);
    d0 = c.addsub(false, 1, d0 as u32, false, Size::Word, false) as u16;
    let max = get(b, MAX);
    cmp(c, max as u32, d0 as u32, Size::Word);
    if !c.cond(13 /* lt */) {
        d0 = max;
        c.flags_logic(d0 as u32, Size::Word);
    }
    d0 &= 0x1f;
    set_low_word(&mut c.d[0], d0);
    set(b, CURRENT, d0);
    c.flags_logic(d0 as u32, Size::Word);
}

/// 0x1f300: `lower`.
fn lower_regs(c: &mut Cpu, b: &mut Cps2Bus) {
    let delay = get(b, RAMP_DELAY);
    set(b, TIMER, delay);
    c.flags_logic(delay as u32, Size::Word);
    c.a[1] = BASE + DROP_TABLE;
    let mode = rd8(b, A5 + PLAYERS_MODE) as u32;
    c.d[1] = mode;
    let drop = rd8(b, BASE + DROP_TABLE + mode);
    set_low_byte(&mut c.d[1], drop);
    let mut d0 = get(b, CURRENT);
    set_low_word(&mut c.d[0], d0);
    d0 = c.addsub(true, c.d[1] & 0xffff, d0 as u32, false, Size::Word, false) as u16;
    let min = get(b, MIN);
    cmp(c, min as u32, d0 as u32, Size::Word);
    if !c.cond(12 /* ge */) {
        d0 = min;
        c.flags_logic(d0 as u32, Size::Word);
    }
    d0 &= 0x1f;
    set_low_word(&mut c.d[0], d0);
    set(b, CURRENT, d0);
    c.flags_logic(d0 as u32, Size::Word);
}

pub const RANK_RESET: Native = Native {
    entry: 0x1f258,
    name: "rank_reset",
    run: crate::Body::WithCpu(|c, b| {
        reset_regs(c, b);
        rts(c, b);
    }),
    touches_sound: false,
    outputs: regs::ALL,
};

pub const RANK_LOAD_PRESET: Native = Native {
    entry: 0x1f1b8,
    name: "rank_load_preset",
    run: crate::Body::WithCpu(|c, b| {
        load_preset_regs(c, b);
        rts(c, b);
    }),
    touches_sound: false,
    outputs: regs::ALL,
};

pub const RANK_RAISE: Native = Native {
    entry: 0x1f2c0,
    name: "rank_raise",
    run: crate::Body::WithCpu(|c, b| {
        raise_regs(c, b);
        rts(c, b);
    }),
    touches_sound: false,
    outputs: regs::ALL,
};

pub const RANK_LOWER: Native = Native {
    entry: 0x1f300,
    name: "rank_lower",
    run: crate::Body::WithCpu(|c, b| {
        lower_regs(c, b);
        rts(c, b);
    }),
    touches_sound: false,
    outputs: regs::ALL,
};

pub const RANK_TICK: Native = Native {
    entry: 0x1f27c,
    name: "rank_tick",
    run: crate::Body::WithCpu(|c, b| {
        tick_regs(c, b);
        rts(c, b);
    }),
    touches_sound: false,
    outputs: regs::ALL,
};
