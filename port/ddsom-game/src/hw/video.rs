//! Graphics memory and video registers, as the game addresses them (docs/video.md, "Carte graphique").
//!
//! * graphics RAM (0x900000-0x92ffff): the tilemaps of the three scroll layers ([`Tilemap`], [`Cell`]), the
//!   palettes ([`PALETTES`]) and the row scroll table, all words ([`Vram`]);
//! * object RAM (0x708000, the sprite list: [`OBJECTS`]) and the object output control (0x400000-0x40000b);
//! * the CPS-A and CPS-B registers ([`Reg`], [`regs`]).
//!
//! The game places its tables at fixed addresses (layer 1 at 0x910000, the character layer at 0x912000, layer 2 at
//! 0x914000, layer 3 at 0x918000, the palettes at 0x91c000), so the places stay addresses here, typed by what they
//! hold: the screens write a cell or a register, never a bus address.

use cps2::Cps2Bus;

use crate::mem::{rd16, rd32, wr16, wr32};

/// A word of the graphics RAM, of the object RAM or of the object output control.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Vram(pub u32);

impl Vram {
    /// The word `n` bytes further.
    pub const fn offset(self, n: u32) -> Vram {
        Vram(self.0.wrapping_add(n))
    }
    pub fn get(self, b: &mut Cps2Bus) -> u16 {
        rd16(b, self.0)
    }
    pub fn set(self, b: &mut Cps2Bus, v: u16) {
        wr16(b, self.0, v)
    }
    /// Two words read together (a cell, a palette pair...).
    pub fn long(self, b: &mut Cps2Bus) -> u32 {
        rd32(b, self.0)
    }
    pub fn set_long(self, b: &mut Cps2Bus, v: u32) {
        wr32(b, self.0, v)
    }
}

/// A cell of a tilemap: tile number (word), then attributes (word: palette in bits 0 to 4, mirrors in 5 and 6...).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Cell(pub u32);

impl Cell {
    /// The cell `n` bytes further (4 per cell).
    pub const fn offset(self, n: u32) -> Cell {
        Cell(self.0.wrapping_add(n))
    }
    /// Writes the tile and its attributes.
    pub fn put(self, b: &mut Cps2Bus, tile: u16, attr: u16) {
        wr16(b, self.0, tile);
        wr16(b, self.0.wrapping_add(2), attr);
    }
    pub fn tile(self, b: &mut Cps2Bus) -> u16 {
        rd16(b, self.0)
    }
    pub fn set_tile(self, b: &mut Cps2Bus, tile: u16) {
        wr16(b, self.0, tile)
    }
    pub fn attr(self, b: &mut Cps2Bus) -> u16 {
        rd16(b, self.0.wrapping_add(2))
    }
    pub fn set_attr(self, b: &mut Cps2Bus, attr: u16) {
        wr16(b, self.0.wrapping_add(2), attr)
    }
    /// Tile and attributes as one long (tile in the high word).
    pub fn long(self, b: &mut Cps2Bus) -> u32 {
        rd32(b, self.0)
    }
    pub fn set_long(self, b: &mut Cps2Bus, v: u32) {
        wr32(b, self.0, v)
    }
}

/// A tilemap as the game places it: base address, distance between two columns and between two rows.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Tilemap {
    pub base: u32,
    pub column: u32,
    pub row: u32,
}

impl Tilemap {
    /// Cell (`col`, `row`).
    pub const fn cell(self, col: u32, row: u32) -> Cell {
        Cell(self.base + self.column * col + self.row * row)
    }
    /// Cell at a byte offset from the base (offsets read from the tables of the original).
    pub const fn at(self, offset: u32) -> Cell {
        Cell(self.base.wrapping_add(offset))
    }
}

/// Layer 1 (8x8 tiles), stored by columns: the next cell of a column is 4 bytes further, the next column 0x80 bytes
/// further.
pub const SCROLL1: Tilemap = Tilemap { base: 0x910000, column: 0x80, row: 4 };
/// The character layer: the visible window of layer 1 where the texts, panels and digits are written.
pub const TEXT: Tilemap = Tilemap { base: 0x912000, column: 0x80, row: 4 };
/// Layer 2 (16x16 tiles) and layer 3 (32x32 tiles): the screens load blocks of cells there (`video::load_grid`).
pub const SCROLL2: Vram = Vram(0x914000);
pub const SCROLL3: Vram = Vram(0x918000);
/// The palettes: four pages of 0x400 bytes (32 palettes of 16 colors, one word per color).
pub const PALETTES: Vram = Vram(0x91c000);
/// End of the graphics RAM used by the game as a scratch area (sprite tables of the title, cleared by the screens).
pub const SCRATCH: Vram = Vram(0x920000);
/// Object RAM: the sprite list (four words per sprite).
pub const OBJECTS: Vram = Vram(0x708000);

/// Color `color` of palette page `page` (word).
pub const fn palette(page: u32, color: u32) -> Vram {
    PALETTES.offset(0x400 * page + 2 * color)
}

/// A CPS-A, CPS-B or object output register (word).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Reg(pub u32);

impl Reg {
    pub fn set(self, b: &mut Cps2Bus, v: u16) {
        wr16(b, self.0, v)
    }
    pub fn get(self, b: &mut Cps2Bus) -> u16 {
        rd16(b, self.0)
    }
    /// Two consecutive registers written together (a scroll position x, y).
    pub fn set_long(self, b: &mut Cps2Bus, v: u32) {
        wr32(b, self.0, v)
    }
    /// The register `n` words further.
    pub const fn nth(self, n: u32) -> Reg {
        Reg(self.0 + 2 * n)
    }
}

/// The registers written by the game.
pub mod regs {
    use super::Reg;

    /// CPS-A: base of the object table (in units of 256 bytes: 0x7080 for 0x708000).
    pub const OBJECT_BASE: Reg = Reg(0x804100);
    /// CPS-A: bases of layers 1 to 3 (0x9100, 0x9140, 0x9180).
    pub const SCROLL_BASES: Reg = Reg(0x804102);
    /// CPS-A: base of the row scroll table (0x9200).
    pub const ROW_SCROLL_BASE: Reg = Reg(0x804108);
    /// CPS-A: scroll positions (x, y) of layers 1 to 3, six words.
    pub const SCROLLS: Reg = Reg(0x80410c);
    /// CPS-A: video control (row scroll, layer 1 page...).
    pub const VIDEO_CONTROL: Reg = Reg(0x804122);
    /// CPS-B: layer enable and priority (image in `A5+0x14`).
    pub const LAYER_CONTROL: Reg = Reg(0x804166);
    /// Object output control word (0x400004): 0x1355 when the screen is on.
    pub const OBJECT_OUTPUT: Reg = Reg(0x400004);
    /// Digital volume (read only: 0xe021, no additional RAM).
    pub const VOLUME: Reg = Reg(0x804030);
}
