//! Texts and erasures on the 8x8 character layer (tilemap 0x912000).
//!
//! The layer is stored by columns: one cell = 2 words (tile, attributes), the next cell of the
//! same column is 4 bytes further, the next column 0x80 bytes further.
//! A text is described in ROM by: column, row, attributes, then the characters and control
//! codes. All these routines return `A4` positioned after the descriptor (callers
//! chain the texts) and preserve the other registers.

use crate::world::World;

use crate::hw::video::{Cell, TEXT};
use crate::world::objects::Obj;
use crate::world::screen_tables as tables;

const LAYER: u32 = TEXT.base;
const ROW: u32 = TEXT.row;
const COLUMN: u32 = TEXT.column;
const BLANK: u16 = 0x20;
/// Palette of an object (`+0x2b`), taken by the blocks whose attributes are negative.
const OBJECT_PALETTE: u32 = 0x2b;

/// Sequential reader of the descriptor pointed to by A4.
struct Script<'a> {
    b: &'a mut World,
    at: u32,
}

impl Script<'_> {
    fn byte(&mut self) -> u8 {
        let v = tables::byte(self.b, self.at);
        self.at += 1;
        v
    }
    fn word(&mut self) -> u16 {
        let v = tables::word(self.b, self.at);
        self.at += 2;
        v
    }
}

/// Cell (column, row) of the layer; signed positions, with the 16-bit computation of the original
/// (ext.l then shift of the low word only). Also returns the carry of the final addition
/// (X flag left by the original).
fn cell_signed(col: u8, row: u8) -> (u32, bool) {
    let shift = |v: u8, n: u32| {
        let x = v as i8 as i32 as u32;
        (x & 0xffff_0000) | ((x << n) & 0xffff)
    };
    let (sum, carry) = shift(col, 7).overflowing_add(shift(row, 2));
    (LAYER.wrapping_add(sum) & 0xff_ffff, carry)
}

fn cell(col: u8, row: u8) -> u32 {
    LAYER + COLUMN * col as u32 + ROW * row as u32
}

fn put(b: &mut World, at: u32, code: u16, attr: u16) {
    Cell(at).put(b, code, attr);
}

/// The three variants of the simple text.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    /// 0x0d5e: '@' end, '/' next line, 0xff n: n spaces.
    Text,
    /// 0x0d32: same descriptors, each character becomes a space (the attributes have no effect).
    Clear,
    /// 0x0dca: like the text, but '_' goes to the next line and spaces leave the background intact.
    Overlay,
}

/// Returns the end of the descriptor and the X flag left by the original.
pub(crate) fn text(b: &mut World, descriptor: u32, kind: Kind) -> (u32, bool) {
    let mut s = Script { b, at: descriptor };
    let (col, row) = (s.byte(), s.byte());
    let attr = s.word();
    let (mut line, mut x) = cell_signed(col, row);
    let mut at = line;
    let newline = if kind == Kind::Overlay { b'_' } else { b'/' };
    loop {
        match s.byte() {
            b'@' => break,
            0xff => {
                let n = s.byte();
                x = n == 0; // subq.b #1
                for _ in 0..n.wrapping_sub(1) as u16 + 1 {
                    put(s.b, at, BLANK, 0);
                    at += COLUMN;
                }
            }
            ch if ch == newline => {
                line += ROW;
                at = line;
            }
            b' ' if kind == Kind::Overlay => at += COLUMN,
            ch => {
                let (code, attr) = if kind == Kind::Clear { (BLANK, 0) } else { (ch as u16, attr) };
                put(s.b, at, code, attr);
                at += COLUMN;
            }
        }
    }
    (s.at, x)
}

/// 0x0d5e: writes the text described at `descriptor` (column, row, attributes, characters); returns the address
/// following the descriptor (A4 of the original: callers chain the texts).
pub fn print(b: &mut World, descriptor: u32) -> u32 {
    text(b, descriptor, Kind::Text).0
}

/// 0x0d32: erases the text described at `descriptor`; returns the end of the descriptor.
pub fn erase(b: &mut World, descriptor: u32) -> u32 {
    text(b, descriptor, Kind::Clear).0
}

/// 0x0dca: overlay text; returns the end of the descriptor.
pub fn print_over(b: &mut World, descriptor: u32) -> u32 {
    text(b, descriptor, Kind::Overlay).0
}

/// Common body of the tall texts (2 tiles one under the other, codes on a word):
/// '@@' end, '##' column skipped, '/@' next line, '//' two lines down.
fn tall_body(s: &mut Script, start: u32, attr: u16) {
    let mut line = start;
    let mut at = line;
    loop {
        match s.word() {
            0x2323 => at += COLUMN,
            0x4040 => return,
            0x2f40 => {
                line += ROW;
                at = line;
            }
            0x2f2f => {
                line += 2 * ROW;
                at = line;
            }
            ch => {
                put(s.b, at, ch, attr);
                put(s.b, at + ROW, ch.wrapping_add(0x10), attr);
                at += COLUMN;
            }
        }
    }
}

