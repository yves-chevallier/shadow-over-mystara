//! Terrain and collision with the scenery: `assets/terrain.json`, see docs/assets.md.
//!
//! The terrain number of a point is the `attributes >> 7` field of the scroll 2 cell under it
//! (already in `levels.json`), except in two scenes that read a fixed map (`FixedMap`). This number
//! indexes a table of kinds (`KindTable`, two copies depending on the collision routine).

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::json::hex;

/// Fixed terrain map: columns 16 pixels wide, 16 cells of 16 pixels high each.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct FixedMap {
    #[serde(with = "hex")]
    pub rom: u32,
    /// Scene (`A5+0x34`) that reads this map.
    pub stage: u8,
    /// Added to y before the read (word).
    pub dy: i16,
    /// Attribute words, column by column (increasing x); within a column, word `k` covers
    /// `(y + dy) / 16 % 16 = 15 - k` (top of the screen first).
    pub columns: Vec<[u16; 16]>,
}

impl FixedMap {
    /// Attribute word at world point (x, y), with the arithmetic of 0x24b0 (`None` outside the map).
    pub fn word(&self, x: u16, y: u16) -> Option<u16> {
        let y = y.wrapping_add(self.dy as u16);
        let k = 15 - ((y >> 4) & 15) as usize;
        let column = (x.wrapping_add(x) & 0xffe0) as i16 as isize / 32;
        let c = self.columns.get(usize::try_from(column).ok()?)?;
        Some(c[k])
    }

    /// Terrain number at point (x, y).
    pub fn terrain(&self, x: u16, y: u16) -> Option<u16> {
        self.word(x, y).map(|w| (w & 0xff80) >> 7)
    }
}

/// Terrain kind (16-byte entry, words), see `ddsom-game/src/terrain.rs`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kind {
    /// 0 passable, 1 solid obstacle, 2 and above slope.
    pub category: u16,
    /// Subroutine (0, 4, 8, 12): edge below, above, right, left.
    pub routine: u16,
    /// Slope: direction (0 or 1).
    pub flip: u16,
    /// Slope: shift (scale) of the height.
    pub shift: u16,
    /// Depth of the edge (slope: base height).
    pub edge: u16,
    /// Solid obstacle: non-zero, a single crossing.
    pub stop: u16,
    /// Words 6 and 7, unused (zero).
    pub unused: [u16; 2],
}

impl Kind {
    pub fn words(&self) -> [u16; 8] {
        [self.category, self.routine, self.flip, self.shift, self.edge, self.stop, self.unused[0], self.unused[1]]
    }

    pub fn from_words(w: [u16; 8]) -> Self {
        Kind { category: w[0], routine: w[1], flip: w[2], shift: w[3], edge: w[4], stop: w[5], unused: [w[6], w[7]] }
    }
}

/// Kind table of a collision routine (0x33fe4: `0x3425c`; 0x3403c: `0x34a5c`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct KindTable {
    #[serde(with = "hex")]
    pub rom: u32,
    /// Address of the routine that reads it.
    #[serde(with = "hex")]
    pub routine: u32,
    pub kinds: Vec<Kind>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Terrain {
    pub maps: Vec<FixedMap>,
    pub kinds: Vec<KindTable>,
}

impl Terrain {
    pub fn load(path: &Path) -> io::Result<Self> {
        crate::json::load(path)
    }
}
