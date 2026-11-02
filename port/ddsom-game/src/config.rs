//! Game configuration (M2): settings kept in the EEPROM, RAM copy, default values.
//!
//! The EEPROM contains two copies of the configuration (bytes 0x00-0x2f and 0x30-0x5f), followed by
//! four bookkeeping counters (bytes 0x60-0x6f). Copy 1 carries at 0x20-0x2f a
//! signature specific to the game variant (region, `A5+0x70`); at initialisation (0x2d30a), if the
//! signature does not match, the variant's default values are rewritten.
//!
//! Bytes of the RAM copy (`CONFIG`) and the variables they feed (0x2d358):
//!
//! | Byte | Variable | Role (h) |
//! |---|---|---|
//! | 0 | `A5+0x1c` | coin mode (0-16: table of coins per credit; 17: free start; 18: free play) |
//! | 1 | `A5+0x1d` | credit mode: indexes the three tables that give `A5+0x20..0x22` |
//! | 2 | `A5+0x23` | |
//! | 3-5 | `A5+0x26..0x28` | |
//! | 6 | `A5+0x29` | |
//! | 7 | `A5+0x73` | |
//! | 8, 9 | `A5+0x84`, `A5+0x85` | |
//! | 16, 17 | `A5+0x2a`, `A5+0x2b` | |
//! | 0x60.. | `A5+0x76/0x7a/0x7e/0x7fd4` | bookkeeping counters (longs) |
//!
//! The initialisation (0x2d30a, at startup and by the startup task) reads the EEPROM; if the read
//! fails or the signature does not match the variant, the default values are rewritten,
//! otherwise the settings read are validated; then the game variables are derived from them.

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use crate::mem::{rd16, rd32, rd8, rts, wr16, wr32, wr8};
use crate::{regs, Native};

mod ram {
    /// Game variant (region), in bytes (index ×2 in the block table).
    pub const VARIANT: u32 = 0x70;
    /// RAM copy of the EEPROM (64 words).
    pub const CONFIG: u32 = 0xffff_9d8e;
    /// Coin mode, credit mode... (destinations of `load_variables`).
    pub const COIN_MODE: u32 = 0x1c;
    pub const MODE_CREDIT: u32 = 0x1d;
    pub const PIECES: u32 = 0x1e;
    pub const COUNTERS: [(u32, u32); 4] = [(0x60, 0x76), (0x64, 0x7a), (0x68, 0x7e), (0x6c, 0x7fd4)];
    /// EEPROM transfer requested: number of words minus one, start (see `eeprom.rs`).
    pub const WORDS: u32 = 0xffff_9e92;
    pub const START: u32 = 0xffff_9e96;
}

/// Distance between the configuration copy and its backup copy.
const FALLBACK: u32 = 0x30;
/// Table of default value blocks per variant (words relative to the table), in code.
const TABLE_VARIANTS: u32 = 0x2d656;
/// Loading tables: coins per credit (words) by coin mode, two bytes then one
/// byte by credit mode.
const TABLE_PIECES: u32 = 0x2d38c;
const TABLE_CREDIT_A: u32 = 0x2d40e;
const TABLE_CREDIT_B: u32 = 0x2d426;

fn a5(c: &Cpu, off: u32) -> u32 {
    c.a[5].wrapping_add(off)
}

fn set_word(r: &mut u32, v: u16) {
    *r = *r & 0xffff_0000 | v as u32;
}

// ---------------------------------------------------------------------------------------------
// 0x2d648: default value block of the variant

/// Returns D0 = displacement read from the table, A6 = default value block (blocks of 0x30 bytes:
/// values 0x00-0x11, signature 0x20-0x2f).
fn variant(c: &mut Cpu, b: &mut Cps2Bus) {
    let index = rd16(b, a5(c, ram::VARIANT));
    let displacement = crate::mem::prog16(b, TABLE_VARIANTS.wrapping_add(index as i16 as u32));
    set_word(&mut c.d[0], displacement);
    c.flags_logic(displacement as u32, Size::Word);
    c.a[6] = TABLE_VARIANTS.wrapping_add(displacement as i16 as u32);
    rts(c, b);
}

