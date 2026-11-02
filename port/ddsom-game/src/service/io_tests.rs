//! Screens 1. INPUT, 2. OUTPUT and 3. SOUND & VOICE of the TEST menu.
//!
//! Each has two steps (preparation, then update every frame) and returns to the menu by
//! held 1P start + fire 1 (`exit_check`).

use super::{exit_check, f, get, get16, hex_digits, key, layers_off, layers_on, next_step, pressed, print, set, set16, var};
use crate::modes::Flow;
use crate::scene::Ctx;

/// Players displayed according to the coin slot type (`A5+0x1d`: 0-2 two players, 3-6 three, 7+ four).
fn players(x: &mut Ctx) -> u32 {
    match x.v8(var::FALLS) as i8 {
        c if c >= 7 => 4,
        c if c >= 3 => 3,
        _ => 2,
    }
}

// ---------------------------------------------------------------------------------------------
// 1. INPUT (0x2a4f4)

mod input {
    /// Title, help, and tables of the 2, 3 or 4 players (pointers addressed PC-relative).
    pub const TITLE: u32 = 0x2a6e4;
    pub const HELP: u32 = 0x2a726;
    pub const TABLES: u32 = 0x2a558;
    /// Cells of the "0/1" (pointer tables in ROM): service, test, coins, starts, joysticks (one table
    /// of 8 directions per player), buttons.
    pub const SERVICE: u32 = 0x2a9f4;
    pub const TEST: u32 = 0x2a9f8;
    pub const COINS: u32 = 0x2a9fc;
    pub const STARTS: u32 = 0x2aa0c;
    pub const LEVERS: u32 = 0x2a63c;
    pub const BUTTONS: u32 = 0x2aa9c;
    /// Joystick values (4 bits) for each of the 8 directions displayed.
    pub const DIRECTIONS: u32 = 0x2a64c;
}

pub fn input(x: &mut Ctx) -> Flow {
    if get(x, f::STEP) == 0 {
        next_step(x);
        layers_off(x);
        print(x, input::TITLE);
        print(x, input::HELP);
        layers_on(x);
        let table = 4 * (players(x) - 2);
        let text = (x.prog16(input::TABLES + table) as u32) << 16 | x.prog16(input::TABLES + table + 2) as u32;
        print(x, text);
    } else {
        input_update(x);
    }
    exit_check(x)
}

/// `0x2a6b6`: coloured "1" or "0" in cell `k` of the table `cells`.
fn show_bit(x: &mut Ctx, cells: u32, k: u32, on: bool) {
    let cell = x.rd32(cells + 4 * k);
    x.wr16(cell, 0x30 + on as u16);
    x.wr16(cell + 2, if on { super::HIGHLIGHT } else { 0 });
}

/// `0x2a564`: state of each input.
fn input_update(x: &mut Ctx) {
    let switches = x.v8(var::SWITCHES);
    show_bit(x, input::SERVICE, 0, switches & 4 != 0);
    show_bit(x, input::TEST, 0, switches & 2 != 0);
    let n = players(x);
    let system = x.v8(var::SYSTEM);
    for k in 0..n {
        show_bit(x, input::COINS, k, (system >> 4) >> k & 1 != 0);
    }
    for k in 0..n {
        show_bit(x, input::STARTS, k, system >> k & 1 != 0);
    }
    for p in 0..n {
        let lever = x.v8(var::INPUT + p) & 0xf;
        let cells = x.rd32(input::LEVERS + 4 * p);
        for k in 0..8 {
            let on = lever == x.rd8(input::DIRECTIONS + k);
            show_bit(x, cells, 7 - k, on);
        }
    }
    // buttons: 4 bits per player, P1 first
    let mut buttons = 0u32;
    for p in 0..4 {
        buttons |= ((x.v8(var::INPUT + p) >> 4) as u32) << (4 * p);
    }
    for k in 0..4 * n {
        show_bit(x, input::BUTTONS, k, buttons >> k & 1 != 0);
    }
}

// ---------------------------------------------------------------------------------------------
// 2. OUTPUT (0x2aadc)

