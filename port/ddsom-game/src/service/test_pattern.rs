//! Screens 4. COLOR, 5. DOT CROSS HATCH and 6. GAME DATA of the TEST menu: two preparation steps (or one),
//! then waiting for the return to the menu (1P start + fire 1).

use super::{decimal, exit_check, f, get, layers_off, layers_on, next_step, print, var};
use crate::mem::wr32;
use crate::modes::Flow;
use crate::scene::Ctx;

/// Help common to the test patterns: "EXIT = 1P START & SHOT1".
const HELP: crate::screens::tables::Rom = crate::screens::tables::Rom::at(0x2a076);

// ---------------------------------------------------------------------------------------------
// 4. COLOR (0x29366)

mod color {
    /// Bar palettes: 4 × 16 colours (red, green, blue, white) copied into palettes 0x1c-0x1f.
    pub const RAMPS: crate::screens::tables::Rom = crate::screens::tables::Rom::at(0x293a6);
    pub const PALETTES: u32 = 0x91c780;
    pub const TITLE: crate::screens::tables::Rom = crate::screens::tables::Rom::at(0x2a224);
    /// Bars: (starting cell, palette).
    pub const BARS: [(u32, u16); 4] = [(0x9127a0, 0x1c), (0x9127b4, 0x1d), (0x9127c8, 0x1e), (0x9127dc, 0x1f)];
    /// Tile of the first column (the following ones: +1).
    pub const FIRST_TILE: u16 = 0x11;
}

pub fn color(x: &mut Ctx) -> Flow {
    match get(x, f::STEP) {
        0 => {
            next_step(x);
            layers_off(x);
            for k in 0..0x40 {
                let v = color::RAMPS.word(x.b, 2 * k);
                x.wr16(color::PALETTES + 2 * k, v);
            }
            Flow::Done
        }
        2 => {
            next_step(x);
            print(x, color::TITLE.addr());
            print(x, HELP.addr());
            for (cell, palette) in color::BARS {
                bar(x, cell, palette);
            }
            layers_on(x);
            Flow::Done
        }
        _ => exit_check(x),
    }
}

