//! Drawing of the panels on the character layer: banners, portraits, digits, HP gauge, names.
//!
//! Each zone of a panel (`zone::*`) is a rectangle of cells whose origin comes from `layer::origin`.
//! The tiles come from ROM tables; the palette attributes depend on the player (8 + number).

use cps2::Cps2Bus;

use super::layer::{columns, origin, Brush, COLUMN, EMPTY};
use super::{panel, player, v8, var, Slot};
use crate::mem::{prog16, rd16, rd32, rd8, wr16, wr32};

/// Zones of a panel (row index into the origins table).
pub mod zone {
    /// Name of the chosen character (4x2 cells).
    pub const NAME_LETTERS: u16 = 1;
    /// HP gauge.
    pub const HP_BAR: u16 = 2;
    pub const CHARACTER_NAME: u16 = 3;
    /// "INSERT COIN / PUSH START" banner, cleared before drawing another one.
    pub const BANNER: u16 = 4;
    /// Choice countdown.
    pub const COUNTDOWN: u16 = 5;
    /// Digits of the continue countdown.
    pub const CONTINUE_DIGITS: u16 = 6;
    pub const PORTRAIT: u16 = 7;
    pub const FACE: u16 = 8;
    pub const ITEMS: u16 = 9;
    pub const MONEY: u16 = 0xa;
    pub const SCORE: u16 = 0xb;
    /// Player's large frame (cleared when they leave).
    pub const FRAME: u16 = 0xc;
    pub const SELECTION: u16 = 0xd;
    pub const NAMES_ALT: u16 = 0xe;
    pub const START_BANNER: u16 = 0xf;
    pub const ITEMS_ROW: u16 = 0x10;
    pub const RANKS: u16 = 0x11;
    pub const CLASS_ICON: u16 = 0x12;
}

/// Palette attribute of a player.
pub fn player_attr(b: &mut Cps2Bus, s: Slot) -> u16 {
    8 + s.index(b) as u16
}

/// Narrow columns (2-column layout, `A5+0x344` odd).
pub fn narrow(b: &mut Cps2Bus) -> bool {
    v8(b, var::LAYOUT) & 1 != 0
}

pub(super) fn zone_brush<'a>(b: &'a mut Cps2Bus, s: Slot, zone: u16) -> Brush<'a> {
    let index = s.index(b);
    let at = origin(b, index, zone);
    Brush::new(b, at)
}

/// Clears a rectangle of `columns` columns and `rows` rows starting at `at`.
fn clear_rect(b: &mut Cps2Bus, at: u32, rows: u32, columns: u32) {
    for column in 0..columns {
        for row in 0..rows {
            wr32(b, at + column * COLUMN + row * 4, EMPTY);
        }
    }
}

/// 0x16c86: clears the two layer strips reserved for the panels (top and bottom of the screen).
pub fn clear_panels(b: &mut Cps2Bus) {
    clear_rect(b, 0x91_2008, 4, 48);
    clear_rect(b, 0x91_2068, 4, 48);
}

// ---------------------------------------------------------------------------------------------
// Banners
// ---------------------------------------------------------------------------------------------

/// 0x17542: clears the panel banner (3 cells high: 24 columns, or twice 16 columns in the
/// 3-player layout; the second half comes 25 rows lower).
pub fn clear_banner(b: &mut Cps2Bus, s: Slot) {
    const WIDE: u32 = 0x3f_55e2;
    const HALF: u32 = 0x3f_6758;
    const ATTR: u16 = 0xe;
    let layout = v8(b, var::LAYOUT);
    let mut brush = zone_brush(b, s, zone::BANNER);
    let start = brush.at;
    let (table, n) = if layout == 1 { (HALF, 16) } else { (WIDE, 24) };
    columns(&mut brush, table, n, ATTR);
    if layout != 0 {
        brush.at = start + 0x64;
        columns(&mut brush, table, n, ATTR);
    }
}

