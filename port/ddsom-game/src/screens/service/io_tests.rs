//! Screens 1. INPUT, 2. OUTPUT and 3. SOUND & VOICE of the TEST menu.
//!
//! Each has two steps (preparation, then update every frame) and returns to the menu by
//! held 1P start + fire 1 (`exit_check`).

use crate::hw::video::{regs, Cell};
use crate::world::ram::Long;
use crate::world::screen_tables as tables;
use super::{exit_check, f, hex_digits, key, layers_off, layers_on, next_step, pressed, print, var, MENU_BLOCK};
use crate::screens::modes::Flow;
use crate::screens::scene::Ctx;

/// Players displayed according to the coin slot type (`A5+0x1d`: 0-2 two players, 3-6 three, 7+ four).
fn players(x: &mut Ctx) -> u32 {
    match var::FALLS.get(x.b) as i8 {
        c if c >= 7 => 4,
        c if c >= 3 => 3,
        _ => 2,
    }
}

// ---------------------------------------------------------------------------------------------
// 1. INPUT (0x2a4f4)

mod input {
    /// Title, help, and tables of the 2, 3 or 4 players (pointers addressed PC-relative).
    pub const TITLE: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0x2a6e4);
    pub const HELP: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0x2a726);
    pub const TABLES: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0x2a558);
    /// Cells of the "0/1" (pointer tables in ROM): service, test, coins, starts, joysticks (one table
    /// of 8 directions per player), buttons.
    pub const SERVICE: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0x2a9f4);
    pub const TEST: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0x2a9f8);
    pub const COINS: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0x2a9fc);
    pub const STARTS: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0x2aa0c);
    pub const LEVERS: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0x2a63c);
    pub const BUTTONS: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0x2aa9c);
    /// Joystick values (4 bits) for each of the 8 directions displayed.
    pub const DIRECTIONS: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0x2a64c);
}

pub fn input(x: &mut Ctx) -> Flow {
    if MENU_BLOCK.get8(x.b, f::STEP) == 0 {
        next_step(x);
        layers_off(x);
        print(x, input::TITLE.addr());
        print(x, input::HELP.addr());
        layers_on(x);
        let table = 4 * (players(x) - 2);
        let text = (input::TABLES.pc_word(x.b, table) as u32) << 16 | input::TABLES.pc_word(x.b, table + 2) as u32;
        print(x, text);
    } else {
        input_update(x);
    }
    exit_check(x)
}

/// `0x2a6b6`: coloured "1" or "0" in cell `k` of the table `cells`.
fn show_bit(x: &mut Ctx, cells: u32, k: u32, on: bool) {
    let cell = tables::long(x.b, cells + 4 * k);
    Cell(cell).put(x.b, 0x30 + on as u16, if on { super::HIGHLIGHT } else { 0 });
}

/// `0x2a564`: state of each input.
fn input_update(x: &mut Ctx) {
    let switches = var::SWITCHES.get(x.b);
    show_bit(x, input::SERVICE.addr(), 0, switches & 4 != 0);
    show_bit(x, input::TEST.addr(), 0, switches & 2 != 0);
    let n = players(x);
    let system = var::SYSTEM.get(x.b);
    for k in 0..n {
        show_bit(x, input::COINS.addr(), k, (system >> 4) >> k & 1 != 0);
    }
    for k in 0..n {
        show_bit(x, input::STARTS.addr(), k, system >> k & 1 != 0);
    }
    for p in 0..n {
        let lever = x.v8(var::INPUT + p) & 0xf;
        let cells = input::LEVERS.long(x.b, 4 * p);
        for k in 0..8 {
            let on = lever == input::DIRECTIONS.byte(x.b, k);
            show_bit(x, cells, 7 - k, on);
        }
    }
    // buttons: 4 bits per player, P1 first
    let mut buttons = 0u32;
    for p in 0..4 {
        buttons |= ((x.v8(var::INPUT + p) >> 4) as u32) << (4 * p);
    }
    for k in 0..4 * n {
        show_bit(x, input::BUTTONS.addr(), k, buttons >> k & 1 != 0);
    }
}