/// Default value block of the current variant.
fn block_defaults(b: &mut Cps2Bus) -> u32 {
    let index = crate::mem::rd16(b, crate::mem::A5 + ram::VARIANT);
    let displacement = crate::mem::prog16(b, TABLE_VARIANTS.wrapping_add(index as i16 as u32));
    TABLE_VARIANTS.wrapping_add(displacement as i16 as u32)
}

/// Bytes of the default block and the game variables they feed, in the order of the original's
/// comparisons (0x2d5c6).
const SETTINGS: [(u32, u32); 12] =
    [(0, 0x1c), (2, 0x23), (3, 0x26), (4, 0x27), (5, 0x28), (1, 0x1d), (6, 0x29), (7, 0x73), (8, 0x84), (9, 0x85), (0x10, 0x2a), (0x11, 0x2b)];

/// 0x2d5c6: are the current settings the factory ones (default block of the variant)?
pub fn is_factory(x: &mut crate::scene::Ctx) -> bool {
    let block = block_defaults(x.b);
    SETTINGS.iter().all(|&(byte, variable)| rd8(x.b, block + byte) == x.v8(variable))
}

// ---------------------------------------------------------------------------------------------
// 0x2d7aa: default values

/// Copies the variant's default block into two copies (copy and backup), then
/// clears the rest of the copy (the bookkeeping counters).
fn load_defaults(c: &mut Cpu, b: &mut Cps2Bus) {
    defaults(c, b);
    rts(c, b);
}

fn defaults(c: &mut Cpu, b: &mut Cps2Bus) {
    let index = rd16(b, a5(c, ram::VARIANT));
    let displacement = crate::mem::prog16(b, TABLE_VARIANTS.wrapping_add(index as i16 as u32));
    let block = TABLE_VARIANTS.wrapping_add(displacement as i16 as u32);
    let mut destination = a5(c, ram::CONFIG);
    for _ in 0..2 {
        for i in 0..24 {
            let word = rd16(b, block + 2 * i);
            wr16(b, destination, word);
            destination += 2;
        }
    }
    for i in 0..16 {
        wr16(b, destination + 2 * i, 0);
    }
    // leftovers of the original: A1 and A6 at the end of the zone, D0 = 0, D7 = 0xFFFF, flags of the last move
    c.a[1] = destination + 32;
    c.a[6] = block + 48;
    c.d[0] = 0;
    c.d[7] = 0xffff;
    c.flags_logic(0, Size::Word);
}

// ---------------------------------------------------------------------------------------------
// 0x2d662: validation and repair

/// Replaces value `i` of the copy (and of its backup) with the default value.
fn repair(b: &mut Cps2Bus, copy: u32, defaults: u32, i: u32) {
    let d = rd8(b, defaults + i);
    wr8(b, copy + i, d);
    wr8(b, copy + FALLBACK + i, d);
}