/// 0x17400 / 0x17458: "PUSH START" banner (after clearing the old one), wide and narrow versions.
pub fn start_banner(b: &mut Cps2Bus, s: Slot, tables: (u32, u32)) {
    clear_banner(b, s);
    let (table, n) = if narrow(b) { (tables.1, 12) } else { (tables.0, 17) };
    let mut brush = zone_brush(b, s, zone::START_BANNER);
    columns(&mut brush, table, n, 0xe);
}

pub const START_BANNER_A: (u32, u32) = (0x3f_5c06, 0x3f_6710);
pub const START_BANNER_B: (u32, u32) = (0x3f_5c6c, 0x3f_66c8);

/// One of the "INSERT COIN" banners (pointer tables in the code, a column of 3 words per entry):
/// `source` is the address of the table pointer, `rows` the number of columns minus one.
pub fn pointer_banner(b: &mut Cps2Bus, s: Slot, pointer: u32, columns_count: u32, attr: u16) {
    let table = rd32(b, pointer);
    let mut brush = zone_brush(b, s, zone::START_BANNER);
    columns(&mut brush, table, columns_count, attr);
}

// ---------------------------------------------------------------------------------------------
// Digits
// ---------------------------------------------------------------------------------------------

/// 0x159fc: the choice countdown (two BCD digits, one cell each).
pub fn countdown_digits(b: &mut Cps2Bus, s: Slot) {
    let value = s.p8(b, panel::COUNTDOWN);
    let attr = player_attr(b, s);
    let mut brush = zone_brush(b, s, zone::COUNTDOWN);
    for digit in [value >> 4, value & 0xf] {
        brush.cell(0x65c + digit as u16, attr);
        brush.skip_column_after_cell();
    }
}

/// 0x1749e: the two digits of the continue countdown (large: two cells high, or just one in
/// narrow columns). The tens digit uses tiles in reverse order.
pub fn continue_digits(b: &mut Cps2Bus, s: Slot) {
    let value = s.p8(b, panel::COUNTDOWN);
    let narrow = narrow(b);
    let brush = zone_brush(b, s, zone::CONTINUE_DIGITS);
    let (tens, units) = ((value >> 4) as u16, (value & 0xf) as u16);
    let at = brush.at;
    let (tens_tile, units_tile) = if narrow { (0x641u16.wrapping_sub(tens), 0x642 + units) } else { (0x621u16.wrapping_sub(tens), 0x622 + units) };
    for (column, tile) in [(0, tens_tile), (1, units_tile)] {
        let cell = at + column * COLUMN;
        wr16(brush.b, cell, tile);
        wr16(brush.b, cell + 2, 0xe);
        if !narrow {
            wr16(brush.b, cell + 4, tile + 0x10);
            wr16(brush.b, cell + 6, 0xe);
        }
    }
}

/// A decimal digit on screen: a non-significant zero (while no digit has been written yet) is replaced by
/// an empty cell. Returns the next position.
struct Digits<'a> {
    brush: Brush<'a>,
    attr: u16,
    base: u16,
    started: bool,
}

impl Digits<'_> {
    fn put(&mut self, nibble: u8) {
        let nibble = nibble & 0xf;
        if nibble == 0 && !self.started {
            self.brush.cell(0x8e1, self.attr);
        } else {
            self.brush.cell(self.base + nibble as u16, self.attr);
            self.started = true;
        }
        self.brush.skip_column_after_cell();
    }

    fn byte(&mut self, value: u8) {
        self.put(value >> 4);
        self.put(value);
    }
}

/// 0x1725e: the score (player `+0x110`, seven BCD digits); returns the cell following the last digit.
pub fn score(b: &mut Cps2Bus, s: Slot) -> u32 {
    let value = s.g32(b, player::SCORE);
    s.set_p32(b, panel::SHOWN_SCORE, value);
    let attr = player_attr(b, s);
    let [b0, b1, b2, b3] = value.to_be_bytes();
    let brush = zone_brush(b, s, zone::SCORE);
    let mut digits = Digits { brush, attr, base: 0x960, started: false };
    digits.put(b0);
    digits.byte(b1);
    digits.byte(b2);
    // the last byte is written in full, zeros included
    digits.put(b3 >> 4);
    digits.started = true;
    digits.put(b3);
    digits.brush.at
}

