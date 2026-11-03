//! Terrain: fixed maps of scenes 2 and 0x14 and terrain type tables (`ddsom-game/src/video.rs`,
//! `terrain.rs`).

use std::io;
use std::path::Path;

use ddsom_assets::json;
use ddsom_assets::terrain::{FixedMap, Kind, KindTable, Terrain};

use crate::rom::Program;

/// (address, scene, y correction, columns): the first map goes up to the second (its last 16
/// columns are null), the second up to the 0xffff fill at the end of the ROM.
const MAPS: [(u32, u8, i16, u32); 2] = [(0x3f8e9c, 2, -0x10, 112), (0x3f9c9c, 0x14, 0x10, 96)];
/// (table, routine): 128 types each, the second follows the first, code follows the second.
const KINDS: [(u32, u32); 2] = [(0x3425c, 0x33fe4), (0x34a5c, 0x3403c)];
const KIND_COUNT: u32 = 128;

pub fn extract(roms: &Path, path: &Path) -> io::Result<()> {
    json::save(path, &terrain(&Program::load(roms)?), 4)
}

/// The terrain tables of the program `rom` (`terrain.json`).
pub fn terrain(rom: &Program) -> Terrain {
    let maps = MAPS
        .iter()
        .map(|&(at, stage, dy, n)| FixedMap {
            rom: at,
            stage,
            dy,
            columns: (0..n).map(|c| std::array::from_fn(|k| rom.rd16(at + 32 * c + 2 * k as u32))).collect(),
        })
        .collect();
    let kinds = KINDS
        .iter()
        .map(|&(at, routine)| KindTable {
            rom: at,
            routine,
            kinds: (0..KIND_COUNT).map(|i| Kind::from_words(std::array::from_fn(|k| rom.rd16(at + 16 * i + 2 * k as u32)))).collect(),
        })
        .collect();
    Terrain { maps, kinds }
}
