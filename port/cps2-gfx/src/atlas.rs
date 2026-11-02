//! The graphics as the renderer reads them: 16x16 cells of 8-bit pens, decoded once.
//!
//! The hardware stores a cell in 128 bytes, 4 bit planes per group of 8 pixels (`rom::load_gfx`). The atlas
//! stores the same cells with one byte per pixel (pen 0..15, 15 = transparent), in 256 bytes: row `y` of cell
//! `c` is the 16 bytes at `c * 256 + y * 16`. A group of 4 plane bytes at hardware offset `o` thus becomes the
//! 8 pens at atlas offset `2 * o`, which is how the renderer maps the hardware tile addressing onto the atlas.

use std::sync::Arc;

/// Pens of all the graphic cells, shared (cloning an atlas does not copy the pixels).
#[derive(Clone)]
pub struct TileAtlas {
    pens: Arc<[u8]>,
}

impl TileAtlas {
    /// Side of a cell, in pixels.
    pub const CELL: usize = 16;
    /// Bytes of a cell in the atlas.
    pub const CELL_PENS: usize = Self::CELL * Self::CELL;
    /// Bytes of a cell in the hardware format.
    pub const CELL_PLANAR: usize = 128;

    /// Atlas from the pens of the cells, cell after cell, row after row (as `ddsom_assets::Cells` loads them
    /// from `gfx/cells_NN.png`).
    pub fn from_pens(pens: Vec<u8>) -> Self {
        assert!(pens.len().is_multiple_of(Self::CELL_PENS), "atlas: {} pens is not a whole number of cells", pens.len());
        TileAtlas { pens: pens.into() }
    }

    /// Atlas decoded from the graphics region in the hardware format (`rom::load_gfx`).
    pub fn from_planar(region: &[u8]) -> Self {
        let mut pens = vec![0u8; region.len() * 2];
        for (group, out) in region.as_chunks::<4>().0.iter().zip(pens.as_chunks_mut::<8>().0) {
            *out = decode_group(group);
        }
        Self::from_pens(pens)
    }

    /// Number of cells.
    pub fn cells(&self) -> usize {
        self.pens.len() / Self::CELL_PENS
    }

    /// All the pens.
    pub fn pens(&self) -> &[u8] {
        &self.pens
    }

    /// Pen of pixel (x, y) of cell `c`.
    pub fn pen(&self, c: usize, x: usize, y: usize) -> u8 {
        self.pens[c * Self::CELL_PENS + y * Self::CELL + x]
    }
}

/// `BURST[b]`: the 8 bits of `b` laid out one per byte, the most significant bit in byte 0 (pixel 0).
const BURST: [u64; 256] = {
    let mut t = [0u64; 256];
    let mut b = 0;
    while b < 256 {
        let mut k = 0;
        while k < 8 {
            t[b] |= (((b >> (7 - k)) & 1) as u64) << (8 * k);
            k += 1;
        }
        b += 1;
    }
    t
};

/// The 8 pens of a group of 4 plane bytes (plane 0 first, most significant bit = leftmost pixel).
fn decode_group(planes: &[u8; 4]) -> [u8; 8] {
    let [p0, p1, p2, p3] = planes.map(|p| BURST[p as usize]);
    (p0 | p1 << 1 | p2 << 2 | p3 << 3).to_le_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planar_decoding() {
        // pixel 0: pen 0b1010 (planes 1 and 3), pixel 7: pen 0b0101 (planes 0 and 2)
        let atlas = TileAtlas::from_planar(&[[0x01, 0x80, 0x01, 0x80]; 32].concat());
        assert_eq!(atlas.cells(), 1);
        assert_eq!(&atlas.pens()[..8], &[10, 0, 0, 0, 0, 0, 0, 5]);
    }
}
