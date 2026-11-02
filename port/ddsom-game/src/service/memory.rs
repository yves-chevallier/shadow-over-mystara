//! Screen 8. MEMORY CHECK of the TEST menu (`0x2ceb4`): test of each memory, then return to the menu after
//! one second.
//!
//! The test (`0x2cf26`) writes four patterns into every word of the work RAM, the GFX RAM, the two
//! Object RAM banks and the Z80 shared RAM (rereading immediately, then restoring the word),
//! tests the EEPROM (`eeprom::tester`) and compares the one-byte-in-0xfff sum of each 512 KB block of the
//! program with the header table (0xca). Each line displays "TESTING..." then "OK"; an error
//! displays "NG" and blocks the task.
//!
//! **Accepted difference**: the original runs this test with interrupts masked, in one go; it lasts about
//! twenty frames at ×16 (several seconds on the cabinet), and its duration depends on the 68000 frequency.
//! It is instantaneous in both validation machines: here in Rust, and in the original code through
//! the patch [`patch_instant_check`], which runs it without advancing time.

use super::{back_to_menu, f, get, layers_off, layers_on, next_step, print, set};
use crate::mem::{rd32, wr8};
use crate::modes::Flow;
use crate::scene::Ctx;
use cps2::Cps2Bus;
use m68k::Cpu;

/// Entry of the test in the original (called by `bsr` from step 2 of the screen).
pub const CHECK: u32 = 0x2cf26;
/// Endless loop of the original after an "NG".
pub const BLOCKED: u32 = 0x2d112;

const TITLE: u32 = 0x2d184;
const LABELS: u32 = 0x2d1a8;
/// 10-character texts (bytes in ROM).
const TESTING: u32 = 0x2d27c;
const OK: u32 = 0x2d286;
const NG: u32 = 0x2d290;
/// Countdown before returning to the menu (record field).
const COUNTDOWN: u32 = 0x14;
const COUNTDOWN_FRAMES: u8 = 0x3c;

/// Patterns written into each word (table addressed PC-relative).
const PATTERNS: u32 = 0x2d0ce;
/// Output that selects the Object RAM bank seen at 0x708000.
const OBJ_BANK: u32 = 0x8040e1;
/// Byte of the Z80 shared RAM set before and after its test.
const Z80_FLAG: u32 = 0x619ffb;
/// Expected sums of the program blocks (header words).
const ROM_SUMS: u32 = 0xca;
const ROM_BLOCK: u32 = 0x80000;
const ROM_STRIDE: u32 = 0xfff;

/// Cells of each result line.
mod cell {
    pub const WORK_RAM: u32 = 0x912ea0;
    pub const CPS_RAM: u32 = 0x912ea8;
    pub const OBJECT_RAM: u32 = 0x912eb0;
    pub const SOUND_RAM: u32 = 0x912eb8;
    pub const EEPROM: u32 = 0x912ec0;
    pub const ROM: u32 = 0x912ec8;
}

pub fn memory(x: &mut Ctx) -> Flow {
    match get(x, f::STEP) {
        0 => {
            next_step(x);
            layers_off(x);
            print(x, TITLE);
            print(x, LABELS);
            layers_on(x);
            Flow::Done
        }
        2 => {
            // the original masks interrupts around the test (trap 12): no effect on an instantaneous test
            if !check(x) {
                crate::kernel::block(x.b, BLOCKED);
                return Flow::Yield;
            }
            next_step(x);
            set(x, COUNTDOWN, COUNTDOWN_FRAMES);
            Flow::Done
        }
        _ => {
            let left = get(x, COUNTDOWN) as i8 as i16 - 1;
            set(x, COUNTDOWN, left as u8);
            if left > 0 { Flow::Done } else { back_to_menu(x) }
        }
    }
}

/// `0x2d12a`: a 10-character text on line `at`.
fn label(x: &mut Ctx, at: u32, text: u32, attr: u16) {
    for k in 0..10 {
        let ch = x.rd8(text + k) as u16;
        x.wr16(at + 0x80 * k, ch);
        x.wr16(at + 0x80 * k + 2, attr);
    }
}

/// Result of a line: "OK", or "NG" (coloured).
fn verdict(x: &mut Ctx, at: u32, ok: bool) -> bool {
    if ok {
        label(x, at, OK, 0);
    } else {
        label(x, at, NG, super::HIGHLIGHT);
    }
    ok
}