/// Checks each setting of the RAM copy against its limits; replaces the invalid values with
/// those of the variant's default block. The order of the tests and its quirks are those of the original: for example,
/// byte 4 is only examined if byte 3 was invalid.
fn validate(b: &mut Cps2Bus, c: &mut Cpu) {
    let copy = a5(c, ram::CONFIG);
    // the original starts with 0x2d648: A6 = default block of the variant
    let variant = rd16(b, a5(c, ram::VARIANT));
    let displacement = crate::mem::prog16(b, TABLE_VARIANTS.wrapping_add(variant as i16 as u32));
    set_word(&mut c.d[0], displacement);
    let defaults = TABLE_VARIANTS.wrapping_add(displacement as i16 as u32);
    c.a[6] = defaults;

    // byte 0: at most 9 (variants 4 and 10) or 18; the value 17 is tolerated for 4 and 10
    let mut cabinet = 9;
    if !(variant == 4) && !(variant == 10) {
        cabinet = 18;
    }
    let mode = rd8(b, copy);
    if !(cabinet >= mode) {
        let mut repair_0 = true;
        let mut special_variant = variant == 4;
        if !special_variant {
            special_variant = !(variant != 10);
        }
        if special_variant && (mode == 17) {
            repair_0 = false;
        }
        if repair_0 {
            repair(b, copy, defaults, 0);
        }
    }
    // byte 1: at most 11
    let o1 = rd8(b, copy + 1);
    if !(o1 <= 11) {
        repair(b, copy, defaults, 1);
    }
    // byte 2: exactly 1 if the mode (possibly repaired) is 17, otherwise 0 or 1
    let mode = rd8(b, copy);
    let o2 = rd8(b, copy + 2);
    let valid_2 = if mode == 17 { o2 == 1 } else { o2 <= 1 };
    if !valid_2 {
        repair(b, copy, defaults, 2);
    }
    // bytes 3 and 4: 0 or 1; byte 4 is only examined if byte 3 had to be repaired
    let o3 = rd8(b, copy + 3);
    if !(o3 <= 1) {
        repair(b, copy, defaults, 3);
        let o4 = rd8(b, copy + 4);
        if !(o4 <= 1) {
            repair(b, copy, defaults, 4);
        }
    }
    // byte 5: 0 or 1
    let o5 = rd8(b, copy + 5);
    if !(o5 <= 1) {
        repair(b, copy, defaults, 5);
    }
    // byte 6: at most 0, or 1 for variant 6
    let cabinet6 = if variant != 6 { 0 } else { 1 };
    let o6 = rd8(b, copy + 6);
    if !(cabinet6 >= o6) {
        repair(b, copy, defaults, 6);
    }
    // byte 7: equal to the default, or greater by at most 2 to 10
    let o7 = rd8(b, copy + 7);
    let d7 = rd8(b, defaults + 7);
    if !(o7 == d7) && !(o7.wrapping_sub(2) <= 8) {
        repair(b, copy, defaults, 7);
    }
    // byte 8: 0 or 1
    let o8 = rd8(b, copy + 8);
    if !(o8 <= 1) {
        repair(b, copy, defaults, 8);
    }
    // byte 9: between 2 and 14
    let o9 = rd8(b, copy + 9) as i8;
    if (o9 < 2) || !(o9 <= 14) {
        repair(b, copy, defaults, 9);
    }
    // bytes 16 and 17: at most 7
    for i in [16, 17] {
        let o = rd8(b, copy + i);
        if !(o <= 7) {
            repair(b, copy, defaults, i);
        }
    }
}

/// 0x2d662 called from Rust.
pub fn validate_copy(c: &mut Cpu, b: &mut Cps2Bus) {
    validate(b, c);
}

fn native_validate(c: &mut Cpu, b: &mut Cps2Bus) {
    validate(b, c);
    rts(c, b);
}

// ---------------------------------------------------------------------------------------------
// 0x2d358: from the RAM copy to the game variables

/// Copies the settings from the RAM copy to the game variables and derives the coin
/// settings from them (`A5+0x1e..0x22`). Leaves D0, D1, A4 and the flags like the original.
fn load_variables(c: &mut Cpu, b: &mut Cps2Bus) {
    variables(c, b);
    rts(c, b);
}

