//! Palettes des scènes : tables qui désignent, par scène, les palettes de la bibliothèque à charger
//! (`ddsom-game/src/palette.rs`, docs/video.md V3).

use std::io;
use std::path::Path;

use ddsom_assets::json;
use ddsom_assets::scene_palettes::{AddressTable, ScenePalettes};

use crate::rom::Program;

const LIBRARY: u32 = 0x253000;
const STAGES: u32 = 0x2ae81e;
/// Scènes 0 à 114 (les tables par scène s'arrêtent là : la suite est du code ou d'autres données).
const SCENES: u32 = 115;

fn table(rom: &Program, at: u32, program: bool, count: u32, n: u32) -> AddressTable {
    AddressTable { rom: at, program, count, entries: (0..n).map(|i| rom.long(at + 4 * i, program)).collect() }
}

pub fn extract(roms: &Path, path: &Path) -> io::Result<()> {
    let rom = Program::load(roms)?;
    let mut stages = Vec::new();
    for s in 0..SCENES {
        let mut list = [0i16; 32];
        for (i, p) in list.iter_mut().enumerate() {
            let w = rom.rd16(STAGES + 0x40 * s + 2 * i as u32);
            *p = match w {
                0xffff => -1,
                w if w < 0x8000 && w % 32 == 0 => (w / 32) as i16,
                _ => return Err(io::Error::new(io::ErrorKind::InvalidData, format!("scène {s:#x} : déplacement {w:#x}"))),
            };
        }
        stages.push(list);
    }
    let palettes = ScenePalettes {
        library: LIBRARY,
        stages_rom: STAGES,
        stages,
        extra: table(&rom, 0x1a41c, false, 3, SCENES),
        sets: table(&rom, 0x1a3fc, false, 32, 8),
        pages: vec![
            table(&rom, 0x1a62c, true, 32, 3),
            table(&rom, 0x1a6a0, true, 32, SCENES),
            table(&rom, 0x1a8d4, true, 32, SCENES),
        ],
    };
    json::save(path, &palettes, 3)
}