/// `0x2d096`: each word of `start..end` receives the four patterns, reread immediately (low-order byte
/// only for the Z80 RAM, `byte`), then its original value.
fn test_words(x: &mut Ctx, start: u32, end: u32, byte: bool) -> bool {
    let patterns: [u16; 4] = std::array::from_fn(|k| x.prog16(PATTERNS + 2 * k as u32));
    for a in (start..end).step_by(2) {
        let saved = x.rd16(a);
        for p in patterns {
            x.wr16(a, p);
            let ok = if byte { x.rd8(a + 1) == p as u8 } else { x.rd16(a) == p };
            if !ok {
                return false;
            }
            x.wr16(a, saved);
        }
    }
    true
}

/// `bchg #0` of the Object RAM bank output (reread as the original does).
fn toggle_bank(x: &mut Ctx) {
    let v = x.rd8(OBJ_BANK) ^ 1;
    wr8(x.b, OBJ_BANK, v);
}

/// `0x2d0d6`: one-byte-in-0xfff sum of block `k` of the program.
fn rom_sum(x: &mut Ctx, k: u32) -> u16 {
    let start = ROM_BLOCK * k;
    let mut sum = 0u32;
    let mut a = start;
    while a < start + ROM_BLOCK {
        sum = sum.wrapping_add(x.rd8(a) as u32);
        a += ROM_STRIDE;
    }
    sum as u16
}

/// A RAM test line: "TESTING...", the test, then "OK" (unless `quiet`) or "NG".
fn test_line(x: &mut Ctx, at: u32, (start, end): (u32, u32), byte: bool, quiet: bool) -> bool {
    let ok = test_words(x, start, end, byte);
    if !ok || !quiet {
        verdict(x, at, ok);
    }
    ok
}

/// `0x2cf26`: the complete test. False after an "NG" (the original stays blocked).
fn check(x: &mut Ctx) -> bool {
    label(x, cell::WORK_RAM, TESTING, 0);
    if !test_line(x, cell::WORK_RAM, (0xff_0000, 0x100_0000), false, false) {
        return false;
    }
    label(x, cell::CPS_RAM, TESTING, 0);
    if !test_line(x, cell::CPS_RAM, (0x90_0000, 0x93_0000), false, false) {
        return false;
    }
    // Object RAM: the bank seen at 0x708000 (without "OK"), then the other
    label(x, cell::OBJECT_RAM, TESTING, 0);
    if !test_line(x, cell::OBJECT_RAM, (0x70_8000, 0x70_a000), false, true) {
        return false;
    }
    toggle_bank(x);
    if !test_line(x, cell::OBJECT_RAM, (0x70_8000, 0x70_a000), false, false) {
        return false;
    }
    toggle_bank(x);
    wr8(x.b, Z80_FLAG, 0);
    label(x, cell::SOUND_RAM, TESTING, 0);
    if !test_line(x, cell::SOUND_RAM, (0x61_8000, 0x61_a000), true, false) {
        return false;
    }
    wr8(x.b, Z80_FLAG, 0x88);
    label(x, cell::EEPROM, TESTING, 0);
    let ok = crate::eeprom::tester(x.b);
    if !verdict(x, cell::EEPROM, ok) {
        return false;
    }
    for k in 0..8 {
        let at = cell::ROM + 4 * k;
        label(x, at, TESTING, 0);
        let ok = rom_sum(x, k) == x.rd16(ROM_SUMS + 2 * k);
        if !verdict(x, at, ok) {
            return false;
        }
    }
    true
}

/// Patch common to both machines (`lib.rs`, `patches`): the original test (`0x2cf26`, entered by
/// `bsr`) is run until its return by the interpreter, without advancing the machine's time.
/// After an "NG", the test loops forever: execution stops after an instruction budget and
/// the machine resumes the loop.
pub fn patch_instant_check(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    let cycles = c.cycles;
    let sp = c.a[7];
    let ret = rd32(b, sp);
    let mut budget = 200_000_000u32;
    loop {
        c.step(b);
        budget -= 1;
        if (c.pc == ret && c.a[7] == sp.wrapping_add(4)) || budget == 0 {
            break;
        }
    }
    c.cycles = cycles;
    true
}
