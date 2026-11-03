//! Character layer (8x8 cells, `0x912000` for the visible part) as seen by the end-of-level screens (countdown, bonus).
//!
//! A cell is a long (tile, attributes); the layer is stored by columns: the next cell of a column
//! is 4 bytes further, the next column 0x80 bytes further. The digits and letters of these screens
//! are two cells high (the bottom tile is the top one + 16), the big digits two wider still.

use cps2::Cps2Bus;

use crate::hw::video::{Cell, TEXT};
use crate::screens::tables;

pub const LAYER: u32 = TEXT.base;
pub const COLUMN: u32 = TEXT.column;
/// Empty cell: tile 0x20 (space), null attributes.
pub const BLANK_TILE: u16 = 0x20;

/// First tiles of digits 0 to 7 and 8, 9 (two rows of 16 tiles further).
const DIGIT_0: u16 = 472;
const DIGIT_8: u16 = 496;
/// Colon of a duration, and second (″ sign) of the seconds count.
pub const COLON: u16 = 766;
pub const SECONDS: u16 = 767;
/// Big digit tiles (words, table read relative to the PC).
const BIG_DIGITS: crate::screens::tables::Rom = crate::screens::tables::Rom::at(0xf6e74);

/// A cell two high: `tile` on top, `tile + 16` below.
pub fn tall(b: &mut Cps2Bus, at: u32, tile: u16, attr: u16) {
    Cell(at).put(b, tile, attr);
    Cell(at + 4).put(b, tile.wrapping_add(16), attr);
}

/// Two empty cells one under the other.
pub fn blank(b: &mut Cps2Bus, at: u32) {
    Cell(at).put(b, BLANK_TILE, 0);
    Cell(at + 4).put(b, BLANK_TILE, 0);
}

/// `n` columns of two empty cells starting at `at`; returns the next column.
pub fn blank_columns(b: &mut Cps2Bus, at: u32, n: u32) -> u32 {
    for k in 0..n {
        blank(b, at + k * COLUMN);
    }
    at + n * COLUMN
}

/// A digit (low nibble of `d`) two cells high (0x0f6d94).
pub fn digit(b: &mut Cps2Bus, at: u32, d: u8, attr: u16) {
    let d = (d & 15) as u16;
    let tile = if DIGIT_0 + d < 480 { DIGIT_0 + d } else { DIGIT_8 - 8 + d };
    tall(b, at, tile, attr);
}

/// Writing a number digit by digit, one column per digit; as long as no non-zero digit has been
/// written (`lead` false), leading zeros are empty cells (0x0f6b72).
pub struct Number {
    pub at: u32,
    pub lead: bool,
    pub attr: u16,
}

impl Number {
    pub fn new(at: u32, attr: u16) -> Self {
        Number { at, lead: false, attr }
    }
    pub fn digit(&mut self, b: &mut Cps2Bus, d: u8) {
        let d = d & 15;
        if d == 0 && !self.lead {
            blank(b, self.at);
        } else {
            self.lead = true;
            digit(b, self.at, d, self.attr);
        }
        self.at += COLUMN;
    }
    /// The two digits of a BCD byte (0x0f6b66).
    pub fn byte(&mut self, b: &mut Cps2Bus, v: u8) {
        self.digit(b, v >> 4);
        self.digit(b, v);
    }
    /// Last digit, always written.
    pub fn last(&mut self, b: &mut Cps2Bus, d: u8) {
        self.lead = true;
        self.digit(b, d);
    }
}

/// The six BCD digits of the last three bytes of the long at `src` (RAM or ROM table), leading zeros
/// erased, the last always written (0x0f6b40).
pub fn number6(b: &mut Cps2Bus, at: u32, src: u32, attr: u16) {
    let mut n = Number::new(at, attr);
    let (high, middle) = (tables::byte(b, src + 1), tables::byte(b, src + 2));
    n.byte(b, high);
    n.byte(b, middle);
    let low = tables::byte(b, src + 3);
    n.digit(b, low >> 4);
    n.last(b, low);
}

/// A `m:ss` duration (BCD word 0x0mss, the tens-of-minutes digit omitted if zero) (0x0f6d60).
pub fn duration(b: &mut Cps2Bus, at: u32, t: u16) {
    let nib = |k: u32| (t >> (12 - 4 * k)) as u8 & 15;
    if nib(0) != 0 {
        digit(b, at, nib(0), 0);
    }
    digit(b, at + COLUMN, nib(1), 0);
    tall(b, at + 2 * COLUMN, COLON, 0);
    digit(b, at + 3 * COLUMN, nib(2), 0);
    digit(b, at + 4 * COLUMN, nib(3), 0);
}

/// A big digit (two columns of two cells), or two columns skipped for a leading zero (0x0f6e2e).
pub fn big_digit(b: &mut Cps2Bus, n: &mut Number, d: u8) {
    let d = d & 15;
    if d != 0 || n.lead {
        n.lead = true;
        let tile = BIG_DIGITS.pc_word(b, 2 * d as u32);
        glyph(b, n.at, tile, n.attr);
    }
    n.at += 2 * COLUMN;
}

/// A two-column character: `tile`, `tile + 16` then `tile + 1`, `tile + 17`.
pub fn glyph(b: &mut Cps2Bus, at: u32, tile: u16, attr: u16) {
    tall(b, at, tile, attr);
    tall(b, at + COLUMN, tile.wrapping_add(1), attr);
}

/// Text in two-column characters (0x0f6f90): an attributes word then tiles; 0 ends, 0xffff
/// re-reads attributes, 0xfffe leaves an empty column.
pub fn glyphs(b: &mut Cps2Bus, mut at: u32, mut src: u32) -> u32 {
    let mut attr = tables::word(b, src);
    src += 2;
    loop {
        match tables::word(b, src) {
            0 => return at,
            0xffff => {
                attr = tables::word(b, src + 2);
                src += 4;
            }
            0xfffe => {
                src += 2;
                blank(b, at);
                at += COLUMN;
            }
            tile => {
                src += 2;
                glyph(b, at, tile, attr);
                at += 2 * COLUMN;
            }
        }
    }
}

/// Column of cells from a word table (tile pairs top/bottom, 0xffff ends), attributes 2 (0x0f7262).
pub fn label(b: &mut Cps2Bus, mut at: u32, mut src: u32) {
    while tables::word(b, src) != 0xffff {
        let (top, bottom) = (tables::word(b, src), tables::word(b, src + 2));
        Cell(at).put(b, top, 2);
        Cell(at + 4).put(b, bottom, 2);
        src += 4;
        at += COLUMN;
    }
}