fn variables(c: &mut Cpu, b: &mut Cps2Bus) {
    let a4 = a5(c, ram::CONFIG);
    c.a[4] = a4;
    for k in [0x23, 0x25, 0x24] {
        wr8(b, a5(c, k), 0);
    }
    let mode = rd8(b, a4);
    wr8(b, a5(c, ram::COIN_MODE), mode);
    // coins per credit: table, or 0x0101 for modes 17 (free start) and 18 (free play)
    let (pieces, common) = match mode {
        18 => {
            wr8(b, a5(c, 0x25), 1);
            (0x0101, false)
        }
        17 => {
            wr8(b, a5(c, 0x23), 1);
            wr8(b, a5(c, 0x24), 1);
            (0x0101, true)
        }
        _ => (crate::mem::prog16(b, TABLE_PIECES + 2 * mode as u32), false),
    };
    wr16(b, a5(c, ram::PIECES), pieces);
    if !common {
        let o2 = rd8(b, a4 + 2);
        wr8(b, a5(c, 0x23), o2);
    }
    for (byte, k) in [(3, 0x26), (4, 0x27), (5, 0x28)] {
        let o = rd8(b, a4 + byte);
        wr8(b, a5(c, k), o);
    }
    let credit = rd8(b, a4 + 1);
    wr8(b, a5(c, ram::MODE_CREDIT), credit);
    let double = 2 * credit as u32;
    let a = crate::mem::prog8(b, TABLE_CREDIT_A + double);
    let a2 = crate::mem::prog8(b, TABLE_CREDIT_A + 1 + double);
    let b3 = crate::mem::prog8(b, TABLE_CREDIT_B + credit as u32);
    wr8(b, a5(c, 0x21), a);
    wr8(b, a5(c, 0x22), a2);
    wr8(b, a5(c, 0x20), b3);
    for (byte, k) in [(6, 0x29), (7, 0x73), (8, 0x84), (9, 0x85), (16, 0x2a), (17, 0x2b)] {
        let o = rd8(b, a4 + byte);
        wr8(b, a5(c, k), o);
    }
    wr8(b, a5(c, 0x72), 0);
    let last = {
        let mut last = 0;
        for (place, variable) in ram::COUNTERS {
            last = rd32(b, a4 + place);
            wr32(b, a5(c, variable), last);
        }
        last
    };
    c.d[0] = double;
    c.d[1] = credit as u32;
    c.flags_logic(last, Size::Long);
}

pub const VARIANT: Native = Native {
    entry: 0x2d648,
    name: "config_variant",
    run: crate::Body::WithCpu(variant),
    touches_sound: false,
    outputs: regs::d(0) | regs::a(6) | regs::CCR,
};

pub const DEFAULTS: Native = Native {
    entry: 0x2d7aa,
    name: "config_defaults",
    run: crate::Body::WithCpu(load_defaults),
    touches_sound: false,
    outputs: 0,
};

pub const VALIDATE: Native = Native {
    entry: 0x2d662,
    name: "config_validate",
    run: crate::Body::WithCpu(native_validate),
    touches_sound: false,
    outputs: 0,
};

pub const VARIABLES: Native = Native {
    entry: 0x2d358,
    name: "config_variables",
    run: crate::Body::WithCpu(load_variables),
    touches_sound: false,
    outputs: regs::d(0) | regs::d(1) | regs::a(4) | regs::CCR,
};

// ---------------------------------------------------------------------------------------------
// 0x2d30a: initialisation

/// Variant signature: bytes 0x20-0x2f of the default block and of copy 1.
const SIGNATURE: u32 = 0x20;
const SIGNATURE_LEN: u32 = 16;
/// "EEPROM ERROR" message (12 characters, attribute 8) displayed if writing the default values
/// fails, and the original's endless loop.
const MESSAGE_ERROR: u32 = 0x2d7fc;
const CASE_ERROR: u32 = 0x912938;
pub const BLOCKED: u32 = 0x2d7fa;

