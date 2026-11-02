//! 93C46 serial EEPROM and bookkeeping (M2).
//!
//! The EEPROM (64 16-bit words) is driven bit by bit through port 0x804040 (bit 12: DI, bit 13:
//! CLK, bit 14: CS); its DO output is read in bit 0 of 0x804020. The game keeps a copy in RAM
//! (`CONFIG`, 64 words) and rewrites it by blocks: the write is followed by a reread into `COPY`
//! and a comparison, with 10 tries.
//!
//! The native routines emit exactly the same sequence of writes on the port (the order matters:
//! the emulated EEPROM is a state machine).
//!
//! Transfer parameters (in work RAM, in the order in which the game sets them):
//!
//! | Variable | Role |
//! |---|---|
//! | `WORDS` | number of words minus one |
//! | `START` | offset in bytes in the copy (EEPROM address = offset / 2) |
//! | `BUFFER` / `CURRENT_WORDS` / `CURRENT_START` | working copies of the transfer in progress |
//!
//! Bookkeeping counters (`compter_*`): a long counter in RAM, its image in `CONFIG`, and the
//! two-word EEPROM block where it is stored.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_X};

use crate::mem::{rd16, rd32, rd8, rts, wr16, wr32, wr8};
use crate::{regs, Native};

pub mod dev_card;

/// Output port (EEPROM lines) and input port (DO on bit 0).
const PORT_EXIT: u32 = 0x804040;
const PORT_ENTRY: u32 = 0x804020;
/// EEPROM lines in the output word.
const DI: u16 = 0x1000;
const CLK: u16 = 0x2000;
const CS: u16 = 0x4000;

/// Command bytes sent (10 bits: a padding 0, the start bit, the 2-bit opcode, 6 address bits).
const CMD_READ: u16 = 0x180;
const CMD_CLEAR: u16 = 0x1c0;
const CMD_WRITE: u16 = 0x140;
const CMD_ALLOW: u16 = 0x130;

/// Variables, relative to A5.
mod ram {
    /// Copy of register 0x804040 outside the EEPROM lines (word) and counter bits (byte).
    pub const WORD_OUT: u32 = 0x16;
    pub const BYTE_OUT: u32 = 0x18;
    /// Non-zero during an EEPROM access (the input reading does not touch it).
    pub const BUSY: u32 = 0x74;
    /// "Development card" mode: no bookkeeping.
    pub const NO_ACCOUNTING: u32 = 0x82;
    pub const BUFFER: u32 = 0xffff_9e8e;
    pub const WORDS: u32 = 0xffff_9e92;
    pub const CURRENT_WORDS: u32 = 0xffff_9e94;
    pub const START: u32 = 0xffff_9e96;
    pub const CURRENT_START: u32 = 0xffff_9e98;
    pub const TESTS: u32 = 0xffff_9e9a;
    /// RAM copy of the EEPROM contents (64 words) and reread buffer.
    pub const CONFIG: u32 = 0xffff_9d8e;
    pub const COPY: u32 = 0xffff_9e0e;
}

/// Write / read tries.
const TESTS: u8 = 10;
/// Maximum duration of the wait for end of write (iterations).
const WAIT_MAX: u32 = 0x2ffff;

/// Address `A5 + off` (A5 is always `mem::A5`).
const fn a5(off: u32) -> u32 {
    crate::mem::A5.wrapping_add(off)
}

/// The state of the EEPROM lines, which the original keeps in D2 from one end of the transfer to the other.
struct Lines {
    state: u16,
}

impl Lines {
    /// 0x2da2c: writes the lines to the port, completed by the other output bits.
    fn write(&mut self, b: &mut Cps2Bus) {
        let mut v = self.state | rd16(b, a5(ram::WORD_OUT));
        v &= !6;
        v |= rd8(b, a5(ram::BYTE_OUT)) as u16;
        wr16(b, PORT_EXIT, v);
        self.state = v & (DI | CLK | CS);
    }

    /// 0x2da22: a clock pulse.
    fn pulse(&mut self, b: &mut Cps2Bus) {
        self.state |= CLK;
        self.write(b);
        self.state &= !CLK;
        self.write(b);
    }

    /// End of command: CS low, a write and a pulse.
    fn release(&mut self, b: &mut Cps2Bus) {
        self.state &= !CS;
        self.write(b);
        self.pulse(b);
    }

