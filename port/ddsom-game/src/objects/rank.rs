//! Rang (difficulté dynamique, hypothèse) : un niveau de 0 à 31 qui monte avec le temps de jeu et
//! redescend quand un joueur est touché ; les ennemis le lisent (`A5+0x7f92`) pour choisir leurs
//! attaques et leurs dégâts.
//!
//! Structure à `A5 + 0x7f92` (mots) :
//!
//! | décalage | contenu |
//! |---|---|
//! | +0x00 | rang courant (0..=31) |
//! | +0x02 | rang maximal (réglage) |
//! | +0x04 | rang minimal (réglage) |
//! | +0x06 | rang initial en partie (réglage) |
//! | +0x0a | images entre deux montées (réglage) |
//! | +0x0c | images entre deux montées hors partie (0x384 par défaut) |
//! | +0x0e | octets : baisse du rang quand un joueur est touché, selon `A5+0x35` |
//! | +0x12 | compte à rebours avant la prochaine montée (0xFFFF : arrêté) |
//!
//! Les réglages (+0x02..=+0x11) sont copiés d'une ligne de 16 octets de la table 0x1f1d8, choisie
//! par l'octet `A5+0x2a`.

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

/// Variables de A5 : partie en cours, mode de jeu (nombre de joueurs), ligne de réglages.
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

/// Copie les réglages de la ligne `row` de la table.
pub fn load_preset(b: &mut Cps2Bus, row: u8) {
    let src = PRESETS + 16 * row as u32;
    for i in 0..4 {
        let v = rd32(b, src + 4 * i);
        wr32(b, BASE + MAX + 4 * i, v);
    }
}

/// 0x1f238 : les réglages de la ligne `A5+0x2a`, puis `reset`.
pub fn charger_preset(b: &mut Cps2Bus) {
    let row = rd8(b, A5 + PRESET_ROW);
    load_preset(b, row);
    reset(b);
}

/// Remet le compte à rebours et le rang à leur valeur de départ.
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

/// Un pas par image : le rang monte d'un cran toutes les `RAMP_DELAY` images jusqu'au maximum.
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

/// Monte d'un cran (au plus au maximum) et relance le compte à rebours.
pub fn raise(b: &mut Cps2Bus) {
    let delay = get(b, RAMP_DELAY);
    set(b, TIMER, delay);
    let next = (get(b, CURRENT).wrapping_add(1) as i16).min(get(b, MAX) as i16);
    set(b, CURRENT, next as u16 & 0x1f);
}

/// Baisse selon le mode de jeu (au plus jusqu'au minimum) et relance le compte à rebours.
pub fn lower(b: &mut Cps2Bus) {
    let delay = get(b, RAMP_DELAY);
    set(b, TIMER, delay);
    let mode = rd8(b, A5 + PLAYERS_MODE) as u32;
    let drop = rd8(b, BASE + DROP_TABLE + mode) as u16;
    let next = (get(b, CURRENT).wrapping_sub(drop) as i16).max(get(b, MIN) as i16);
    set(b, CURRENT, next as u16 & 0x1f);
}

// ---- adaptation aux registres du 68000 ----

/// Registres que laissent les routines d'origine (voir `diag::registres_morts`, étiquette `rank`).
pub const REGS_RANK: u32 = regs::d(0) | regs::d(1) | regs::a(0) | regs::a(1) | regs::a(4) | regs::CCR;

/// `reset` avec les registres de l'original (A0 = structure).
#[track_caller]
pub fn reset_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    reset_regs(c, b);
    crate::diag::registres_morts(c, "rank", REGS_RANK);
}

/// `load_preset` de la ligne `A5+0x2a` puis `reset`, avec les registres de l'original.
#[track_caller]
pub fn load_preset_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    load_preset_regs(c, b);
    crate::diag::registres_morts(c, "rank", REGS_RANK);
}

/// `tick` avec les registres de l'original (D0 = rang, A0 = structure).
#[track_caller]
pub fn tick_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    tick_regs(c, b);
    crate::diag::registres_morts(c, "rank", REGS_RANK);
}

/// `raise` avec les registres de l'original (D0 = rang).
#[track_caller]
pub fn raise_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    raise_regs(c, b);
    crate::diag::registres_morts(c, "rank", REGS_RANK);
}

/// `lower` avec les registres de l'original (D0 = rang, D1 = baisse, A1 = table des baisses).
#[track_caller]
pub fn lower_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    lower_regs(c, b);
    crate::diag::registres_morts(c, "rank", REGS_RANK);
}

/// 0x1f258 : `reset`. Laisse A0 = structure.
fn reset_regs(c: &mut Cpu, b: &mut Cps2Bus) {
    c.a[0] = BASE;
    let active = rd8(b, A5 + GAME_ACTIVE);
    c.flags_logic(active as u32, Size::Byte);
    reset(b);
    let initial = get(b, INITIAL);
    c.flags_logic(initial as u32, Size::Word);
}

/// 0x1f1b8 : charge la ligne de réglages `A5+0x2a`, puis `reset`.
fn load_preset_regs(c: &mut Cpu, b: &mut Cps2Bus) {
    let row = rd8(b, A5 + PRESET_ROW);
    let off = lsl_word(c, row as u16, 4);
    c.d[0] = off as u32;
    load_preset(b, row);
    c.a[4] = PRESETS + 16 * row as u32 + 16;
    reset_regs(c, b);
}

/// 0x1f27c : `tick`, opérations et drapeaux de l'original (D0 = rang, A0 = structure).
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

/// 0x1f2c0 : `raise`.
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

/// 0x1f300 : `lower`.
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
    run: crate::Corps::Processeur(|c, b| {
        reset_regs(c, b);
        rts(c, b);
    }),
    touches_sound: false,
    outputs: regs::ALL,
};

pub const RANK_LOAD_PRESET: Native = Native {
    entry: 0x1f1b8,
    name: "rank_load_preset",
    run: crate::Corps::Processeur(|c, b| {
        load_preset_regs(c, b);
        rts(c, b);
    }),
    touches_sound: false,
    outputs: regs::ALL,
};

pub const RANK_RAISE: Native = Native {
    entry: 0x1f2c0,
    name: "rank_raise",
    run: crate::Corps::Processeur(|c, b| {
        raise_regs(c, b);
        rts(c, b);
    }),
    touches_sound: false,
    outputs: regs::ALL,
};

pub const RANK_LOWER: Native = Native {
    entry: 0x1f300,
    name: "rank_lower",
    run: crate::Corps::Processeur(|c, b| {
        lower_regs(c, b);
        rts(c, b);
    }),
    touches_sound: false,
    outputs: regs::ALL,
};

pub const RANK_TICK: Native = Native {
    entry: 0x1f27c,
    name: "rank_tick",
    run: crate::Corps::Processeur(|c, b| {
        tick_regs(c, b);
        rts(c, b);
    }),
    touches_sound: false,
    outputs: regs::ALL,
};