mod output {
    pub const TITLE: u32 = 0x2acd2;
    pub const HELP: u32 = 0x2acf4;
    pub const LOCKS_12: u32 = 0x2ad16;
    pub const LOCKS_3: u32 = 0x2ad7a;
    pub const LOCKS_34: u32 = 0x2ad9c;
    pub const CARD: u32 = 0x2addc;
    /// Electric counter: state toggled by P1's fire 2 (A5 - 0x6294), and long reset to zero (A5 - 0x6298).
    pub const COUNTER: u32 = 0xff_1d6c;
    pub const CLEARED: u32 = 0xff_1d68;
    /// Card dispenser: command (A5 - 0x5fcf).
    pub const DISPENSE: u32 = 0xff_2031;
}

/// Game variants that have a card dispenser.
fn card_variant(x: &mut Ctx) -> bool {
    matches!(x.v16(var::VARIANT), 4 | 10)
}

pub fn output(x: &mut Ctx) -> Flow {
    if get(x, f::STEP) == 0 {
        next_step(x);
        x.wr16(output::CLEARED, 0);
        x.wr16(output::CLEARED + 2, 0);
        crate::mem::wr8(x.b, output::COUNTER, 0);
        layers_off(x);
        print(x, output::TITLE);
        print(x, output::HELP);
        print(x, output::LOCKS_12);
        let falls = x.v8(var::FALLS) as i8;
        if falls > 2 {
            print(x, if falls > 6 { output::LOCKS_34 } else { output::LOCKS_3 });
        }
        if card_variant(x) && x.v8(var::CARD_DISPENSER) != 0 {
            print(x, output::CARD);
        }
        layers_on(x);
    } else {
        output_update(x);
    }
    exit_check(x)
}

/// Lock of the coin slot of player `p` (bit `mask`): set when its fire 1 is pressed, cleared when it is
/// released.
fn coin_lock(x: &mut Ctx, p: u32, mask: u8) {
    let (cur, prev) = (x.v8(var::INPUT + p), x.v8(var::PREV + p));
    if cur & !prev & key::SHOT1 != 0 {
        crate::video::lock_counters(x.b, mask);
    } else if !cur & prev & key::SHOT1 != 0 {
        crate::video::unlock_counters(x.b, mask);
    }
}

