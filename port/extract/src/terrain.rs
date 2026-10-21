//! Terrain : cartes fixes des scènes 2 et 0x14 et tables des types de terrain (`ddsom-game/src/video.rs`,
//! `terrain.rs`).

use std::io;
use std::path::Path;

use ddsom_assets::json;
use ddsom_assets::terrain::{FixedMap, Kind, KindTable, Terrain};

use crate::rom::Program;

/// (adresse, scène, correction de y, colonnes) : la première carte va jusqu'à la seconde (ses 16 dernières
/// colonnes sont nulles), la seconde jusqu'au remplissage 0xffff de la fin de la ROM.
const MAPS: [(u32, u8, i16, u32); 2] = [(0x3f8e9c, 2, -0x10, 112), (0x3f9c9c, 0x14, 0x10, 96)];
/// (table, routine) : 128 types chacune, la seconde suit la première, du code suit la seconde.
const KINDS: [(u32, u32); 2] = [(0x3425c, 0x33fe4), (0x34a5c, 0x3403c)];
const KIND_COUNT: u32 = 128;

pub fn extract(roms: &Path, path: &Path) -> io::Result<()> {
    let rom = Program::load(roms)?;
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
    json::save(path, &Terrain { maps, kinds }, 4)
}