/// Compares the signature of the RAM copy with that of the variant, like the original's loop (registers
/// included: D0 last byte expected, D7 counter, A1 and A6 after the last byte examined).
fn signature_ok(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    let block = block_defaults(b);
    c.a[6] = block + SIGNATURE;
    c.a[1] = a5(c, ram::CONFIG) + SIGNATURE;
    c.d[7] = SIGNATURE_LEN - 1;
    for _ in 0..SIGNATURE_LEN {
        let expected = rd8(b, c.a[6]);
        let read_byte = rd8(b, c.a[1]);
        c.d[0] = c.d[0] & !0xff | expected as u32;
        c.a[6] += 1;
        c.a[1] += 1;
        if expected != read_byte {
            return false;
        }
        let remainder = (c.d[7] as u16).wrapping_sub(1);
        set_word(&mut c.d[7], remainder);
    }
    true
}

/// 0x2d30a: EEPROM read, default settings if needed, game variables. Returns false if writing the
/// default settings failed: the game is then blocked on "EEPROM ERROR" (the task ceded control
/// for good).
pub fn initialize(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    crate::eeprom::all_eeprom(b);
    let read_byte = crate::eeprom::read_verify(b);
    let valid = read_byte && {
        // D0 of 0x2d648 (block displacement), like the original before its loop
        let index = crate::mem::rd16(b, a5(c, ram::VARIANT));
        set_word(&mut c.d[0], crate::mem::prog16(b, TABLE_VARIANTS.wrapping_add(index as i16 as u32)));
        signature_ok(c, b)
    };
    if valid {
        validate(b, c);
        c.a[1] = a5(c, ram::CONFIG);
    } else {
        defaults(c, b);
        crate::eeprom::all_eeprom(b);
        if !crate::eeprom::write_verify(b) {
            error(b);
            return false;
        }
    }
    variables(c, b);
    true
}

fn init(c: &mut Cpu, b: &mut Cps2Bus) {
    if initialize(c, b) {
        rts(c, b);
    }
}

/// 0x2d7d2: "EEPROM ERROR" and blocking (never seen: the emulated EEPROM does not refuse the write).
fn error(b: &mut Cps2Bus) {
    for k in 0..12 {
        let ch = rd8(b, MESSAGE_ERROR + k) as u16;
        crate::mem::wr16(b, CASE_ERROR + 0x80 * k, ch);
        crate::mem::wr16(b, CASE_ERROR + 0x80 * k + 2, 8);
    }
    crate::kernel::block(b, BLOCKED);
}

pub const INIT: Native = Native {
    entry: 0x2d30a,
    name: "config_init",
    run: crate::Body::WithCpu(init),
    touches_sound: false,
    outputs: regs::d(0) | regs::d(1) | regs::a(4) | regs::CCR,
};

// ---------------------------------------------------------------------------------------------
// TEST menu, 7. CONFIGURATION: factory settings and saving

/// 0x2d546: the variant's factory settings into the game variables.
pub fn reset_factory(x: &mut crate::scene::Ctx) {
    let block = block_defaults(x.b);
    for (byte, variable) in SETTINGS {
        let v = rd8(x.b, block + byte);
        x.set_v8(variable, v);
    }
}

/// 0x2d47c: the game variables into the two copies of the RAM copy (with the variant's
/// signature), written to the EEPROM, then reread into the variables (0x2d358). False if the write failed:
/// "EEPROM ERROR" is displayed and the PC is set on the original's endless loop.
pub fn save(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    let block = block_defaults(b);
    for copy in [a5(c, ram::CONFIG), a5(c, ram::CONFIG) + FALLBACK] {
        for (byte, variable) in SETTINGS {
            let v = rd8(b, a5(c, variable));
            crate::mem::wr8(b, copy + byte, v);
        }
        for k in 0..SIGNATURE_LEN {
            let v = rd8(b, block + SIGNATURE + k);
            crate::mem::wr8(b, copy + SIGNATURE + k, v);
        }
    }
    // the two copies: 0x30 words
    crate::mem::wr16(b, a5(c, ram::WORDS), 0x2f);
    crate::mem::wr16(b, a5(c, ram::START), 0);
    if !crate::eeprom::write_verify(b) {
        error(b);
        return false;
    }
    variables(c, b);
    true
}
