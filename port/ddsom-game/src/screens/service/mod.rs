//! TEST menu (M1, `docs/service.md`): the cabinet's service menu and its test screens.
//!
//! The menu task (`service/task.rs`) calls the handler of the current state every frame:
//! preparation (0), menu (2) or chosen screen (4). The state lives in the [`MENU`] record (FF1CF8,
//! reset to zero when the task is created):
//!
//! | Byte | Role |
//! |---|---|
//! | 0 | menu state: 0 preparation, 2 menu, 4 test screen |
//! | 1 | chosen line (0, 2... 16: INPUT... EXIT) |
//! | 2 | step of the test screen (0: preparation) |
//! | 3-5 | cursors of the configuration menus |
//! | 0x10, 0x11, 0x12 | auto-repeat (up/down, left/right, fire buttons) |
//! | 0x30-0x33 | SOUND & VOICE: code (word), volume, to redisplay |
//! | 0x5e | "Secret Test" requested (if the program word 0xc0 is not zero: never) |
//!
//! The screens are Rust functions; those that cede control in the middle (the menu after the screen
//! is cleared) resume in a native placed at the original return address, the task stack containing
//! the same return addresses as the original.

pub mod io_tests;
pub mod memory;
pub mod test_pattern;
pub mod settings;
pub mod task;

use crate::hw::video::{regs, Cell, PALETTES};
use crate::world::ram::Long;
use crate::world::screen_tables::Rom;
use crate::screens::block::Block;
use crate::services::config;
use crate::services::video::Layer;
use crate::world::tasks::Flow;
use crate::world::tasks::Ctx;

/// Menu record (A5 - 0x6308), 0x60 bytes.
pub const MENU: u32 = 0xff_1cf8;
pub const MENU_LONGS: u32 = 24;

/// Fields of the record.
pub mod f {
    pub const STATE: u32 = 0;
    pub const CURSOR: u32 = 1;
    pub const STEP: u32 = 2;
    pub const REPEAT_V: u32 = 0x10;
    pub const REPEAT_H: u32 = 0x11;
    pub const REPEAT_SHOTS: u32 = 0x12;
    pub const SECRET: u32 = 0x5e;
}

/// Menu states.
mod state {
    pub const INIT: u8 = 0;
    pub const MENU: u8 = 2;
    pub const SCREEN: u8 = 4;
}

/// Work RAM variables (offsets from A5).
pub mod var {
    use crate::world::ram::{Byte, Word};
    /// Inputs: current state P1..P4, system buttons (starts, coins), switches (test, service).
    pub const INPUT: u32 = 0;
    pub const SYSTEM: Byte = Byte::a5(4);
    pub const SWITCHES: Byte = Byte::a5(5);
    pub const PREV: u32 = 6;
    pub const LAYERS: Word = Word::a5(0x14);
    pub const FLAGS: Word = Word::a5(0x12);
    pub const FALLS: Byte = Byte::a5(0x1d);
    pub const VARIANT: Word = Word::a5(0x70);
    pub const DEV_BOARD: Byte = Byte::a5(0x82);
    pub const CARD_DISPENSER: Byte = Byte::a5(0x84);
    /// TEST menu active (read by the sound).
    pub const ACTIVE: Word = Word::a5(0x7fcc);
    pub const SCROLLS: [u32; 6] = [0x4b6, 0x4ba, 0x506, 0x50a, 0x556, 0x55a];
    /// Bytes reset to zero by the menu (unknown role; `0x5bc` is the output copied to 0x8040e1 by VBLANK).
    pub const FRAME_FLAG: Byte = Byte::a5(0x5bd);
    pub const BYTE_34: Byte = Byte::a5(0x34);
}

/// Bits of a player's inputs (see `inputs.rs`).
pub mod key {
    pub const RIGHT: u8 = 0x01;
    pub const LEFT: u8 = 0x02;
    pub const DOWN: u8 = 0x04;
    pub const UP: u8 = 0x08;
    pub const SHOT1: u8 = 0x10;
    pub const SHOT2: u8 = 0x20;
    /// Start of player 1 in the system byte.
    pub const START1: u8 = 0x01;
}

/// Entry of the startup task, which the menu task becomes again on leaving.
const BOOT_TASK: crate::world::tasks::Task = crate::world::tasks::Task::Startup;

