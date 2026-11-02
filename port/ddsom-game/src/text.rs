//! Texts and erasures on the 8x8 character layer (tilemap 0x912000).
//!
//! The layer is stored by columns: one cell = 2 words (tile, attributes), the next cell of the
//! same column is 4 bytes further, the next column 0x80 bytes further.
//! A text is described in ROM by: column, row, attributes, then the characters and control
//! codes. All these routines return `A4` positioned after the descriptor (callers
//! chain the texts) and preserve the other registers.

use cps2::Cps2Bus;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::mem::{rd16, rd8, rts, wr16};
use crate::{regs, Native};

const LAYER: u32 = 0x912000;
const ROW: u32 = 4;
const COLUMN: u32 = 0x80;
const BLANK: u16 = 0x20;

/// Sequential reader of the descriptor pointed to by A4.
struct Script<'a> {
    b: &'a mut Cps2Bus,
    at: u32,
}

impl Script<'_> {
    fn byte(&mut self) -> u8 {
        let v = rd8(self.b, self.at);
        self.at += 1;
        v
    }
    fn word(&mut self) -> u16 {
        let v = rd16(self.b, self.at);
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

fn put(b: &mut Cps2Bus, at: u32, code: u16, attr: u16) {
    wr16(b, at, code);
    wr16(b, at + 2, attr);
}

/// End of routine: A4 advanced, flags of the final comparison (equality) and X left by the
/// last operation that modifies it.
fn finish(c: &mut Cpu, b: &mut Cps2Bus, a4: u32, x: bool) {
    c.a[4] = a4;
    c.sr = (c.sr & !(SR_X | SR_N | SR_V | SR_C)) | SR_Z | if x { SR_X } else { 0 };
    rts(c, b);
}

/// The three variants of the simple text.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// 0x0d5e: '@' end, '/' next line, 0xff n: n spaces.
    Text,
    /// 0x0d32: same descriptors, each character becomes a space (the attributes have no effect).
    Clear,
    /// 0x0dca: like the text, but '_' goes to the next line and spaces leave the background intact.
    Overlay,
}

/// Returns the end of the descriptor and the X flag left by the original.
fn text(b: &mut Cps2Bus, descriptor: u32, kind: Kind) -> (u32, bool) {
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
pub fn print(b: &mut Cps2Bus, descriptor: u32) -> u32 {
    text(b, descriptor, Kind::Text).0
}

/// 0x0d32: erases the text described at `descriptor`; returns the end of the descriptor.
pub fn erase(b: &mut Cps2Bus, descriptor: u32) -> u32 {
    text(b, descriptor, Kind::Clear).0
}

/// 0x0dca: overlay text; returns the end of the descriptor.
pub fn print_over(b: &mut Cps2Bus, descriptor: u32) -> u32 {
    text(b, descriptor, Kind::Overlay).0
}

fn native_text(c: &mut Cpu, b: &mut Cps2Bus, kind: Kind) {
    let (end, x) = text(b, c.a[4], kind);
    finish(c, b, end, x);
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
pub fn print_tall(b: &mut Cps2Bus, descriptor: u32) -> u32 {
    let mut s = Script { b, at: descriptor };
    let (col, row) = (s.byte(), s.byte());
    let attr = s.word();
    tall_body(&mut s, cell(col, row), attr);
    s.at
}

/// 0x1234: tall text at position (`col`, `row`) (D5, D6 of the original); the descriptor starts at the
/// attributes. Returns the end of the descriptor.
pub fn print_tall_at(b: &mut Cps2Bus, descriptor: u32, col: u8, row: u8) -> u32 {
    let mut s = Script { b, at: descriptor };
    let attr = s.word();
    tall_body(&mut s, cell(col, row), attr);
    s.at
}

fn print_tall_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let end = print_tall(b, c.a[4]);
    finish(c, b, end, false);
}

fn print_tall_at_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let end = print_tall_at(b, c.a[4], c.d[5] as u8, c.d[6] as u8);
    finish(c, b, end, false);
}

/// 2x2-cell blocks: '@@' end, '##' skips two columns, '//' two lines down, 0xffff n: n
/// empty blocks (tile 0x20). Any other word `t` draws a block: tiles `t, t+1` then `t+0x10, t+0x11`
/// below with the attributes of the descriptor (negative: palette `+0x2b` of object A0) for 0x0f38;
/// empty block (tile 0x2fc, attributes 0) for 0x0f4a.
fn blocks(b: &mut Cps2Bus, descriptor: u32, object: u32, drawn: bool) -> (u32, bool) {
    let attr = rd16(b, descriptor + 2);
    let attr = if attr & 0x8000 != 0 { rd8(b, object + 0x2b) as u16 } else { attr };
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
pub fn clear_blocks(b: &mut Cps2Bus, descriptor: u32, object: u32) -> u32 {
    blocks(b, descriptor, object, false).0
}

/// 0x0f38: draws 2x2-cell blocks (negative attributes: palette `+0x2b` of `object`); returns the end of the
/// descriptor.
pub fn draw_blocks(b: &mut Cps2Bus, descriptor: u32, object: u32) -> u32 {
    blocks(b, descriptor, object, true).0
}

fn blocks_native(c: &mut Cpu, b: &mut Cps2Bus, drawn: bool) {
    let (end, x) = blocks(b, c.a[4], c.a[0], drawn);
    finish(c, b, end, x);
}

/// Erases a rectangle of the layer (column, row, width, height) with spaces. A row
/// of at least 0x20 is brought back into the layer.
pub fn clear_cells(b: &mut Cps2Bus, col: u8, row: u8, width: u8, height: u8) {
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
pub fn clear_box(b: &mut Cps2Bus, descriptor: u32) -> u32 {
    clear_box_height(b, descriptor).0
}

fn clear_box_height(b: &mut Cps2Bus, descriptor: u32) -> (u32, u8) {
    let mut s = Script { b, at: descriptor };
    let col = s.byte();
    let row = s.byte();
    let width = s.byte();
    let height = rd8(s.b, s.at);
    let end = s.at;
    clear_cells(b, col, row, width, height);
    (end, height)
}

fn clear_box_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let (end, height) = clear_box_height(b, c.a[4]);
    c.a[4] = end;
    // flags: last move.w #0 (Z); X = carry of "height - 1"
    let x = if height == 0 { SR_X } else { 0 };
    c.sr = (c.sr & !(SR_X | SR_N | SR_V | SR_C)) | SR_Z | x;
    rts(c, b);
}

const fn native(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus)) -> Native {
    Native { entry, name, run: crate::Body::Processeur(run), touches_sound: false, outputs: regs::a(4) | regs::CCR }
}

pub const PRINT: Native = native(0x0d5e, "text_print", |c, b| native_text(c, b, Kind::Text));
pub const ERASE: Native = native(0x0d32, "text_erase", |c, b| native_text(c, b, Kind::Clear));
pub const PRINT_OVER: Native = native(0x0dca, "text_print_over", |c, b| native_text(c, b, Kind::Overlay));
pub const PRINT_TALL: Native = native(0x1136, "text_print_tall", print_tall_native);
pub const PRINT_TALL_AT: Native = native(0x1234, "text_print_tall_at", print_tall_at_native);
pub const CLEAR_BLOCKS: Native = native(0x0f4a, "text_clear_blocks", |c, b| blocks_native(c, b, false));
pub const DRAW_BLOCKS: Native = native(0x0f38, "text_draw_blocks", |c, b| blocks_native(c, b, true));
pub const CLEAR_BOX: Native = native(0x1324, "text_clear_box", clear_box_native);