/// 0x171da: the money (player `+0x114`); in narrow columns two padding cells precede the digits.
/// Returns the cell following the last digit.
pub fn money(b: &mut Cps2Bus, s: Slot) -> u32 {
    let value = s.g32(b, player::MONEY);
    s.set_p32(b, panel::SHOWN_MONEY, value);
    let attr = player_attr(b, s);
    let narrow = narrow(b);
    let [_, b1, b2, b3] = value.to_be_bytes();
    let mut brush = zone_brush(b, s, zone::MONEY);
    if narrow {
        for _ in 0..2 {
            brush.cell(0x8e1, attr);
            brush.skip_column_after_cell();
        }
    }
    let mut digits = Digits { brush, attr, base: 0x970, started: false };
    digits.put(b1);
    digits.byte(b2);
    digits.put(b3 >> 4);
    digits.started = true;
    digits.put(b3);
    digits.brush.at
}

// ---------------------------------------------------------------------------------------------
// Portraits and identity
// ---------------------------------------------------------------------------------------------

/// Portrait pointer tables (one per player) in the code.
const PORTRAIT_POINTERS: [u32; 2] = [0x17b96, 0x17ba6];

/// 0x17096 / 0x170e0: player portrait (3 cells high); `alt` selects the second series of images.
pub fn portrait(b: &mut Cps2Bus, s: Slot, alt: bool) {
    let attr = player_attr(b, s);
    let index = s.index(b) as u32;
    let (table, n) = if narrow(b) {
        (if alt { 0x3f_6668 } else { 0x3f_6ba8 }, 16)
    } else {
        (rd32(b, PORTRAIT_POINTERS[alt as usize] + 4 * index), 24)
    };
    let mut brush = zone_brush(b, s, zone::PORTRAIT);
    columns(&mut brush, table, n, attr);
}

/// Tile of the first cell of each name row (per character and variant), code table.
const NAME_TILES: u32 = 0x16fe2;

/// 0x17012 / 0x1701c: the name of character `character` (4 cells wide, 2 high).
pub fn character_name(b: &mut Cps2Bus, s: Slot, character: u8) {
    let attr = 4 + s.index(b) as u16;
    let mut tile = prog16(b, NAME_TILES.wrapping_add_signed(2 * character as i8 as i32));
    let mut brush = zone_brush(b, s, zone::CHARACTER_NAME);
    let start = brush.at;
    for row in 0..2 {
        brush.at = start + 4 * row;
        for _ in 0..4 {
            brush.cell(tile, attr);
            brush.skip_column_after_cell();
            tile = tile.wrapping_add(1);
        }
        tile = tile.wrapping_add(0xc);
    }
}

/// Tiles of the class icon strip (per character).
const CLASS_ICON_TILES: u32 = 0x1707c;

/// 0x17034: class icon of the player's character (strip of 6 cells, or one cell in narrow columns).
pub fn class_icon(b: &mut Cps2Bus, s: Slot) {
    let character = s.g8(b, player::CHARACTER) & !1;
    let attr = player_attr(b, s);
    let narrow = narrow(b);
    let mut brush = zone_brush(b, s, zone::CLASS_ICON);
    if narrow {
        brush.cell(0x96a + (character >> 1) as u16, attr);
    } else {
        let mut tile = prog16(brush.b, CLASS_ICON_TILES + character as u32);
        for _ in 0..6 {
            brush.cell(tile, attr);
            brush.skip_column_after_cell();
            tile = tile.wrapping_add(1);
        }
    }
}

/// Face tiles (two cells per class, read in the code as data).
const FACE_TILES: u32 = 0x17136;

/// 0x17106: the face of class `class` (8 to 15), two cells wide.
pub fn face(b: &mut Cps2Bus, s: Slot, class: u8) {
    let attr = player_attr(b, s);
    let tiles = FACE_TILES + 4 * (class.wrapping_sub(8) as u32);
    let mut brush = zone_brush(b, s, zone::FACE);
    for k in 0..2 {
        let tile = rd16(brush.b, tiles + 2 * k);
        brush.cell(tile, attr);
        brush.skip_column_after_cell();
    }
}

