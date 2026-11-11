//! Tools: the natives of `crate::services::video` for the original code that calls them (arguments and results in the registers
//! of the original).

use crate::world::World;
use m68k::Cpu;

use crate::services::video::*;
use crate::tools::frames::*;
use crate::{regs, Native};
use crate::hw::mem::{rd16, set_low_byte, set_low_word, wr16};
use crate::world::terrain::terrain_regs;

/// 0xb40: called at each wait (0xa40); sets D1 then draws the gauge (0xb46).
fn gauge_with_value(c: &mut Cpu, b: &mut World) {
    c.d[1] = GAUGE;
    gauge(c, b);
}

/// `update_gauge` (D1 = marker) with the registers of the original: D2 and the flags of the last computation, A0
/// after the last write (the world update leaves them to the next handler).
pub(crate) fn gauge_68k(c: &mut Cpu, b: &mut World) {
    let flags_clear = !(m68k::SR_N | m68k::SR_Z | m68k::SR_V | m68k::SR_C);
    if !update_gauge(b, c.d[1]) {
        c.d[2] = c.d[2] & 0xffff_0000 | 0x400;
        c.sr &= flags_clear;
    } else {
        c.d[2] |= 0xffff;
        c.a[0] = hw::TEXT_LAYER + 4 * GAUGE_LONGS;
        c.sr = c.sr & flags_clear
            | if c.d[1] == 0 { m68k::SR_Z } else { 0 }
            | if c.d[1] & 0x8000_0000 != 0 { m68k::SR_N } else { 0 };
    }
}

fn gauge(c: &mut Cpu, b: &mut World) {
    gauge_68k(c, b);
    rts(c, b);
}

fn lock(c: &mut Cpu, b: &mut World) {
    let forced = lock_counters(b, c.d[0] as u8);
    set_low_byte(&mut c.d[1], forced);
    rts(c, b);
}

/// D0 is returned unchanged.
fn unlock(c: &mut Cpu, b: &mut World) {
    let forced = unlock_counters(b, c.d[0] as u8);
    set_low_byte(&mut c.d[1], forced);
    rts(c, b);
}

fn enable_layers(c: &mut Cpu, b: &mut World) {
    turn_layers_on(b, c.d[0] as u16);
    rts(c, b);
}

fn disable_layers(c: &mut Cpu, b: &mut World) {
    let bits = c.d[0] as u16;
    set_low_word(&mut c.d[0], !bits);
    turn_layers_off(b, bits);
    rts(c, b);
}

/// `clear_layer1_start` with the registers of the original: the cell in D0-D5, A2 and A3, D6 = -1 and A1 at the end
/// of the zone.
pub(crate) fn clear_layer1_start_68k(c: &mut Cpu, b: &mut World) {
    clear_layer1_start(b);
    leftovers_clear_layer1_start(c);
}

/// The registers that 0x3525c leaves.
pub(crate) fn leftovers_clear_layer1_start(c: &mut Cpu) {
    const SIZE: u32 = SCROLL1_START;
    for n in 0..6 {
        c.d[n] = 0x1f_0000;
    }
    c.a[2] = 0x1f_0000;
    c.a[3] = 0x1f_0000;
    c.a[1] = hw::SCROLL1 + SIZE;
    set_low_word(&mut c.d[6], 0xffff);
    c.sr &= !(m68k::SR_N | m68k::SR_Z | m68k::SR_V | m68k::SR_C);
}

fn clear_scroll1_start(c: &mut Cpu, b: &mut World) {
    clear_layer1_start_68k(c, b);
    rts(c, b);
}

const fn fixed(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut World), outputs: u32) -> Native {
    Native { entry, name, run: crate::Body::WithCpu(run), touches_sound: false, outputs }
}

/// 0x24b0: terrain number at (D0, D1).
fn terrain_native(c: &mut Cpu, b: &mut World) {
    let t = terrain(b, c.d[0] as u16, c.d[1] as u16);
    terrain_regs(c, &t);
    rts(c, b);
}

/// 0x24a8: terrain number under object A0 (x position at +8, y at +0x10).
fn terrain_of_object(c: &mut Cpu, b: &mut World) {
    set_low_word(&mut c.d[0], rd16(b, c.a[0] + 8));
    set_low_word(&mut c.d[1], rd16(b, c.a[0] + 0x10));
    terrain_native(c, b);
}

/// `load_grid` (D0 = number, A1 = destination) with the registers of the original: D0 = 0xffff, A1 and A2 at the end
/// of the grid.
pub(crate) fn load_grid_68k(c: &mut Cpu, b: &mut World, grid: Grid) {
    let end = load_grid(b, grid, c.d[0] as u16, c.a[1]);
    c.d[0] = 0xffff;
    c.a[1] = end.destination;
    c.a[2] = end.indices;
}

