//! Tools: the natives of `crate::eeprom` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_X};

use crate::eeprom::*;
use crate::tools::frames::*;
use crate::{regs, Native};
use crate::mem::rd8;

macro_rules! native_counter {
    ($fn:ident, $k:expr) => {
        fn $fn(c: &mut Cpu, b: &mut Cps2Bus) {
            count_68k(c, b, &$k);
            rts(c, b);
        }
    };
}

macro_rules! native {
    ($ident:ident, $entry:expr, $name:expr, $run:expr, $exits:expr) => {
        pub(crate) const $ident: Native = Native {
            entry: $entry,
            name: $name,
            run: crate::Body::WithCpu($run),
            touches_sound: false,
            outputs: $exits,
        };
    };
}

/// Result of `moveq #0/-1,D0`: D0, N, Z, V, C; X is zero in these routines (last
/// instruction touching X: an `addq` without carry).
fn give_back(c: &mut Cpu, b: &mut Cps2Bus, success: Success) {
    c.d[0] = if success { 0 } else { u32::MAX };
    c.flags_logic(c.d[0], Size::Long);
    c.sr &= !SR_X;
    rts(c, b);
}

fn native_read(c: &mut Cpu, b: &mut Cps2Bus) {
    read(b);
    rts(c, b);
}

fn native_write(c: &mut Cpu, b: &mut Cps2Bus) {
    let ok = write(b);
    give_back(c, b, ok);
}

fn native_write_verify(c: &mut Cpu, b: &mut Cps2Bus) {
    let ok = write_verify(b);
    give_back(c, b, ok);
}

fn native_read_verify(c: &mut Cpu, b: &mut Cps2Bus) {
    let ok = read_verify(b);
    give_back(c, b, ok);
}

/// `count` with the registers of the original.
fn count_68k(c: &mut Cpu, b: &mut Cps2Bus, k: &Counter) {
    match count(b, k) {
        Count::Nothing => {}
        Count::Value(v) => c.d[0] = v,
        Count::Written(ok) => {
            c.d[0] = if ok { 0 } else { u32::MAX };
            c.flags_logic(c.d[0], Size::Long);
            c.sr &= !SR_X;
        }
    }
}

/// `full_accounting` with the registers of the original (D0 of the counter, flags of the last test).
fn full_accounting_68k(c: &mut Cpu, b: &mut Cps2Bus, k: &Full) {
    count_68k(c, b, k.counter);
    let active = rd8(b, a5(ram::NO_ACCOUNTING)) != 0;
    c.flags_logic(active as u32, Size::Byte);
    if active {
        let value = (k.card)(b);
        c.flags_logic(value, Size::Long);
    }
    rts(c, b);
}

fn native_count_coins(c: &mut Cpu, b: &mut Cps2Bus) {
    full_accounting_68k(c, b, &FULL_COINS);
}

fn native_count_starts(c: &mut Cpu, b: &mut Cps2Bus) {
    full_accounting_68k(c, b, &FULL_STARTS);
}

fn vblank_accounting(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(word) = vblank(b) {
        c.d[0] = c.d[0] & 0xffff_0000 | word as u32;
    }
    rts(c, b);
}

/// `tst.b (NO_ACCOUNTING,A5)`: returns true if the extra RAM is used (flags as in the original).
fn has_dev_card(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    let v = rd8(b, a5(ram::NO_ACCOUNTING));
    c.flags_logic(v as u32, Size::Byte);
    v != 0
}

fn count_games_acc(c: &mut Cpu, b: &mut Cps2Bus) {
    let free = rd8(b, a5(FREE_PLAY)) != 0;
    if free {
        count_68k(c, b, &CONTINUES);
    }
    if has_dev_card(c, b) {
        let value = if free { dev_card::free_games(b) } else { dev_card::paid_games(b) };
        c.flags_logic(value, Size::Long);
    }
    rts(c, b);
}

/// 0x2dc9a (startup): check of the extra RAM, only on the development card.
fn accounting_init(c: &mut Cpu, b: &mut Cps2Bus) {
    if has_dev_card(c, b) {
        dev_card::start_up(b);
    }
    rts(c, b);
}

/// 0x2e630 (chosen character, D0): statistic of the development card RAM; on a cabinet, nothing.
fn stat_character(c: &mut Cpu, b: &mut Cps2Bus) {
    if has_dev_card(c, b) {
        dev_card::character_part(b, c.d[0] as u16);
    }
    rts(c, b);
}

/// 0x2e696 (game duration, D0, in frames): same.
fn stat_duration(c: &mut Cpu, b: &mut Cps2Bus) {
    if has_dev_card(c, b) {
        dev_card::game_duration(b, c.d[0]);
    }
    rts(c, b);
}

pub(crate) const STAT_CHARACTER: Native = Native {
    entry: 0x2e630,
    name: "accounting_stat_character",
    run: crate::Body::WithCpu(stat_character),
    touches_sound: false,
    outputs: regs::CCR,
};

pub(crate) const STAT_DURATION: Native = Native {
    entry: 0x2e696,
    name: "accounting_stat_duration",
    run: crate::Body::WithCpu(stat_duration),
    touches_sound: false,
    outputs: regs::CCR,
};

pub(crate) const COUNT_GAMES_ACC: Native = Native {
    entry: 0x2e4ec,
    name: "accounting_games",
    run: crate::Body::WithCpu(count_games_acc),
    touches_sound: false,
    outputs: regs::CCR,
};

pub(crate) const ACCOUNTING_INIT: Native = Native {
    entry: 0x2dc9a,
    name: "accounting_init",
    run: crate::Body::WithCpu(accounting_init),
    touches_sound: false,
    outputs: regs::CCR,
};

native!(READ, 0x2d846, "eeprom_read", native_read, 0);

native!(WRITE, 0x2d90e, "eeprom_write", native_write, regs::d(0) | regs::CCR);

native!(WRITE_VERIFY, 0x2d8aa, "eeprom_write_verify", native_write_verify, regs::d(0) | regs::CCR);

native!(READ_VERIFY, 0x2d808, "eeprom_read_verify", native_read_verify, regs::d(0) | regs::CCR);

native!(VBLANK_ACCOUNTING, 0x2dcf6, "accounting_vblank", vblank_accounting, regs::d(0));

native!(FULL_COINS_NATIVE, 0x2e448, "accounting_complete_coins", native_count_coins, regs::CCR);

native!(FULL_STARTS_NATIVE, 0x2e49a, "accounting_complete_starts", native_count_starts, regs::CCR);

native_counter!(count_coins, PIECES);

native_counter!(count_starts, STARTS);

native_counter!(count_continues, CONTINUES);

native_counter!(count_games, GAMES);

native!(COUNT_COINS, 0x2daca, "accounting_coins", count_coins, regs::d(0));

native!(COUNT_STARTS, 0x2daf4, "accounting_starts", count_starts, regs::d(0));

native!(COUNT_CONTINUES, 0x2db1e, "accounting_continues", count_continues, regs::d(0));

native!(COUNT_GAMES, 0x2db4e, "accounting_games", count_games, regs::d(0));

const GAMES: Counter = Counter { condition: None, value: 0x7fd4, image: 0xffff_9dfa, place: 0x6c };
