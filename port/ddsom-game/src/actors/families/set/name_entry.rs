use super::*;
use crate::hw::mem::{rd16, rd32, rd8, wr16, wr32, wr8};
use crate::actors::families::set::tables as set_tables;
use crate::world::objects::Obj;

// --- video RAM and tables ---
const TILEMAP: u32 = 0x912000;
const PALETTE_BANK: u32 = 0x91c400;
const PALETTE_SOURCE: u32 = 0x25cd40;
const PALETTE_ENTRY: u32 = 0x20;
const KEEP_BRIGHTNESS: u32 = 0xf000f000;
/// Starting cell of the cursors (program space): two words per player and per mode.
const START_CELLS: u32 = 0xc9abe;
/// Width of each row of the grid (words, data space).
const ROW_WIDTH_NARROW: u32 = 0xcad74;
const ROW_WIDTH_WIDE: u32 = 0xcad80;
/// Default names: 8 bytes per player variant (+0x60 in the wide grid), data space.
const DEFAULT_NAMES: u32 = 0xc9daa;
/// Character of each cell (bytes, program space), index = column + 16 × row.
const GRID_NARROW: u32 = 0xcaacc;
const GRID_WIDE: u32 = 0xcab36;
/// Voiced vowel conversions (bytes, data space) for 0x9f and 0x9d.
const VOICED: u32 = 0xcabdc;
const SEMI_VOICED: u32 = 0xcac64;
/// Secret codes: character sequences (length byte, characters), end = negative byte.
const RESET_CODES: u32 = 0xcade6;
const SPECIAL_CODES: u32 = 0xcad8a;
const SPECIAL_REPLACEMENTS: u32 = 0xca5a6;
/// BCD value of each character (bytes, data space) and default check digit.
const CHAR_DIGITS: u32 = 0xcacec;
const DEFAULT_CHECK: u32 = 0xca6aa;
/// Tints of the special cell (bytes, program space).
const SPECIAL_TILES: u32 = 0xca83e;
/// Tile placement tables (words, program space), narrow grid then wide.
const NARROW_ROW_OFFSETS: u32 = 0xca894;
const NARROW_CELLS: u32 = 0xca8c8;
const WIDE_ROW_OFFSETS: u32 = 0xca9c8;
const WIDE_CELLS: u32 = 0xca9f2;
/// Positions of the end indicator (words, program space) and end records (longs).
const FINISH_POSITIONS: u32 = 0xca0e4;
const FINISH_RECORDS: u32 = 0xca1a4;
const FLAGS_ARRAY: u32 = 0xff2478;

// --- variables de A5 ---
const INPUT_HELD: u32 = 0x0;
const INPUT_PRESSED: u32 = 0xc;
const BUTTONS_MASK: u32 = 0x4;
const BUTTONS_OLD: u32 = 0xa;
const GAME_MODE: u32 = 0x34;
const WIDE_FLAG: u32 = 0x70;
const BLINK_FLAGS: u32 = 0x344;

// --- common fields of the board and the cursors ---
const WIDE: u32 = 0x13;
const COUNT_TYPED: u32 = 0x14;
const PAIRS: u32 = 0x15;
const COUNT_DONE: u32 = 0x16;
const COUNT_ACTIVE: u32 = 0x17;
const OWNER: u32 = 0x22;
const ACTIVE: u32 = 0x24;
const PLAYER: u32 = 0x2f;
// --- cursor fields ---
const LENGTH: u32 = 0x18;
const VOICED_PENDING: u32 = 0x19;
const DONE: u32 = 0x1a;
const NO_NAME: u32 = 0x1b;
const PREV_X: u32 = 0x1c;
const PREV_Y: u32 = 0x1e;
const REPEAT: u32 = 0x20;
const TYPED: u32 = 0x21;
const HELD_FRAMES: u32 = 0x2e;
const COL: u32 = 0x8;
const ROW: u32 = 0xc;
/// Player's character buffer: relative address, size.
const BUFFER: u32 = 0x118;
const EMPTY: u8 = 0xff;
const ENTER: u8 = 0x81;
const NAME_END: u8 = 0xf0;
// --- fields of a board overlap record ---
const REC_DIR: u32 = 0;
const REC_TIMER: u32 = 1;
const REC_COL: u32 = 2;
const REC_ROW: u32 = 4;
const REC_MASK: u32 = 6;
const RECORDS: u32 = 0x30;
const PREV_RECORDS: u32 = 0x40;
const SLAVES: u32 = 0x18;
const FREE_ON_NEXT: u32 = 0x6000000;
const GRID_COLS: u16 = 11;
const GRID_ROWS: u16 = 6;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Obj::VARIANT) != 0 {
        return cursor::run(x);
    }
    match x.u8(Obj::STATE) {
        0 => {
            board::init(x);
            board::update(x);
        }
        2 => board::update(x),
        _ => board::release(x),
    }
}

