//! Tools: the natives of `crate::objects::rank` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::objects::rank::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::tools::objects::motion::{cmp, lsl_word};
use crate::mem::{rd16, rd32, rd8, set_low_byte, set_low_word, wr16, wr32, A5};

/// `reset` with the original's registers (A0 = structure).
#[track_caller]
pub fn reset_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    reset_regs(c, b);
    crate::tools::diag::dead_regs(c, "rank", REGS_RANK);
}

/// `load_preset` of row `A5+0x2a` then `reset`, with the original's registers.
#[track_caller]
pub fn load_preset_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    load_preset_regs(c, b);
    crate::tools::diag::dead_regs(c, "rank", REGS_RANK);
}

/// `tick` with the original's registers (D0 = rank, A0 = structure).
#[track_caller]
pub fn tick_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    tick_regs(c, b);
    crate::tools::diag::dead_regs(c, "rank", REGS_RANK);
}

/// `raise` with the original's registers (D0 = rank).
#[track_caller]
pub fn raise_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    raise_regs(c, b);
    crate::tools::diag::dead_regs(c, "rank", REGS_RANK);
}

/// `lower` with the original's registers (D0 = rank, D1 = drop, A1 = drop table).
#[track_caller]
pub fn lower_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    lower_regs(c, b);
    crate::tools::diag::dead_regs(c, "rank", REGS_RANK);
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
