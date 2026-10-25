//! Scripts d'apparition des scènes (0xf2e5c, `ddsom-game/src/scene/script.rs`).

use std::io;
use std::path::Path;

use ddsom_assets::json;
use ddsom_assets::spawns::{Entry, Script, Spawns};

use crate::rom::Program;

const TABLE: u32 = 0xf2f62;
/// Scènes 0 à 114.
const SCENES: u32 = 115;
const END: u16 = 0xffff;

fn script(rom: &Program, at: u32) -> Script {
    let mut entries = Vec::new();
    let mut p = at;
    while rom.rd16(p) != END {
        let x = rom.rd16(p) as i16;
        if rom.rd16(p + 2) == 0xffff {
            entries.push(Entry::Call { x, call: rom.rd32(p + 4), arg: rom.rd16(p + 8) });
            p += 10;
        } else {
            let b = |k: u32| rom.rd8(p + k);
            entries.push(Entry::Spawn {
                x,
                dx: rom.rd16(p + 2) as i16,
                z: rom.rd16(p + 4) as i16,
                y: rom.rd16(p + 6) as i16,
                rank: b(8),
                players: b(9),
                unused: b(10),
                class: b(11),
                f2c: b(12),
                kind: b(13),
                variant: b(14),
                f2f: b(15),
                f42: rom.rd16(p + 16),
            });
            p += 0x12;
        }
    }
    Script { rom: at, entries, pad: Vec::new() }
}

pub fn extract(roms: &Path, path: &Path) -> io::Result<()> {
    let rom = Program::load(roms)?;
    let scenes: Vec<u32> = (0..SCENES).map(|s| rom.rd32(TABLE + 4 * s)).collect();
    let mut starts = scenes.clone();
    starts.sort();
    starts.dedup();
    let mut scripts: Vec<Script> = starts.iter().map(|&a| script(&rom, a)).collect();
    // bourrage entre un script et le suivant (au plus 2 octets, 0x4e71)
    for i in 0..scripts.len().saturating_sub(1) {
        let end = scripts[i].rom + scripts[i].bytes().len() as u32;
        let next = scripts[i + 1].rom;
        if next > end && next - end <= 2 {
            scripts[i].pad = (end..next).map(|a| rom.rd8(a)).collect();
        }
    }
    eprintln!(
        "apparitions : {} scènes, {} scripts, {} entrées",
        scenes.len(),
        scripts.len(),
        scripts.iter().map(|s| s.entries.len()).sum::<usize>()
    );
    std::fs::create_dir_all(path.parent().unwrap())?;
    json::save(path, &Spawns { table: TABLE, scenes, scripts }, 4)
}
