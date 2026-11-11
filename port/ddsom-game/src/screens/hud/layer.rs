//! Character layer (8x8 tilemap at `0x912000`) as seen by the player panels.
//!
//! A cell is a long (tile, attributes). The layer is stored by columns: the next cell of a
//! column is 4 bytes further, the next column 0x80 bytes further. The panels are
//! rectangles of cells whose origin depends on the player, the zone and the screen layout.

use crate::world::World;

use super::var;
use crate::hw::video::{Cell, TEXT};
use crate::world::screen_tables::{self as tables, Rom};

pub const LAYER: u32 = TEXT.base;
pub const COLUMN: u32 = TEXT.column;
pub const CELL: u32 = TEXT.row;
/// Empty cell: tile 0x20 (space), zero attributes.
pub const EMPTY_TILE: u16 = 0x20;

/// Empties the cell at `at`.
pub fn clear_cell(b: &mut World, at: u32) {
    Cell(at).put(b, EMPTY_TILE, 0);
}

/// Brush: a write pointer that advances cell by cell, like `(A1)+` in the original.
pub struct Brush<'a> {
    pub b: &'a mut World,
    pub at: u32,
}

impl<'a> Brush<'a> {
    pub fn new(b: &'a mut World, at: u32) -> Self {
        Brush { b, at }
    }

    /// Writes a cell (tile, attributes) and moves on to the next one in the column.
    pub fn cell(&mut self, tile: u16, attr: u16) {
        Cell(self.at).put(self.b, tile, attr);
        self.at += CELL;
    }

    /// Moves to the next column, at the same height as the start of a series of `written` cells.
    pub fn next_column(&mut self, written: u32) {
        self.at = self.at + COLUMN - written * CELL;
    }

    /// Advances one column from the current cell (already at the end of its cell).
    pub fn skip_column_after_cell(&mut self) {
        self.at += COLUMN - CELL;
    }

    /// Vertical series of three cells read from a table (consecutive tiles), common attributes, then
    /// next column: the unit of the large waiting images.
    pub fn column3(&mut self, tiles: [u16; 3], attr: u16) {
        for tile in tiles {
            self.cell(tile, attr);
        }
        self.next_column(3);
    }
}

/// Origin of a panel zone for player index `player`: program ROM table (read PC-relative,
/// hence decrypted), one row of 8 words per zone, four positions per layout; the
/// two-column layout (`A5+0x344` odd) uses the second half of the row.
pub fn origin(b: &mut World, player: u8, zone: u16) -> u32 {
    const ORIGINS: Rom = Rom::at(0x172b4);
    let narrow = var::LAYOUT.get(b) & 1 != 0;
    let index = 2 * player as u32 + 16 * zone as u32 + if narrow { 8 } else { 0 };
    LAYER.wrapping_add(ORIGINS.pc_word(b, index) as i16 as i32 as u32)
}

/// Series of columns of three cells read from the ROM (`n` columns, three words per column).
pub fn columns(brush: &mut Brush, table: u32, n: u32, attr: u16) {
    for i in 0..n {
        let tiles = [0, 1, 2].map(|k| tables::word(brush.b, table + 6 * i + 2 * k));
        brush.column3(tiles, attr);
    }
}