/// Tiles of the special items (codes 0xf9 to 0xfe of an item row), code table.
const ITEM_SPECIAL_TILES: u32 = 0x171b4;

/// 0x17176: the player's six items (one cell each; a special code of at least 0xf9 fills the end of the
/// row with a text).
pub fn items(b: &mut Cps2Bus, s: Slot) {
    let attr = player_attr(b, s);
    let codes: Vec<u8> = (0..6).map(|k| s.g8(b, player::ITEMS + k)).collect();
    let mut brush = zone_brush(b, s, zone::ITEMS);
    for (k, &code) in codes.iter().enumerate() {
        if code == 0xff {
            brush.at += COLUMN;
        } else if code < 0xf9 {
            wr16(brush.b, brush.at, 0x860 + code as u16);
            wr16(brush.b, brush.at + 2, attr);
            brush.at += COLUMN;
        } else {
            let index = (code.wrapping_neg().wrapping_sub(2) as u16 as u32) * 2;
            let mut tile = prog16(brush.b, ITEM_SPECIAL_TILES + index);
            for _ in k..6 {
                brush.cell(tile, attr);
                brush.skip_column_after_cell();
                tile = tile.wrapping_add(1);
            }
            return;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Clearing zones
// ---------------------------------------------------------------------------------------------

/// 0x16f22: clears the player's large frame.
pub fn clear_frame(b: &mut Cps2Bus, s: Slot) {
    let at = zone_brush(b, s, zone::FRAME).at;
    if narrow(b) {
        clear_rect(b, at, 5, 16);
    } else if rd8(b, var::FROZEN) != 0 {
        let at = if s.index(b) & 2 != 0 { at + 8 } else { at };
        clear_rect(b, at, 3, 24);
    } else if v8(b, 0x50) != 0 && s.index(b) & 2 != 0 {
        clear_rect(b, at + 4, 4, 24);
    } else {
        clear_rect(b, at, 5, 24);
    }
}

/// 0x16f9e: clears the item row.
pub fn clear_items_row(b: &mut Cps2Bus, s: Slot) {
    let at = zone_brush(b, s, zone::ITEMS_ROW).at;
    let width = if narrow(b) { 16 } else { 24 };
    clear_rect(b, at, 4, width);
}

/// 0x16f0a: clears a 24-column row at `at`; returns the start of the next row.
pub(super) fn clear_row(b: &mut Cps2Bus, at: u32) -> u32 {
    for column in 0..24 {
        wr32(b, at + column * COLUMN, EMPTY);
    }
    at + 4
}

// ---------------------------------------------------------------------------------------------
// Palettes
// ---------------------------------------------------------------------------------------------

/// 0x17b56: copies 8 longs (16 colors) from `source` to `dest` keeping the brightness (high nibbles)
/// of the first long of the destination.
pub fn blend_palette(b: &mut Cps2Bus, source: u32, dest: u32) {
    let keep = rd32(b, dest) & 0xf000_f000;
    for k in 0..8 {
        let color = rd32(b, source + 4 * k) & 0x0fff_0fff;
        wr32(b, dest + 4 * k, color | keep);
    }
}

/// Player palette in the page: 32 bytes per palette.
pub fn blend_for(b: &mut Cps2Bus, s: Slot, source: u32, dest: u32) {
    let player = s.index(b) as u32;
    let character = s.g8(b, player::CHARACTER) as u32;
    blend_palette(b, source + 32 * character, dest + 32 * player);
}

/// 0x17b14: the three palettes of the player's character.
pub fn character_palettes(b: &mut Cps2Bus, s: Slot) {
    blend_for(b, s, 0x25_c640, 0x91_c500);
    blend_for(b, s, 0x25_c4c0, 0x91_c480);
    blend_for(b, s, 0x25_c7c0, 0x91_c700);
}

/// 0x1796c: palette of the player's waiting room: `base` is the offset specific to the player number
/// (code table), to which 32 bytes per character are added.
pub fn lobby_palette(b: &mut Cps2Bus, s: Slot, base: u16) {
    let character = s.g8(b, player::CHARACTER) as u16;
    let source = base.wrapping_add(character << 5) as i16 as i32;
    let number = s.g8(b, player::NUMBER) as u16;
    let dest = ((0x13 + number) << 5) as i16 as i32;
    blend_palette(b, 0x25_c940u32.wrapping_add_signed(source), 0x91_c400u32.wrapping_add_signed(dest));
}

/// Closing cell after a number in narrow columns.
pub fn closing_cell(b: &mut Cps2Bus, s: Slot, at: u32, tile: u16) {
    let attr = player_attr(b, s);
    wr16(b, at, tile);
    wr16(b, at + 2, attr);
}

/// 0x17aee: palette of the character currently being chosen.
pub fn choice_palette(b: &mut Cps2Bus, s: Slot) {
    let player = s.index(b) as u32;
    let choice = s.p8(b, panel::CHOICE) as u32;
    blend_palette(b, 0x25_c4c0 + 32 * choice, 0x91_c480 + 32 * player);
}

// ---------------------------------------------------------------------------------------------
// HP gauge
// ---------------------------------------------------------------------------------------------

/// Gauge tiles: start of a segment according to the remainder (per notch from 0 to 7), for four widths.
const BAR_TILES: [u32; 4] = [0x17a64, 0x17a76, 0x17a84, 0x17a8e];

/// 0x17a94: one gauge cell for the remainder `d0` (in pixels), between the bounds `d1` and `d2`.
fn bar_cell(brush: &mut Brush, attr: u16, d0: i16, d1: i16, d2: i16) {
    let (limit, table) = if d0 >= 7 {
        (8, BAR_TILES[0])
    } else if d0 >= 5 {
        (6, BAR_TILES[1])
    } else if d0 >= 3 {
        (4, BAR_TILES[2])
    } else {
        (2, BAR_TILES[3])
    };
    let tile = prog16(brush.b, table.wrapping_add_signed(2 * d1 as i32));
    let mut extent = d2.wrapping_sub(d1) as u16;
    let room = (limit as u16).wrapping_sub(d1 as u16);
    if extent > room {
        extent = room;
    }
    brush.cell(tile.wrapping_add(extent), attr);
    brush.skip_column_after_cell();
}

/// 0x179c2: the panel's HP gauge (one segment of 8 hit points per cell, a remainder, then empty).
pub fn hp_bar(b: &mut Cps2Bus, s: Slot) {
    if s.p16(b, panel::HP_BAR) == 0 {
        return;
    }
    let attr = 0x18 + s.index(b) as u16;
    let mut d0 = s.p16(b, panel::SHOWN_HP_MAX) as i16;
    let mut d1 = s.g16(b, player::HP) as i16;
    let mut d2 = s.p16(b, panel::SHOWN_HP) as i16;
    if narrow(b) {
        d0 = ((d0 as u16) >> 1) as i16;
        d1 = ((d1 as u16) >> 1) as i16;
        d2 = ((d2 as u16) >> 1) as i16;
    }
    let mut brush = zone_brush(b, s, zone::HP_BAR);
    // full segments
    while d1 > 8 {
        d1 -= 8;
        d0 -= 8;
        d2 -= 8;
        brush.cell(0x82c, attr);
        brush.skip_column_after_cell();
    }
    bar_cell(&mut brush, attr, d0, d1, d2);
    if d2 != d1 {
        d0 -= 8;
        d2 -= 8;
        if d2 < 0 {
            d0 += 8;
        } else if d0 != d2 {
            // recently lost HP segments
            loop {
                if d2 <= 8 {
                    bar_cell(&mut brush, attr, d0, 0, d2);
                    break;
                }
                d2 -= 8;
                d0 -= 8;
                brush.cell(0x808, attr);
                brush.skip_column_after_cell();
                if d0 == d2 {
                    break;
                }
            }
        }
    }
    // the rest of the gauge (maximum HP): empty segments
    loop {
        d0 -= 8;
        if d0 <= 0 {
            break;
        }
        bar_cell(&mut brush, attr, d0, 0, 0);
    }
    s.set_p8(brush.b, panel::HP_DIRTY, 0);
}