/// Menu texts (descriptors in ROM).
mod text {
    pub const MENU: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0x29f1e);
    pub const HELP: crate::world::screen_tables::Rom = crate::world::screen_tables::Rom::at(0x2a03a);
}

/// Value of the board outputs (0x400004) set by the menu.
const MENU_OUTPUTS: u16 = 0x2355;
/// Program word that enables the "Secret Test" (zero in this game).
const SECRET_ENABLE: Rom = Rom::at(0xc0);
/// Character layer: menu arrow, line "7. CONFIGURATION" coloured if the settings differ
/// from the factory settings.
const MENU_ARROW: u32 = crate::hw::video::TEXT.at(0x690).0;
const MENU_CONFIG_LINE: u32 = crate::hw::video::TEXT.at(0x940).0;
/// Highlight colour (cell attribute).
pub const HIGHLIGHT: u16 = 0x13;
/// Menu lines: the last one is EXIT.
const LAST_LINE: u8 = 0x10;

// ---------------------------------------------------------------------------------------------
// Access

/// The state block of the menu.
pub const MENU_BLOCK: Block = Block(MENU);
/// Next step of the screen.
pub fn next_step(x: &mut Ctx) {
    let s = MENU_BLOCK.get8(x.b, f::STEP).wrapping_add(2);
    MENU_BLOCK.set8(x.b, f::STEP, s);
}

/// Newly pressed P1 keys (active at 1).
pub fn pressed(x: &mut Ctx) -> u8 {
    x.v8(var::INPUT) & !x.v8(var::PREV)
}

/// Displays a text (descriptor in ROM).
pub fn print(x: &mut Ctx, descriptor: u32) {
    crate::services::text::print(x.b, descriptor);
}
/// Screen layers cut off / restored (mask 0x12ce).
pub const ALL_LAYERS: u16 = 0x12ce;
pub fn layers_off(x: &mut Ctx) {
    crate::services::video::turn_layers_off(x.b, ALL_LAYERS);
}
pub fn layers_on(x: &mut Ctx) {
    crate::services::video::turn_layers_on(x.b, ALL_LAYERS);
}

/// Objects and layers cleared, scene palettes reloaded (0xc58, 0xca8, 0xcc8, 0xce8, 0x1a296). Returns the D0 left
/// by the palette loading (reread by the configuration menu).
pub fn clear_screen(x: &mut Ctx) -> u32 {
    clear_layers(x);
    crate::services::palette::load_all_palettes(x.b)
}

/// Objects and layers cleared (0xc58, 0xca8, 0xcc8, 0xce8).
fn clear_layers(x: &mut Ctx) {
    crate::services::video::clear_objects(x.b);
    for layer in [Layer::One, Layer::Two, Layer::Three] {
        crate::services::video::clear_layer(x.b, layer);
    }
}

// ---------------------------------------------------------------------------------------------
// Common helpers (0x29948-0x29de0)

/// Auto-repeating setting (`0x29948` vertical: up goes back; `0x29994` horizontal: left
/// goes back; `0x2ca5e` fire buttons: fire 2 goes back, except with fire 1). `value` advances by `step` in `0..=max`,
/// wrapping around; a newly pressed key acts immediately then after 30 frames, a held key every
/// 8 frames (`counter` of the record). Returns the new value if the key acted.
fn repeat(x: &mut Ctx, value: u8, step: u8, max: u8, keys: u8, back: fn(u8) -> bool, counter: u32) -> Option<u8> {
    let held = x.v8(var::INPUT) & keys;
    if held == 0 {
        return None;
    }
    let new = !x.v8(var::PREV) & keys & held;
    let dir = if new != 0 {
        MENU_BLOCK.set8(x.b, counter, 0x1e);
        new
    } else {
        let left = (MENU_BLOCK.get8(x.b, counter) as i8 as i16) - 1;
        MENU_BLOCK.set8(x.b, counter, left as u8);
        if left > 0 {
            return None;
        }
        MENU_BLOCK.set8(x.b, counter, 8);
        held
    };
    let v = if back(dir) {
        let v = value as i8 as i16 - step as i8 as i16;
        if v >= 0 { v as u8 } else { max }
    } else {
        let v = value.wrapping_add(step);
        if (max as i8) >= (v as i8) { v } else { 0 }
    };
    Some(v)
}