/// `0x29470`: a bar of 15 columns two cells wide and four high, one colour per column.
fn bar(x: &mut Ctx, cell: u32, palette: u16) {
    for column in 0..15 {
        let tile = color::FIRST_TILE + column as u16;
        let top = cell + 0x100 * column;
        for row in 0..4 {
            let at = top + 4 * row;
            for c in [at, at + 0x80] {
                x.wr16(c, tile);
                x.wr16(c + 2, palette);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 5. DOT CROSS HATCH (0x2949c)

mod hatch {
    pub const TITLE: crate::screens::tables::Rom = crate::screens::tables::Rom::at(0x2a298);
    /// Palette 0x40 (0x91c800): white, then 0xff00 alternating.
    pub const PALETTE: u32 = 0x91c800;
    /// Plane 2 (32-bit cells, columns of 0x40 bytes).
    pub const LAYER: u32 = 0x917000;
}

pub fn cross_hatch(x: &mut Ctx) -> Flow {
    match get(x, f::STEP) {
        0 => {
            next_step(x);
            var::LAYERS.set(x.b, super::ALL_LAYERS);
            // D0 is the step (0): no layer cut off
            crate::video::turn_layers_off(x.b, 0);
            print(x, hatch::TITLE.addr());
            print(x, HELP.addr());
            for k in 0..8 {
                wr32(x.b, hatch::PALETTE + 4 * k, 0xffff_ffff);
                wr32(x.b, hatch::PALETTE + 0x20 + 4 * k, 0xff00_ff00);
            }
            Flow::Done
        }
        2 => {
            next_step(x);
            grid(x);
            var::LAYERS.set(x.b, super::ALL_LAYERS);
            // D0 is the step (2)
            crate::video::turn_layers_on(x.b, 2);
            Flow::Done
        }
        _ => exit_check(x),
    }
}

/// `0x29578`: `n` cells of value `v`, `stride` bytes apart. Returns the cell that follows.
fn fill(x: &mut Ctx, mut cell: u32, v: u32, stride: u32, n: u32) -> u32 {
    for _ in 0..n {
        wr32(x.b, cell, v);
        cell += stride;
    }
    cell
}

/// `0x2950e`: the grid (tiles 0x1891-0x1899 of plane 2).
fn grid(x: &mut Ctx) {
    let cell = fill(x, hatch::LAYER + 4, 0x1897_0001, 0x40, 23);
    wr32(x.b, cell, 0x1891_0001);
    fill(x, cell + 4, 0x1898_0041, 4, 13);
    let cell = fill(x, hatch::LAYER + 8, 0x1899_0001, 4, 12);
    fill(x, cell, 0x1899_0001, 0x40, 23);
    for column in 0..22 {
        fill(x, hatch::LAYER + 0x48 + 0x40 * column, 0x1899_0000, 4, 12);
    }
}

// ---------------------------------------------------------------------------------------------
// 6. GAME DATA (0x2b808)

mod data {
    use crate::ram::Long;
    pub const TITLE: crate::screens::tables::Rom = crate::screens::tables::Rom::at(0x2b92a);
    pub const HELP: crate::screens::tables::Rom = crate::screens::tables::Rom::at(0x2b94a);
    pub const COUNTERS: crate::screens::tables::Rom = crate::screens::tables::Rom::at(0x2b984);
    pub const FREEPLAY: crate::screens::tables::Rom = crate::screens::tables::Rom::at(0x2b9b0);
    pub const CARDS: crate::screens::tables::Rom = crate::screens::tables::Rom::at(0x2baf0);
    /// Play time text (development card).
    pub const TIME: crate::screens::tables::Rom = crate::screens::tables::Rom::at(0x2b96c);
    /// First cell of the play time (hours, minutes, seconds, one digit per line).
    pub const TIME_CELL: u32 = 0x912f9c;
    /// Play time (A5 - 0x615c) and its indicator (A5 - 0x6158): without indicator, 00:00:00.
    pub const TIME_VALUE: Long = Long(0xff_1ea4);
    pub const TIME_FLAG: Long = Long(0xff_1ea8);
    /// Seconds then minutes (words, A5 - 0x62a0 and - 0x629e).
    pub const SECONDS: Long = Long(0xff_1d60);
    /// Cells of the three counters (8 digits).
    pub const CELLS: [u32; 3] = [0x912fa4, 0x912fac, 0x912fb4];
    /// Counters: coins, services, free games, cards. In EEPROM (A5 + 0x76, 0x7a, 0x7e, 0x7fd4);
    /// on the development card, copies in extra RAM (A5 - 0x6164, - 0x6160, - 0x6154, - 0x6150).
    pub const EEPROM_COUNTERS: [u32; 4] = [crate::mem::A5 + 0x76, crate::mem::A5 + 0x7a, crate::mem::A5 + 0x7e, crate::mem::A5 + 0x7fd4];
    pub const DEV_COUNTERS: [u32; 4] = [0xff_1e9c, 0xff_1ea0, 0xff_1eac, 0xff_1eb0];
}

/// `0x2b86e`, `0x29ca0`, `0x29cc4`: the play time (counter divided by the indicator, which is the counter's
/// frequency) as `hh:mm:ss`; signed divisions; hours capped at 99 (minutes then 0x3b3b, seconds 0).
fn play_time(x: &mut Ctx) {
    let value = data::TIME_VALUE.get(x.b) as i32;
    let rate = data::TIME_FLAG.get(x.b) as i32;
    let (hours, seconds, minutes);
    if rate == 0 {
        (hours, seconds, minutes) = (0, 0, 0);
    } else {
        let divide = |v: i32, d: i32| {
            let q = (v.unsigned_abs() / d.unsigned_abs()) as i32;
            (if (v < 0) != (d < 0) { -q } else { q }, v.unsigned_abs() % d.unsigned_abs())
        };
        let (q, _) = divide(value, rate);
        let (q, s) = divide(q, 60);
        let (q, m) = divide(q, 60);
        (hours, seconds, minutes) = if q as u16 > 0x63 { (0x63, 0, 0x3b3b) } else { (q as u32, s, m) };
    }
    data::SECONDS.set(x.b, seconds << 16 | minutes);
    let mut cell = data::TIME_CELL;
    for (k, v) in [hours, minutes, seconds].into_iter().enumerate() {
        cell = decimal(x, cell, v, 2, 0);
        if k < 2 {
            wr32(x.b, cell, 0x003a_0000);
            cell += 0x80;
        }
    }
}

pub fn game_data(x: &mut Ctx) -> Flow {
    if get(x, f::STEP) != 0 {
        return exit_check(x);
    }
    next_step(x);
    layers_off(x);
    print(x, data::TITLE.addr());
    print(x, data::HELP.addr());
    // development card (never on a cabinet): play time, counters kept in RAM
    let dev = var::DEV_BOARD.get(x.b) != 0;
    let counters = if dev { data::DEV_COUNTERS } else { data::EEPROM_COUNTERS };
    if dev {
        print(x, data::TIME.addr());
        play_time(x);
    }
    print(x, data::COUNTERS.addr());
    for (cell, counter) in [(data::CELLS[0], counters[0]), (data::CELLS[1], counters[1])] {
        let v = x.rd32(counter);
        decimal(x, cell, v, 8, 0);
    }
    let (label, counter) = if matches!(var::VARIANT.get(x.b), 4 | 10) {
        (data::CARDS.addr(), counters[3])
    } else {
        (data::FREEPLAY.addr(), counters[2])
    };
    print(x, label);
    let v = x.rd32(counter);
    decimal(x, data::CELLS[2], v, 8, 0);
    layers_on(x);
    Flow::Done
}
