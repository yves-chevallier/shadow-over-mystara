//! Speed tables of the objects (motion by table, `objects/motion.rs` of the game): `data/speeds.json`.
//!
//! A table gives, per index, the speed of an object in X and in Y (or Z), in 8.8 pixels per frame. The object keeps
//! the address of its table (`+0x4c`) and its index (`+0x4e`). The original reads `table + 4 × index`: an index past
//! the end of a table reads the next table, which follows it in ROM (`Speeds::at` reads by address across tables).

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::json::hex;
use crate::pointers::Pointers;
use crate::RomAddr;

/// One speed table: `(vx, vy)` per index.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SpeedTable {
    #[serde(with = "hex")]
    pub rom: u32,
    pub speeds: Vec<(i16, i16)>,
}

impl SpeedTable {
    pub fn end(&self) -> u32 {
        self.rom + 4 * self.speeds.len() as u32
    }
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Speeds {
    /// Arrays of tables selected by number (routines 0x1a94 and 0x1b64; program space).
    pub arrays: Vec<Pointers>,
    /// The tables, by increasing address, one after the other.
    pub tables: Vec<SpeedTable>,
}

impl Speeds {
    pub fn load(path: &Path) -> io::Result<Self> {
        crate::json::load(path)
    }

    /// Array of tables stored at `rom`.
    pub fn array(&self, rom: RomAddr) -> Option<&Pointers> {
        self.arrays.iter().find(|a| a.rom == rom.0)
    }

    /// Speed word at address `a` (`vx` at `table + 4 i`, `vy` at `table + 4 i + 2`), in whichever table holds it.
    pub fn at(&self, a: RomAddr) -> Option<u16> {
        let i = self.tables.partition_point(|t| t.rom <= a.0).checked_sub(1)?;
        let t = &self.tables[i];
        let off = a.0 - t.rom;
        let &(vx, vy) = t.speeds.get(off as usize / 4).filter(|_| off.is_multiple_of(2))?;
        Some(if off.is_multiple_of(4) { vx } else { vy } as u16)
    }
}