pub fn vertical(x: &mut Ctx, value: u8, step: u8, max: u8) -> Option<u8> {
    repeat(x, value, step, max, key::UP | key::DOWN, |d| d & key::UP != 0, f::REPEAT_V)
}
pub fn horizontal(x: &mut Ctx, value: u8, step: u8, max: u8) -> Option<u8> {
    repeat(x, value, step, max, key::RIGHT | key::LEFT, |d| d & key::LEFT != 0, f::REPEAT_H)
}
pub fn shots(x: &mut Ctx, value: u8, step: u8, max: u8) -> Option<u8> {
    repeat(x, value, step, max, key::SHOT1 | key::SHOT2, |d| d & key::SHOT1 == 0, f::REPEAT_SHOTS)
}

/// Cell of a menu line: `base` + 4 × `line` (every other menu line of the layer).
fn line_cell(base: u32, line: u8) -> u32 {
    base + 4 * line as u32
}
/// `0x299fc`: arrow (tile 0x2a) in front of the line; `0x29a10`: erased.
pub fn arrow(x: &mut Ctx, base: u32, line: u8) {
    Cell(line_cell(base, line)).put(x.b, 0x2a, 0);
}
pub fn no_arrow(x: &mut Ctx, base: u32, line: u8) {
    Cell(line_cell(base, line)).put(x.b, 0x20, 0);
}
/// Moves the arrow from one line to another.
pub fn move_arrow(x: &mut Ctx, base: u32, from: u8, to: u8) {
    no_arrow(x, base, from);
    arrow(x, base, to);
}

/// `0x299e0`: attribute `attr` on `len` + 1 cells of a line of the character layer.
pub fn paint(x: &mut Ctx, cell: u32, attr: u16, len: u32) {
    for k in 0..=len {
        Cell(cell + 0x80 * k).set_attr(x.b, attr);
    }
}

/// `0x29a7a`: full brightness for the four palette pages (0x91c000-0x91cfff).
pub fn full_brightness(x: &mut Ctx) {
    for i in (0..0x1000).step_by(4) {
        let a = PALETTES.offset(i);
        let v = a.long(x.b) | 0xf000_f000;
        a.set_long(x.b, v);
    }
}

/// Decimal conversion buffer (A5 - 0x62a8, 8 BCD bytes).
const BCD: Long = Long(0xff_1d58);

/// `0x29da4`: `value` in binary-coded decimal, in the buffer.
fn to_bcd(x: &mut Ctx, value: u32) -> u64 {
    let mut bcd = 0u64;
    let mut v = value;
    for shift in 0.. {
        if v == 0 {
            break;
        }
        bcd |= ((v % 10) as u64) << (4 * shift);
        v /= 10;
    }
    BCD.set(x.b, (bcd >> 32) as u32);
    Long(BCD.0 + 4).set(x.b, bcd as u32);
    bcd
}

/// `0x29d88`: the `n` low-order hexadecimal digits of `value` (tile = digit), from left to
/// right, attribute `attr`. Returns the cell that follows.
pub fn hex_digits(x: &mut Ctx, mut cell: u32, value: u64, n: u32, attr: u16) -> u32 {
    for k in (0..n).rev() {
        Cell(cell).put(x.b, (value >> (4 * k)) as u16 & 0xf, attr);
        cell += 0x80;
    }
    cell
}

/// `0x29cf2` (8 digits), `0x29d3a` (2 digits): number in decimal.
pub fn decimal(x: &mut Ctx, cell: u32, value: u32, n: u32, attr: u16) -> u32 {
    let bcd = to_bcd(x, value);
    hex_digits(x, cell, bcd, n, attr)
}

// ---------------------------------------------------------------------------------------------
// The menu

/// One turn of the task: handler of the menu state.
pub fn handler(x: &mut Ctx) -> Flow {
    match MENU_BLOCK.get8(x.b, f::STATE) {
        state::INIT => {
            var::ACTIVE.set(x.b, 1);
            MENU_BLOCK.set8(x.b, f::STATE, state::MENU);
            show_menu(x)
        }
        state::MENU => menu(x),
        _ => screen(x),
    }
}

/// `0x291a0`: screen made new again (layers cut, VRAM cleared, original palettes at full brightness),
/// a waiting frame, then the menu (`menu_after_clear`).
pub fn show_menu(x: &mut Ctx) -> Flow {
    layers_off(x);
    clear_screen(x);
    full_brightness(x);
    for off in var::SCROLLS {
        x.set_v16(off, 0);
    }
    crate::world::tasks::one_frame(x, crate::screens::service::task::Step::MenuAfterClear)
}