/// 0x1136: tall text at the position of the descriptor; returns the end of the descriptor.
pub fn print_tall(b: &mut World, descriptor: u32) -> u32 {
    let mut s = Script { b, at: descriptor };
    let (col, row) = (s.byte(), s.byte());
    let attr = s.word();
    tall_body(&mut s, cell(col, row), attr);
    s.at
}

/// 0x1234: tall text at position (`col`, `row`) (D5, D6 of the original); the descriptor starts at the
/// attributes. Returns the end of the descriptor.
pub fn print_tall_at(b: &mut World, descriptor: u32, col: u8, row: u8) -> u32 {
    let mut s = Script { b, at: descriptor };
    let attr = s.word();
    tall_body(&mut s, cell(col, row), attr);
    s.at
}

/// 2x2-cell blocks: '@@' end, '##' skips two columns, '//' two lines down, 0xffff n: n
/// empty blocks (tile 0x20). Any other word `t` draws a block: tiles `t, t+1` then `t+0x10, t+0x11`
/// below with the attributes of the descriptor (negative: palette `+0x2b` of object A0) for 0x0f38;
/// empty block (tile 0x2fc, attributes 0) for 0x0f4a.
pub(crate) fn blocks(b: &mut World, descriptor: u32, object: u32, drawn: bool) -> (u32, bool) {
    let attr = tables::word(b, descriptor + 2);
    let attr = if attr & 0x8000 != 0 { Obj(object).u8(b, OBJECT_PALETTE) as u16 } else { attr };
    let mut s = Script { b, at: descriptor };
    let (col, row) = (s.byte(), s.byte());
    s.word();
    let mut line = cell(col, row);
    let mut at = line;
    let mut x = false;
    loop {
        match s.word() {
            0x2323 => at += 2 * COLUMN,
            0x4040 => break,
            0xffff => {
                let n = s.word();
                for _ in 0..n.wrapping_sub(1) as u32 + 1 {
                    for (dc, dr) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
                        put(s.b, at + dc * COLUMN + dr * ROW, BLANK, 0);
                    }
                    at += 2 * COLUMN;
                }
                x = false; // add.w D4,D1 (D4 = 0)
            }
            0x2f2f => {
                line += 2 * ROW;
                at = line;
            }
            tile => {
                let (code, attr) = if drawn { (tile, attr) } else { (0x2fc, 0) };
                put(s.b, at, code, attr);
                put(s.b, at + ROW, code.wrapping_add(0x10), attr);
                put(s.b, at + COLUMN, code.wrapping_add(1), attr);
                put(s.b, at + COLUMN + ROW, code.wrapping_add(0x11), attr);
                // carry of the last addi.w #0x10
                x = code.wrapping_add(1).checked_add(0x10).is_none();
                at += 2 * COLUMN;
            }
        }
    }
    (s.at, x)
}

/// 0x0f4a: erases 2x2-cell blocks (`object`: A0 of the original, read only if the attributes of the
/// descriptor are negative); returns the end of the descriptor.
pub fn clear_blocks(b: &mut World, descriptor: u32, object: u32) -> u32 {
    blocks(b, descriptor, object, false).0
}

/// 0x0f38: draws 2x2-cell blocks (negative attributes: palette `+0x2b` of `object`); returns the end of the
/// descriptor.
pub fn draw_blocks(b: &mut World, descriptor: u32, object: u32) -> u32 {
    blocks(b, descriptor, object, true).0
}

/// Erases a rectangle of the layer (column, row, width, height) with spaces. A row
/// of at least 0x20 is brought back into the layer.
pub fn clear_cells(b: &mut World, col: u8, row: u8, width: u8, height: u8) {
    let row = if row >= 0x20 { row - 0x20 } else { row };
    let origin = LAYER + ((col as u32) << 7) + ((row as u32) << 2);
    for x in 0..width.wrapping_sub(1) as u32 + 1 {
        for y in 0..height.wrapping_sub(1) as u32 + 1 {
            put(b, origin + x * COLUMN + y * ROW, BLANK, 0);
        }
    }
}

/// 0x1324: erases the rectangle described at `descriptor` (column, row, width, height) with spaces. Returns
/// the end of the descriptor, which only advances by 3 bytes (the height is read without post-increment).
pub fn clear_box(b: &mut World, descriptor: u32) -> u32 {
    clear_box_height(b, descriptor).0
}

pub(crate) fn clear_box_height(b: &mut World, descriptor: u32) -> (u32, u8) {
    let mut s = Script { b, at: descriptor };
    let col = s.byte();
    let row = s.byte();
    let width = s.byte();
    let height = tables::byte(s.b, s.at);
    let end = s.at;
    clear_cells(b, col, row, width, height);
    (end, height)
}