/// `0x2ab72`.
fn output_update(x: &mut Ctx) {
    coin_lock(x, 0, 0x10);
    coin_lock(x, 1, 0x20);
    let falls = x.v8(var::FALLS) as i8;
    if falls >= 3 {
        coin_lock(x, 2, 0x40);
        if falls >= 7 {
            coin_lock(x, 3, 0x80);
        }
    }
    // electric counter: P1's fire 2 toggles it at each change
    if (x.v8(var::INPUT) ^ x.v8(var::PREV)) & key::SHOT2 != 0 {
        let on = crate::mem::rd8(x.b, output::COUNTER) ^ 1;
        crate::mem::wr8(x.b, output::COUNTER, on);
        if on != 0 {
            crate::video::lock_counters(x.b, 1);
        } else {
            crate::video::unlock_counters(x.b, 1);
        }
    }
    if card_variant(x) && x.v8(var::CARD_DISPENSER) != 0 {
        let new = x.v8(var::INPUT + 1) & !x.v8(var::PREV + 1);
        if new & key::SHOT2 != 0 {
            crate::mem::wr8(x.b, output::DISPENSE, 0x80);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 3. SOUND & VOICE (0x2b450)

mod sound {
    pub const TITLE: u32 = 0x2b618;
    pub const HELP: u32 = 0x2b7e4;
    /// Fields of the record: code (word), volume (0-39), code to redisplay.
    pub const CODE: u32 = 0x30;
    pub const VOLUME: u32 = 0x32;
    pub const REDRAW: u32 = 0x33;
    pub const REPEAT: u32 = 0x10;
    /// Largest code.
    pub const LAST: i32 = 0x4a0;
    /// Cells of the code, of the volume (two digits) and of the gauge.
    pub const CODE_CELL: u32 = 0x912eac;
    pub const VOLUME_CELL: u32 = 0x912dd8;
    pub const GAUGE: u32 = 0x9121e0;
    pub const GAUGE_STEPS: u8 = 0x27;
    /// QSound volume, read from the port.
    pub const VOLUME_PORT: u32 = 0x804030;
    /// Code step per key: (bit of the key in P1's byte, addition to the code).
    pub const KEYS: [(u8, i16); 6] = [(6, 0x100), (7, -0x100), (0, 0x10), (1, -0x10), (2, -1), (3, 1)];
}

pub fn sound(x: &mut Ctx) -> Flow {
    if get(x, f::STEP) == 0 {
        next_step(x);
        set16(x, sound::CODE, 0);
        set(x, sound::REPEAT, 0);
        set(x, sound::REDRAW, 1);
        layers_off(x);
        print(x, sound::TITLE);
        print(x, sound::HELP);
        layers_on(x);
    } else {
        sound_update(x);
    }
    exit_check(x)
}

/// `0x2b4a4`.
fn sound_update(x: &mut Ctx) {
    let held = x.v8(var::INPUT);
    let new = pressed(x);
    for (bit, delta) in sound::KEYS {
        code_key(x, held, new, bit, delta);
    }
    request(x, new);
    if get(x, sound::REDRAW) != 0 {
        set(x, sound::REDRAW, 0);
        let code = get16(x, sound::CODE);
        hex_digits(x, sound::CODE_CELL, code as u64, 4, super::HIGHLIGHT);
    }
    volume(x);
    gauge(x);
}

/// `0x2b518`: a key changes the code, with automatic repeat (shared counter).
fn code_key(x: &mut Ctx, held: u8, new: u8, bit: u8, delta: i16) {
    if new >> bit & 1 != 0 {
        set(x, sound::REPEAT, 0x14);
    } else {
        if held >> bit & 1 == 0 {
            return;
        }
        let left = get(x, sound::REPEAT) as i8 as i16 - 1;
        set(x, sound::REPEAT, left as u8);
        if left > 0 {
            return;
        }
        set(x, sound::REPEAT, 4);
    }
    set(x, sound::REDRAW, 1);
    let code = get16(x, sound::CODE) as i16 as i32 + delta as i32;
    let code = if code < 0 {
        sound::LAST
    } else if code as i16 as i32 > sound::LAST {
        0
    } else {
        code
    };
    set16(x, sound::CODE, code as u16);
}

/// `0x2b556`: fire 1 plays the code (if it is not zero), fire 2 cuts the sound.
fn request(x: &mut Ctx, new: u8) {
    if new & key::SHOT1 != 0 {
        let code = get16(x, sound::CODE);
        if code != 0 {
            // D2 and D3 are those left by the code keys (0x2b50e)
            crate::sound::emit(x.b, crate::sound::Command { d1: code as u32, d2: 1, d3: 3 });
        }
    } else if new & key::SHOT2 != 0 {
        crate::sound::service(x.b, crate::sound::Service::GeneralStop);
    }
}

/// `0x2b582`: volume (0 to 39) read from the port: bits 0-4 (first bit set), bits 5-12 (5 per zero bit).
fn volume(x: &mut Ctx) {
    let port = x.rd16(sound::VOLUME_PORT);
    let low = (port.trailing_zeros()).min(5) as u8;
    let high = ((port >> 5).trailing_zeros()).min(8) as u8;
    let level = sound::GAUGE_STEPS.wrapping_sub(low + 5 * high);
    set(x, sound::VOLUME, level);
    super::decimal(x, sound::VOLUME_CELL, level as u32, 2, super::HIGHLIGHT);
}

/// `0x2b5c0`: volume gauge.
fn gauge(x: &mut Ctx) {
    let mut cell = sound::GAUGE;
    crate::mem::wr32(x.b, cell, 0x28_0000);
    crate::mem::wr32(x.b, cell + 0x1480, 0x29_0000);
    cell += 0x80;
    let level = get(x, sound::VOLUME);
    for _ in 0..level {
        crate::mem::wr32(x.b, cell, 0x2d_0000);
        cell += 0x80;
    }
    crate::mem::wr32(x.b, cell, 0x2b_0013);
    cell += 0x80;
    for _ in 0..sound::GAUGE_STEPS.wrapping_sub(level) {
        crate::mem::wr32(x.b, cell, 0x20_0013);
        cell += 0x80;
    }
}