/// `0x291ec`: the menu, once the screen is cleared.
pub fn menu_after_clear(x: &mut Ctx) -> Flow {
    var::FRAME_FLAG.set(x.b, 0);
    MENU_BLOCK.set8(x.b, f::SECRET, 0);
    MENU_BLOCK.set8(x.b, f::REPEAT_V, 1);
    regs::OBJECT_OUTPUT.set(x.b, MENU_OUTPUTS);
    crate::services::video::unlock_counters(x.b, 0xf1);
    print(x, text::MENU.addr());
    print(x, text::HELP.addr());
    let line = MENU_BLOCK.get8(x.b, f::CURSOR);
    arrow(x, MENU_ARROW, line);
    if !config::is_factory(x.b) {
        paint(x, MENU_CONFIG_LINE, HIGHLIGHT, 0xc);
    }
    layers_on(x);
    Flow::Done
}

/// `0x29254`: choice of a line (up, down), entry into the screen (fire 1).
fn menu(x: &mut Ctx) -> Flow {
    if SECRET_ENABLE.word(x.b, 0) != 0 {
        if MENU_BLOCK.get8(x.b, f::SECRET) != 0 {
            // the program word 0xc0 is zero in this version (docs/service.md): the "Secret Test" is not rewritten
            unreachable!("TEST menu: \"Secret Test\" (0xee16e) requested");
        }
        if (x.v8(0) & x.v8(1)) & key::SHOT2 != 0 {
            MENU_BLOCK.set8(x.b, f::SECRET, 1);
            MENU_BLOCK.set8(x.b, f::STEP, 0);
            return Flow::Done;
        }
    }
    let line = MENU_BLOCK.get8(x.b, f::CURSOR);
    if let Some(to) = vertical(x, line, 2, LAST_LINE) {
        if to != line {
            move_arrow(x, MENU_ARROW, line, to);
            MENU_BLOCK.set8(x.b, f::CURSOR, to);
        }
    }
    if pressed(x) & key::SHOT1 != 0 {
        MENU_BLOCK.set8(x.b, f::STATE, state::SCREEN);
        if MENU_BLOCK.get8(x.b, f::CURSOR) != LAST_LINE {
            crate::services::video::clear_layer(x.b, Layer::One);
        }
    }
    Flow::Done
}

/// `0x292da`: the screen of the chosen line.
fn screen(x: &mut Ctx) -> Flow {
    match MENU_BLOCK.get8(x.b, f::CURSOR) {
        0 => io_tests::input(x),
        2 => io_tests::output(x),
        4 => io_tests::sound(x),
        6 => test_pattern::color(x),
        8 => test_pattern::cross_hatch(x),
        10 => test_pattern::game_data(x),
        12 => settings::configuration(x),
        14 => memory::memory(x),
        _ => exit(x),
    }
}

/// `0x29314` (9. EXIT): screen cleared, the task becomes the startup task again (`0xae4`).
fn exit(x: &mut Ctx) -> Flow {
    clear_layers(x);
    for off in var::SCROLLS {
        x.set_v32(off, 0);
    }
    var::ACTIVE.set(x.b, 0);
    crate::world::tasks::restart(x, BOOT_TASK)
}

/// `0x29a48`: held 1P start and fire 1: return to the menu.
pub fn exit_check(x: &mut Ctx) -> Flow {
    if var::SYSTEM.get(x.b) & key::START1 == 0 || x.v8(var::INPUT) & key::SHOT1 == 0 {
        return Flow::Done;
    }
    back_to_menu(x)
}

/// `0x29a5a`: return to the menu (the sound is cut).
pub fn back_to_menu(x: &mut Ctx) -> Flow {
    crate::services::sound::service(x.b, crate::services::sound::Service::GeneralStop);
    MENU_BLOCK.set8(x.b, f::STATE, state::MENU);
    // step of the screen and configuration cursors (bytes 2 to 5)
    MENU_BLOCK.set16(x.b, f::STEP, 0);
    MENU_BLOCK.set16(x.b, f::STEP + 2, 0);
    let flags = var::FLAGS.get(x.b) & 0x7fff;
    var::FLAGS.set(x.b, flags);
    var::BYTE_34.set(x.b, 0);
    show_menu(x)
}