    /// 0x2d9f4: CS high, then the 10 bits of `command` (most significant first), each followed by a pulse.
    fn command(&mut self, b: &mut Cps2Bus, command: u16) {
        self.state |= CS;
        self.write(b);
        for i in (0..10).rev() {
            self.state = self.state & !DI | ((command >> i & 1) << 12);
            self.write(b);
            self.pulse(b);
        }
    }

    /// 0x2d9c0: waits for the end of a write (DO high). False if the delay is exceeded.
    fn wait(&mut self, b: &mut Cps2Bus) -> bool {
        self.state |= CS;
        self.write(b);
        let mut remaining = WAIT_MAX;
        loop {
            remaining -= 1;
            if remaining == 0 {
                return false;
            }
            self.pulse(b);
            if rd16(b, PORT_ENTRY) & 1 != 0 {
                break;
            }
        }
        self.state &= !CS;
        self.write(b);
        self.pulse(b);
        true
    }
}

/// EEPROM address (6 bits) of a byte offset in the copy.
fn address(offset: u16) -> u16 {
    (offset >> 1) & 0x3f
}

/// Parameters of the transfer in progress.
struct Transfer {
    buffer: u32,
    words: u16,
    start: u16,
}

fn transfer(b: &mut Cps2Bus) -> Transfer {
    Transfer {
        buffer: rd32(b, a5(ram::BUFFER)),
        words: rd16(b, a5(ram::CURRENT_WORDS)),
        start: rd16(b, a5(ram::CURRENT_START)),
    }
}

/// Next word of the copy, starting `start` bytes into the buffer.
fn buffer_start(t: &Transfer) -> u32 {
    t.buffer.wrapping_add(t.start as i16 as u32)
}

// ---------------------------------------------------------------------------------------------
// Reading and writing

/// 0x2d846: reads `words + 1` words from EEPROM address `start / 2` into the buffer.
fn read(b: &mut Cps2Bus) {
    wr8(b, a5(ram::BUSY), 1);
    let mut lines = Lines { state: 0 };
    let t = transfer(b);
    let mut ptr = buffer_start(&t);
    let mut cmd = address(t.start) | CMD_READ;
    for _ in 0..=t.words {
        lines.command(b, cmd);
        let mut word = 0u16;
        for _ in 0..16 {
            lines.pulse(b);
            word = word << 1 | rd16(b, PORT_ENTRY) & 1;
        }
        lines.pulse(b);
        lines.release(b);
        wr16(b, ptr, word);
        ptr = ptr.wrapping_add(2);
        cmd = cmd.wrapping_add(1);
    }
    wr8(b, a5(ram::BUSY), 0);
}

/// Result of a transfer: `true` if everything went well.
type Success = bool;

/// 0x2d90e: writes `words + 1` words of the buffer to the EEPROM (write enable, then for each
/// word: erase, wait, write, wait).
fn write(b: &mut Cps2Bus) -> Success {
    wr8(b, a5(ram::BUSY), 1);
    let mut lines = Lines { state: 0 };
    lines.command(b, CMD_ALLOW);
    lines.release(b);
    let t = transfer(b);
    let mut ptr = buffer_start(&t);
    let mut cmd = address(t.start);
    for _ in 0..=t.words {
        let cmd_clear = cmd & 0x3f | CMD_CLEAR;
        lines.command(b, cmd_clear);
        lines.pulse(b);
        lines.release(b);
        if !lines.wait(b) {
            wr8(b, a5(ram::BUSY), 0);
            return false;
        }
        let cmd_write = cmd_clear & 0x3f | CMD_WRITE;
        lines.command(b, cmd_write);
        let word = rd16(b, ptr);
        ptr = ptr.wrapping_add(2);
        for i in (0..16).rev() {
            lines.state = lines.state & !DI | ((word >> i & 1) << 12);
            lines.write(b);
            lines.pulse(b);
        }
        lines.state &= !CS;
        lines.write(b);
        lines.pulse(b);
        lines.pulse(b);
        if !lines.wait(b) {
            wr8(b, a5(ram::BUSY), 0);
            return false;
        }
        lines.state &= !CLK;
        lines.write(b);
        cmd = cmd_write.wrapping_add(1);
    }
    wr8(b, a5(ram::BUSY), 0);
    true
}

