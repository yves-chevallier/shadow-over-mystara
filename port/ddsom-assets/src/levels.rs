//! Scenery maps: `assets/levels.json`, see docs/assets.md (scroll 2 and 3, plus the fixed images of
//! scroll 1, `Scroll1`).
//!
//! Each layer (scroll 2: 16-pixel cells; scroll 3: 32-pixel cells) is made of *strips*
//! of 256x256 pixels. A strip is a grid of 2x2-cell *blocks*; a block gives the tile code
//! and the attributes of its four cells. For each scene, a *strip table* gives the grid of
//! strip numbers of the map.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::json::hex;

/// 2x2-cell block: (code, attributes) of cells (0,0), (0,1), (1,0), (1,1) (x then y).
pub type Block = [u16; 8];

/// Bank of blocks.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Bank {
    #[serde(with = "hex")]
    pub rom: u32,
    pub blocks: Vec<Block>,
}

/// Strip table of one or several scenes.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct StripTable {
    /// ROM address of the header word (width).
    #[serde(with = "hex")]
    pub rom: u32,
    /// Rows in ROM order, from the top of the map downwards; each has the width of the
    /// header (in scroll 2, its last word is a sentinel, 0xfe).
    pub rows: Vec<Vec<u16>>,
    /// Row with ordinate 0, the one the scene pointer designates (the following ones are below 0).
    pub zero: usize,
}

impl StripTable {
    pub fn width(&self) -> usize {
        self.rows[0].len()
    }

    /// Value of the scene pointer that designates this table.
    pub fn pointer(&self) -> u32 {
        self.rom + 2 + (2 * self.width() * self.zero) as u32
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Layer {
    pub name: String,
    /// Side of a cell in pixels (16 or 32); a strip is `256 / case` cells wide.
    pub case: u16,
    /// Added to a cell code to get the layer's tile (0x1800 or 0x1f00); palette base.
    pub tile_offset: u16,
    pub palette_base: u16,
    /// Filler strip numbers, never on screen (undefined).
    pub fillers: Vec<u16>,
    /// ROM table of block banks (one bank per group of 256 strips).
    #[serde(with = "hex")]
    pub banks_rom: u32,
    pub banks: Vec<Bank>,
    /// Strips: block numbers, column by column (8x8 in scroll 2, 4x4 in scroll 3).
    #[serde(with = "hex")]
    pub strips_rom: u32,
    pub strips: Vec<Vec<u16>>,
    /// ROM table of scene pointers (program space), one per scene 0..114.
    #[serde(with = "hex")]
    pub scenes_rom: u32,
    /// Strip table of each scene (index into `tables`).
    pub scenes: Vec<usize>,
    pub tables: Vec<StripTable>,
}

impl Layer {
    /// Cells per side of a strip.
    pub fn strip_cases(&self) -> usize {
        256 / self.case as usize
    }

    /// Bank of strip `strip`.
    pub fn bank_of(&self, strip: usize) -> usize {
        strip / 256 % if self.case == 16 { 4 } else { 16 }
    }

    /// Cell (cx, cy) of strip `strip`: (tile, attributes), tile already shifted by `tile_offset`.
    pub fn case_at(&self, strip: usize, cx: usize, cy: usize) -> (u16, u16) {
        let blocks_per_side = self.strip_cases() / 2;
        let index = self.strips[strip][(cx / 2) * blocks_per_side + cy / 2] as usize;
        let block = &self.banks[self.bank_of(strip)].blocks[index];
        let k = 2 * ((cx % 2) * 2 + cy % 2);
        (block[k].wrapping_add(self.tile_offset), block[k + 1])
    }
}

/// Scroll 1 (8-pixel cells): fixed 256x256-pixel images that 0x260c copies into the layer
/// (title screen, foregrounds...). An image is a grid of 8x8 blocks of 4x4 cells.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Scroll1 {
    /// Added (modulo 0x10000) to a cell code to get the 8x8 tile.
    pub tile_offset: u16,
    /// Image grids: 64 block numbers, column by column (`(cx / 4) * 8 + cy / 4`), for
    /// D0 = 0, 1...; the following grids of the region (up to `blocks_rom`) are null.
    #[serde(with = "hex")]
    pub grids_rom: u32,
    pub grids: Vec<Vec<u16>>,
    /// Blocks: 16 cells (code, attributes), column by column (`(cx % 4) * 4 + cy % 4`).
    #[serde(with = "hex")]
    pub blocks_rom: u32,
    pub blocks: Vec<Vec<u16>>,
}

impl Scroll1 {
    /// Cell (cx, cy), 0 to 31, of image `grid`: (tile, attributes).
    pub fn case_at(&self, grid: usize, cx: usize, cy: usize) -> (u16, u16) {
        let block = &self.blocks[self.grids[grid][(cx / 4) * 8 + cy / 4] as usize];
        let k = 2 * ((cx % 4) * 4 + cy % 4);
        (block[k].wrapping_add(self.tile_offset), block[k + 1])
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Levels {
    pub layers: Vec<Layer>,
    pub scroll1: Scroll1,
}

impl Levels {
    pub fn load(path: &Path) -> io::Result<Self> {
        crate::json::load(path)
    }
}