// ---------------------------------------------------------------------------------------------
// 2. OUTPUT (0x2aadc)

mod output {
    use crate::world::ram::Byte;
    pub const TITLE: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0x2acd2);
    pub const HELP: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0x2acf4);
    pub const LOCKS_12: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0x2ad16);
    pub const LOCKS_3: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0x2ad7a);
    pub const LOCKS_34: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0x2ad9c);
    pub const CARD: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0x2addc);
    /// Electric counter: state toggled by P1's fire 2 (A5 - 0x6294), and long reset to zero (A5 - 0x6298).
    pub const COUNTER: Byte = Byte(0xff_1d6c);
    pub const CLEARED: u32 = 0xff_1d68;
    /// Card dispenser: command (A5 - 0x5fcf).
    pub const DISPENSE: Byte = Byte(0xff_2031);
}

/// Game variants that have a card dispenser.
fn card_variant(x: &mut Ctx) -> bool {
    matches!(var::VARIANT.get(x.b), 4 | 10)
}

pub fn output(x: &mut Ctx) -> Flow {
    if MENU_BLOCK.get8(x.b, f::STEP) == 0 {
        next_step(x);
        Long(output::CLEARED).set(x.b, 0);
        output::COUNTER.set(x.b, 0);
        layers_off(x);
        print(x, output::TITLE.addr());
        print(x, output::HELP.addr());
        print(x, output::LOCKS_12.addr());
        let falls = var::FALLS.get(x.b) as i8;
        if falls > 2 {
            print(x, if falls > 6 { output::LOCKS_34.addr() } else { output::LOCKS_3.addr() });
        }
        if card_variant(x) && var::CARD_DISPENSER.get(x.b) != 0 {
            print(x, output::CARD.addr());
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
        crate::services::video::lock_counters(x.b, mask);
    } else if !cur & prev & key::SHOT1 != 0 {
        crate::services::video::unlock_counters(x.b, mask);
    }
}

/// `0x2ab72`.
fn output_update(x: &mut Ctx) {
    coin_lock(x, 0, 0x10);
    coin_lock(x, 1, 0x20);
    let falls = var::FALLS.get(x.b) as i8;
    if falls >= 3 {
        coin_lock(x, 2, 0x40);
        if falls >= 7 {
            coin_lock(x, 3, 0x80);
        }
    }
    // electric counter: P1's fire 2 toggles it at each change
    if (x.v8(var::INPUT) ^ x.v8(var::PREV)) & key::SHOT2 != 0 {
        let on = output::COUNTER.get(x.b) ^ 1;
        output::COUNTER.set(x.b, on);
        if on != 0 {
            crate::services::video::lock_counters(x.b, 1);
        } else {
            crate::services::video::unlock_counters(x.b, 1);
        }
    }
    if card_variant(x) && var::CARD_DISPENSER.get(x.b) != 0 {
        let new = x.v8(var::INPUT + 1) & !x.v8(var::PREV + 1);
        if new & key::SHOT2 != 0 {
            output::DISPENSE.set(x.b, 0x80);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 3. SOUND & VOICE (0x2b450)

mod sound {
    pub const TITLE: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0x2b618);
    pub const HELP: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0x2b7e4);
    /// Fields of the record: code (word), volume (0-39), code to redisplay.
    pub const CODE: u32 = 0x30;
    pub const VOLUME: u32 = 0x32;
    pub const REDRAW: u32 = 0x33;
    pub const REPEAT: u32 = 0x10;
    /// Largest code.
    pub const LAST: i32 = 0x4a0;
    /// Cells of the code, of the volume (two digits) and of the gauge.
    pub const CODE_CELL: u32 = crate::hw::video::TEXT.at(0xeac).0;
    pub const VOLUME_CELL: u32 = crate::hw::video::TEXT.at(0xdd8).0;
    pub const GAUGE: u32 = crate::hw::video::TEXT.at(0x1e0).0;
    pub const GAUGE_STEPS: u8 = 0x27;
    /// Code step per key: (bit of the key in P1's byte, addition to the code).
    pub const KEYS: [(u8, i16); 6] = [(6, 0x100), (7, -0x100), (0, 0x10), (1, -0x10), (2, -1), (3, 1)];
}

pub fn sound(x: &mut Ctx) -> Flow {
    if MENU_BLOCK.get8(x.b, f::STEP) == 0 {
        next_step(x);
        MENU_BLOCK.set16(x.b, sound::CODE, 0);
        MENU_BLOCK.set8(x.b, sound::REPEAT, 0);
        MENU_BLOCK.set8(x.b, sound::REDRAW, 1);
        layers_off(x);
        print(x, sound::TITLE.addr());
        print(x, sound::HELP.addr());
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
    if MENU_BLOCK.get8(x.b, sound::REDRAW) != 0 {
        MENU_BLOCK.set8(x.b, sound::REDRAW, 0);
        let code = MENU_BLOCK.get16(x.b, sound::CODE);
        hex_digits(x, sound::CODE_CELL, code as u64, 4, super::HIGHLIGHT);
    }
    volume(x);
    gauge(x);
}

/// `0x2b518`: a key changes the code, with automatic repeat (shared counter).
fn code_key(x: &mut Ctx, held: u8, new: u8, bit: u8, delta: i16) {
    if new >> bit & 1 != 0 {
        MENU_BLOCK.set8(x.b, sound::REPEAT, 0x14);
    } else {
        if held >> bit & 1 == 0 {
            return;
        }
        let left = MENU_BLOCK.get8(x.b, sound::REPEAT) as i8 as i16 - 1;
        MENU_BLOCK.set8(x.b, sound::REPEAT, left as u8);
        if left > 0 {
            return;
        }
        MENU_BLOCK.set8(x.b, sound::REPEAT, 4);
    }
    MENU_BLOCK.set8(x.b, sound::REDRAW, 1);
    let code = MENU_BLOCK.get16(x.b, sound::CODE) as i16 as i32 + delta as i32;
    let code = if code < 0 {
        sound::LAST
    } else if code as i16 as i32 > sound::LAST {
        0
    } else {
        code
    };
    MENU_BLOCK.set16(x.b, sound::CODE, code as u16);
}

/// `0x2b556`: fire 1 plays the code (if it is not zero), fire 2 cuts the sound.
fn request(x: &mut Ctx, new: u8) {
    if new & key::SHOT1 != 0 {
        let code = MENU_BLOCK.get16(x.b, sound::CODE);
        if code != 0 {
            // D2 and D3 are those left by the code keys (0x2b50e)
            crate::services::sound::emit(x.b, crate::services::sound::Command { d1: code as u32, d2: 1, d3: 3 });
        }
    } else if new & key::SHOT2 != 0 {
        crate::services::sound::service(x.b, crate::services::sound::Service::GeneralStop);
    }
}

/// `0x2b582`: volume (0 to 39) read from the port: bits 0-4 (first bit set), bits 5-12 (5 per zero bit).
fn volume(x: &mut Ctx) {
    let port = regs::VOLUME.get(x.b);
    let low = (port.trailing_zeros()).min(5) as u8;
    let high = ((port >> 5).trailing_zeros()).min(8) as u8;
    let level = sound::GAUGE_STEPS.wrapping_sub(low + 5 * high);
    MENU_BLOCK.set8(x.b, sound::VOLUME, level);
    super::decimal(x, sound::VOLUME_CELL, level as u32, 2, super::HIGHLIGHT);
}

/// `0x2b5c0`: volume gauge.
fn gauge(x: &mut Ctx) {
    let mut cell = sound::GAUGE;
    Cell(cell).put(x.b, 0x28, 0);
    Cell(cell + 0x1480).put(x.b, 0x29, 0);
    cell += 0x80;
    let level = MENU_BLOCK.get8(x.b, sound::VOLUME);
    for _ in 0..level {
        Cell(cell).put(x.b, 0x2d, 0);
        cell += 0x80;
    }
    Cell(cell).put(x.b, 0x2b, 0x13);
    cell += 0x80;
    for _ in 0..sound::GAUGE_STEPS.wrapping_sub(level) {
        Cell(cell).put(x.b, 0x20, 0x13);
        cell += 0x80;
    }
}