// --- aides communes -------------------------------------------------------------------------

/// High byte of another object's record designated by a word.
fn at<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> Obj {
    x.linked(off)
}

fn wide<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.u8(WIDE) != 0
}

/// 0xca85c: held keys and pressed keys of the frame for player `player`.
fn inputs<R: Bench>(x: &mut Cx<'_, R>, player: u32) -> (u8, u8) {
    (x.a5_u8(INPUT_HELD + player), x.a5_u8(INPUT_PRESSED + player))
}

/// 0xca8a0: address in the tile map and size (columns, rows - 1) of the cell (`col`,
/// `row`). Rend (adresse, D0 = taille).
fn cell_region<R: Bench>(x: &mut Cx<'_, R>, col: u16, row: u16) -> (u32, u16) {
    let (row_offsets, cells) = if wide(x) { (WIDE_ROW_OFFSETS, WIDE_CELLS) } else { (NARROW_ROW_OFFSETS, NARROW_CELLS) };
    let row_off = set_tables::pc_word(x.b, row_offsets.wrapping_add((row.wrapping_mul(2)) as i16 as i32 as u32));
    let index = col.wrapping_mul(4).wrapping_add(row_off);
    let at = |o: u32| cells.wrapping_add(o).wrapping_add(index as i16 as i32 as u32);
    let (offset, size) = (set_tables::pc_word(x.b, at(0)), set_tables::pc_word(x.b, at(2)));
    (TILEMAP + 2 + offset as i16 as i32 as u32, size)
}

/// 0xca86e: fills region `size` (high byte: rows - 1, low byte: columns - 1) with
/// tile `tile`, one word every 4 bytes.
fn fill_tiles<R: Bench>(x: &mut Cx<'_, R>, mut a1: u32, size: u16, tile: u16) {
    let cols = size & 0xff;
    for _ in 0..=(size >> 8) as u32 {
        let mut advance = 0;
        for _ in 0..=cols as u32 {
            wr16(x.b, a1, tile);
            a1 += 4;
            advance += 4;
        }
        a1 = a1 + 0x80 - advance;
    }
}

/// 0xca81a: redraws cell (`col`, `row`): tile 0x1c, or a tint from the table when it
/// is in the special column.
fn redraw_cell<R: Bench>(x: &mut Cx<'_, R>, col: u16, row: u16) {
    let (a1, size) = cell_region(x, col, row);
    let mut tile: u16 = 0x1c;
    if !wide(x) {
        if col == 0xa {
            tile = set_tables::pc_byte(x.b, SPECIAL_TILES.wrapping_add(row.wrapping_sub(1) as i16 as i32 as u32)) as u16;
        }
    } else if row == 4 && (col as i16) >= 3 {
        tile = set_tables::pc_byte(x.b, SPECIAL_TILES.wrapping_add(row.wrapping_sub(3) as i16 as i32 as u32)) as u16;
    }
    fill_tiles(x, a1, size, tile);
}

// --- the board (variant 0) -------------------------------------------------------------

mod board;

// --- a player's cursor (non-zero variants) --------------------------------------------

mod cursor;