fn set_transfer(b: &mut Cps2Bus, buffer: u32) {
    wr32(b, a5(ram::BUFFER), buffer);
    let words = rd16(b, a5(ram::WORDS));
    wr16(b, a5(ram::CURRENT_WORDS), words);
    let start = rd16(b, a5(ram::START));
    wr16(b, a5(ram::CURRENT_START), start);
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

/// 0x2d8aa: writes the RAM copy to the EEPROM, rereads into `COPY` and compares; 10 tries.
pub(crate) fn write_verify(b: &mut Cps2Bus) -> Success {
    wr8(b, a5(ram::TESTS), TESTS);
    loop {
        set_transfer(b, a5(ram::CONFIG));
        write(b);
        set_transfer(b, a5(ram::COPY));
        read(b);
        let start = rd16(b, a5(ram::START)) as i16 as u32;
        let words = rd16(b, a5(ram::WORDS));
        let (config, copy) = (a5(ram::CONFIG).wrapping_add(start), a5(ram::COPY).wrapping_add(start));
        let equal = (0..=words as u32).all(|i| rd16(b, config + 2 * i) == rd16(b, copy + 2 * i));
        if equal {
            return true;
        }
        let remainder = rd8(b, a5(ram::TESTS)).wrapping_sub(1);
        wr8(b, a5(ram::TESTS), remainder);
        if remainder == 0 {
            return false;
        }
    }
}

fn native_write_verify(c: &mut Cpu, b: &mut Cps2Bus) {
    let ok = write_verify(b);
    give_back(c, b, ok);
}

/// 0x2d808: reads the EEPROM into the RAM copy until its two copies of the
/// configuration (words 0-23 and 24-47) agree; 10 tries.
pub(crate) fn read_verify(b: &mut Cps2Bus) -> Success {
    wr8(b, a5(ram::TESTS), TESTS);
    loop {
        set_transfer(b, a5(ram::CONFIG));
        read(b);
        let config = a5(ram::CONFIG);
        if (0..24).all(|i| rd16(b, config + 2 * i) == rd16(b, config + 0x30 + 2 * i)) {
            return true;
        }
        let remainder = rd8(b, a5(ram::TESTS)).wrapping_sub(1);
        wr8(b, a5(ram::TESTS), remainder);
        if remainder == 0 {
            return false;
        }
    }
}

fn native_read_verify(c: &mut Cpu, b: &mut Cps2Bus) {
    let ok = read_verify(b);
    give_back(c, b, ok);
}

// ---------------------------------------------------------------------------------------------
// EEPROM test (TEST menu, 8. MEMORY CHECK)

/// EEPROM words.
const EEPROM_WORDS: u16 = 0x40;
/// Patterns written to the whole EEPROM.
const PATTERNS: [u16; 4] = [0xffff, 0xaaaa, 0x5555, 0];

/// Sets up the transfer of the whole EEPROM (`WORDS`, `START`).
pub(crate) fn all_eeprom(b: &mut Cps2Bus) {
    wr16(b, a5(ram::WORDS), EEPROM_WORDS - 1);
    wr16(b, a5(ram::START), 0);
}

/// 0x2da90: writes `pattern` to the 64 words of the EEPROM (from `COPY`), rereads and compares.
fn test_pattern_word(b: &mut Cps2Bus, pattern: u16) -> bool {
    let copy = a5(ram::COPY);
    for i in 0..EEPROM_WORDS as u32 {
        wr16(b, copy + 2 * i, pattern);
    }
    wr32(b, a5(ram::BUFFER), copy);
    wr16(b, a5(ram::CURRENT_WORDS), EEPROM_WORDS - 1);
    wr16(b, a5(ram::CURRENT_START), 0);
    write(b);
    read(b);
    (0..EEPROM_WORDS as u32).all(|i| rd16(b, copy + 2 * i) == pattern)
}

/// 0x2da4a: reads the EEPROM into the RAM copy, tests it with four patterns, then rewrites the copy to it
/// (validated by `config::validate_copy`). False if a pattern was not reread.
pub fn tester(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    all_eeprom(b);
    read_verify(b);
    if !PATTERNS.iter().all(|&m| test_pattern_word(b, m)) {
        return false;
    }
    crate::config::validate_copy(c, b);
    all_eeprom(b);
    write_verify(b);
    true
}

// ---------------------------------------------------------------------------------------------
// Bookkeeping counters

/// A long counter in RAM, its image in `CONFIG`, and its place in the EEPROM.
struct Counter {
    /// Only if this byte (relative to A5) is non-zero.
    condition: Option<u32>,
    value: u32,
    image: u32,
    /// Offset in bytes in the EEPROM copy.
    place: u16,
}

const PIECES: Counter = Counter { condition: None, value: 0x76, image: 0xffff_9dee, place: 0x60 };
const STARTS: Counter = Counter { condition: None, value: 0x7a, image: 0xffff_9df2, place: 0x64 };
const CONTINUES: Counter =
    Counter { condition: Some(0x25), value: 0x7e, image: 0xffff_9df6, place: 0x68 };
const GAMES: Counter = Counter { condition: None, value: 0x7fd4, image: 0xffff_9dfa, place: 0x6c };

/// What a counter leaves in D0 (and the flags) of the original.
enum Count {
    /// No counting (condition false, or bookkeeping on the development card): D0 intact.
    Nothing,
    /// D0 = the new value (unchanged from its image: no write).
    Value(u32),
    /// The EEPROM was written (0x2d8aa): D0 = 0 or -1 depending on success.
    Written(Success),
}

/// Increments the counter and, if it changed from its image, writes it to the EEPROM (0x2d8aa).
fn count(b: &mut Cps2Bus, k: &Counter) -> Count {
    if let Some(cond) = k.condition {
        if rd8(b, a5(cond)) == 0 {
            return Count::Nothing;
        }
    }
    let value = rd32(b, a5(k.value)).wrapping_add(1);
    wr32(b, a5(k.value), value);
    if rd8(b, a5(ram::NO_ACCOUNTING)) != 0 {
        return Count::Nothing;
    }
    if value == rd32(b, a5(k.image)) {
        return Count::Value(value);
    }
    wr32(b, a5(k.image), value);
    wr16(b, a5(ram::WORDS), 1);
    wr16(b, a5(ram::START), k.place);
    Count::Written(write_verify(b))
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

macro_rules! native_counter {
    ($fn:ident, $k:expr) => {
        fn $fn(c: &mut Cpu, b: &mut Cps2Bus) {
            count_68k(c, b, &$k);
            rts(c, b);
        }
    };
}

native_counter!(count_coins, PIECES);
native_counter!(count_starts, STARTS);
native_counter!(count_continues, CONTINUES);
native_counter!(count_games, GAMES);

// ---------------------------------------------------------------------------------------------
// Full bookkeeping: EEPROM then extra RAM counters

/// Full bookkeeping counter (0x2e448 coins, 0x2e49a starts): the EEPROM counter,
/// then, if the extra RAM is used (`NO_ACCOUNTING` ≠ 0, development card), the three
/// copies of the counter in that RAM (`dev_card`).
struct Full {
    counter: &'static Counter,
    card: fn(&mut Cps2Bus) -> u32,
}

const FULL_COINS: Full = Full { counter: &PIECES, card: dev_card::pieces };
const FULL_STARTS: Full = Full { counter: &STARTS, card: dev_card::starts };

/// A coin (0x2e448) or a start (0x2e49a) counted: the EEPROM, then the development card RAM. Returns the
/// card's value (`None` on a cabinet).
fn full_accounting(b: &mut Cps2Bus, k: &Full) -> Option<u32> {
    count(b, k.counter);
    (rd8(b, a5(ram::NO_ACCOUNTING)) != 0).then(|| (k.card)(b))
}

/// 0x2e448: a coin counted (reading of the coin slots, VBLANK).
pub fn count_coin(b: &mut Cps2Bus) {
    full_accounting(b, &FULL_COINS);
}

/// 0x2e49a: a start counted (start button of the first coin slot).
pub fn count_start(b: &mut Cps2Bus) {
    full_accounting(b, &FULL_STARTS);
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

// ---------------------------------------------------------------------------------------------
// VBLANK

/// 0x2dcf6 (VBLANK): raises bit 0 of word 0x664000 of the extra RAM (if bookkeeping is active). Returns the
/// final word (D0.w of the original), if it was read.
pub fn vblank(b: &mut Cps2Bus) -> Option<u16> {
    if rd8(b, a5(ram::NO_ACCOUNTING)) == 0 {
        return None;
    }
    let word = rd16(b, 0x664000);
    if word & 1 == 0 {
        wr16(b, 0x664000, word | 1);
    }
    Some(word | 1)
}

fn vblank_accounting(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(word) = vblank(b) {
        c.d[0] = c.d[0] & 0xffff_0000 | word as u32;
    }
    rts(c, b);
}

macro_rules! native {
    ($ident:ident, $entry:expr, $name:expr, $run:expr, $exits:expr) => {
        pub const $ident: Native = Native {
            entry: $entry,
            name: $name,
            run: crate::Body::Processeur($run),
            touches_sound: false,
            outputs: $exits,
        };
    };
}

native!(READ, 0x2d846, "eeprom_lire", native_read, 0);
native!(WRITE, 0x2d90e, "eeprom_ecrire", native_write, regs::d(0) | regs::CCR);
native!(WRITE_VERIFY, 0x2d8aa, "eeprom_ecrire_verifier", native_write_verify, regs::d(0) | regs::CCR);
native!(READ_VERIFY, 0x2d808, "eeprom_lire_verifier", native_read_verify, regs::d(0) | regs::CCR);
native!(COUNT_COINS, 0x2daca, "compta_pieces", count_coins, regs::d(0));
native!(COUNT_STARTS, 0x2daf4, "compta_departs", count_starts, regs::d(0));
native!(COUNT_CONTINUES, 0x2db1e, "compta_continues", count_continues, regs::d(0));
native!(COUNT_GAMES, 0x2db4e, "compta_parties", count_games, regs::d(0));
native!(VBLANK_ACCOUNTING, 0x2dcf6, "compta_vblank", vblank_accounting, regs::d(0));
native!(FULL_COINS_NATIVE, 0x2e448, "compta_complete_pieces", native_count_coins, regs::CCR);
native!(FULL_STARTS_NATIVE, 0x2e49a, "compta_complete_departs", native_count_starts, regs::CCR);

// ---------------------------------------------------------------------------------------------
// Free games and development card RAM

/// Free play (A5+0x25 non-zero): the free games counter (`CONTINUES`, 0x2db1e); on the development
/// card (`NO_ACCOUNTING` non-zero: never on a cabinet), the corresponding counter of the extra RAM.
const FREE_PLAY: u32 = 0x25;

/// `tst.b (NO_ACCOUNTING,A5)`: returns true if the extra RAM is used (flags as in the original).
fn has_dev_card(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    let v = rd8(b, a5(ram::NO_ACCOUNTING));
    c.flags_logic(v as u32, Size::Byte);
    v != 0
}

/// 0x2e4ec (start of a player): in free play, the free games counter; on the development card,
/// that of its RAM.
pub fn count_game(b: &mut Cps2Bus) {
    let free = rd8(b, a5(FREE_PLAY)) != 0;
    if free {
        count(b, &CONTINUES);
    }
    if rd8(b, a5(ram::NO_ACCOUNTING)) != 0 {
        if free {
            dev_card::free_games(b);
        } else {
            dev_card::paid_games(b);
        }
    }
}

/// 0x2dc9a: check of the development card RAM.
pub fn initialize_accounting(b: &mut Cps2Bus) {
    if rd8(b, a5(ram::NO_ACCOUNTING)) != 0 {
        dev_card::start_up(b);
    }
}

/// 0x2e630: statistic of the chosen character (development card only).
pub fn stat_chosen_character(b: &mut Cps2Bus, character: u16) {
    if rd8(b, a5(ram::NO_ACCOUNTING)) != 0 {
        dev_card::character_part(b, character);
    }
}

/// 0x2e696: statistic of the duration of a game, in frames (development card only).
pub fn stat_game_duration(b: &mut Cps2Bus, images: u32) {
    if rd8(b, a5(ram::NO_ACCOUNTING)) != 0 {
        dev_card::game_duration(b, images);
    }
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

/// 0x2e8e4 (reset with the reset buttons held): reset of the extra RAM to zero.
pub fn reset_card(c: &mut Cpu, b: &mut Cps2Bus) {
    if has_dev_card(c, b) {
        dev_card::reset(b);
    }
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

pub const STAT_CHARACTER: Native = Native {
    entry: 0x2e630,
    name: "compta_stat_personnage",
    run: crate::Body::Processeur(stat_character),
    touches_sound: false,
    outputs: regs::CCR,
};

pub const STAT_DURATION: Native = Native {
    entry: 0x2e696,
    name: "compta_stat_duree",
    run: crate::Body::Processeur(stat_duration),
    touches_sound: false,
    outputs: regs::CCR,
};

pub const COUNT_GAMES_ACC: Native = Native {
    entry: 0x2e4ec,
    name: "compta_parties",
    run: crate::Body::Processeur(count_games_acc),
    touches_sound: false,
    outputs: regs::CCR,
};

pub const ACCOUNTING_INIT: Native = Native {
    entry: 0x2dc9a,
    name: "compta_init",
    run: crate::Body::Processeur(accounting_init),
    touches_sound: false,
    outputs: regs::CCR,
};
