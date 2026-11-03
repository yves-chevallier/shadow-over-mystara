//! Scene palettes: which palettes of the library (`palettes.png`) each scene loads,
//! `assets/scene_palettes.json`, see docs/assets.md and docs/video.md (V3).
//!
//! All the designated palettes are in the library (ROM 0x253000): an address `a` is palette
//! `(a - 0x253000) / 32` of `palettes.png`; a whole page is 32 consecutive palettes.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::json::{hex, hex_vec};

/// Palette address table, one per entry (scene, set...).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AddressTable {
    #[serde(with = "hex")]
    pub rom: u32,
    /// Read in program space (PC-relative access) rather than in data space.
    pub program: bool,
    /// Palettes copied from each address.
    pub count: u32,
    /// Address of the first palette (0: none, or ROM address 0 for page 3).
    #[serde(with = "hex_vec")]
    pub entries: Vec<u32>,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct ScenePalettes {
    /// Palette library (32 bytes each).
    #[serde(with = "hex")]
    pub library: u32,
    /// Page 0, per scene: 32 palette numbers of the library (-1: palette left in place).
    /// The game loads only 29 of them, except for scenes 0x45 to 0x59.
    #[serde(with = "hex")]
    pub stages_rom: u32,
    pub stages: Vec<[i16; 32]>,
    /// Three palettes added after those of page 0, per scene (0x1a41c).
    pub extra: AddressTable,
    /// Sets of 32 palettes loaded into page 0 by 0x1a3d4.
    pub sets: AddressTable,
    /// Pages 1 to 3 (32 palettes each), per entry (page 1: 0 to 2; pages 2 and 3: scene).
    pub pages: Vec<AddressTable>,
}

impl ScenePalettes {
    pub fn load(path: &Path) -> io::Result<Self> {
        crate::json::load(path)
    }

    /// Library number of the palette at address `rom`.
    pub fn library_index(&self, rom: u32) -> Option<usize> {
        let d = rom.checked_sub(self.library)?;
        (d % 32 == 0).then_some(d as usize / 32)
    }

    /// Offset word as the ROM stores it for a palette number of `stages`.
    pub fn stage_word(palette: i16) -> u16 {
        if palette < 0 {
            0xffff
        } else {
            palette as u16 * 32
        }
    }
}