fn load_blocks(c: &mut Cpu, b: &mut World, grid: Grid) {
    load_grid_68k(c, b, grid);
    rts(c, b);
}

pub(crate) const LOAD_BLOCKS_4X4: Native = fixed(0x260c, "video_blocks_4x4", |c, b| load_blocks(c, b, Grid::Blocks4x4), LOAD_OUT);

pub(crate) const LOAD_BLOCKS_SCROLL2: Native = fixed(0x26d4, "video_blocks_scroll2", |c, b| load_blocks(c, b, Grid::Layer2), LOAD_OUT);

pub(crate) const LOAD_BLOCKS_SMALL: Native = fixed(0x2766, "video_blocks_small", |c, b| load_blocks(c, b, Grid::Layer3), LOAD_OUT);

/// 0x255a: D0 (0 to 99, saturated) as two decimal digits: tens in A1, units one column further.
/// Returns D0 = units, D1 = tens, A1 = units cell + 2.
fn decimal_digits(c: &mut Cpu, b: &mut World) {
    let mut value = c.d[0] as u16;
    if value >= 100 {
        value = 99;
    }
    let (tens, units) = (value / 10, value % 10);
    put_digit(b, c.a[1], tens);
    put_digit(b, c.a[1] + COLUMN, units);
    c.d[1] = tens as u32;
    set_low_word(&mut c.d[0], units);
    c.a[1] += COLUMN + 2;
    crate::tools::ops::logic(c, 0, m68k::decode::Size::Word);
    rts(c, b);
}

/// 0x25e8: D0 as four hexadecimal digits (high nibble first), one per column starting at A1.
/// Returns A1 = A1 + 4 columns.
fn hex_digits(c: &mut Cpu, b: &mut World) {
    for i in 0..4 {
        put_digit(b, c.a[1] + COLUMN * i, (c.d[0] as u16) >> (12 - 4 * i) & 0xf);
    }
    c.a[1] += 4 * COLUMN;
    crate::tools::ops::logic(c, 0, m68k::decode::Size::Word);
    rts(c, b);
}

pub(crate) const GAUGE_NATIVE: Native = fixed(0xb46, "video_gauge", gauge, regs::d(2) | regs::a(0) | regs::CCR);

/// 0xb3e: `bra.b 0xb46`, called by the world update (D1 = class marker).
pub(crate) const GAUGE_THUNK_NATIVE: Native = fixed(0xb3e, "video_gauge_b3e", gauge, regs::d(2) | regs::a(0) | regs::CCR);

pub(crate) const GAUGE_VALUE_NATIVE: Native = fixed(0xb40, "video_gauge_d1", gauge_with_value, regs::d(1) | regs::d(2) | regs::a(0) | regs::CCR);

pub(crate) const LOCK: Native = fixed(0xb96, "video_lock", lock, regs::d(1));

pub(crate) const UNLOCK: Native = fixed(0xbac, "video_deverrou", unlock, regs::d(0) | regs::d(1));

pub(crate) const ENABLE_LAYERS: Native = fixed(0xbc6, "video_layers_on", enable_layers, 0);

pub(crate) const DISABLE_LAYERS: Native = fixed(0xbfc, "video_layers_off", disable_layers, regs::d(0));

pub(crate) const CLEAR_SCROLL1_START: Native = fixed(0x3525c, "video_clear_start_scroll1", clear_scroll1_start, regs::ALL);

pub(crate) const TERRAIN: Native = fixed(0x24b0, "video_terrain", terrain_native, TERRAIN_OUT);

pub(crate) const TERRAIN_OF_OBJECT: Native = fixed(0x24a8, "video_terrain_object", terrain_of_object, TERRAIN_OUT);

pub(crate) const DECIMAL_DIGITS: Native = fixed(0x255a, "video_digits_decimal", decimal_digits, regs::d(0) | regs::d(1) | regs::a(1) | regs::CCR);

pub(crate) const HEX_DIGITS: Native = fixed(
    0x25e8,
    "video_digits_hex",
    hex_digits,
    regs::a(1) | regs::CCR,
);

/// Fill value of the loading gauge: tile 0x20 (space), attribute 3.
const GAUGE: u32 = 0x0020_0003;

const TERRAIN_OUT: u32 = regs::d(0) | regs::d(1) | regs::d(2) | regs::d(3) | regs::d(4) | regs::a(1) | regs::CCR;

const LOAD_OUT: u32 = regs::d(0) | regs::a(1) | regs::a(2);

/// Writes a digit (tile number = value, attribute 0) at `at`; the next cell of the row is
/// `COLUMN` further in the character layer.
fn put_digit(b: &mut World, at: u32, digit: u16) {
    wr16(b, at, digit);
    wr16(b, at + 2, 0);
}

/// Next column of the character layer (see `text.rs`).
const COLUMN: u32 = 0x80;
