//! Speed tables of the objects: `data/speeds.json` (`ddsom_data::speeds`, docs/assets.md).
//!
//! The tables follow one another from `REGION.start`; each one starts at an address that an array of the code or the
//! code itself designates, and runs up to the next one.

use std::io;
use std::ops::Range;
use std::path::Path;

use ddsom_data::json;
use ddsom_data::pointers::Pointers;
use ddsom_data::speeds::{SpeedTable, Speeds};

use crate::rom::Program;

/// Arrays of table addresses, in the code (program space): start and number of tables. Read by the routines 0x1a94
/// (first array) and 0x1b64 (second); each array ends where the code of the next routine starts.
const ARRAYS: [(u32, u32); 2] = [(0x1ab4, 44), (0x1b84, 76)];
/// The speed tables, one after the other (the visibility boxes follow, 0x1ca538).
const REGION: Range<u32> = 0x1c4836..0x1ca2b6;
/// Tables that the code designates directly: SET items (0x1c65b6), walking and running of the walkers (0x1c75b6,
/// 0x1c7eb6).
const DIRECT: [u32; 3] = [0x1c65b6, 0x1c75b6, 0x1c7eb6];

pub fn speeds(rom: &Program) -> Speeds {
    let arrays: Vec<Pointers> = ARRAYS
        .iter()
        .map(|&(at, n)| Pointers { rom: at, targets: (0..n).map(|i| rom.prog32(at + 4 * i)).collect() })
        .collect();
    let mut starts: Vec<u32> = arrays.iter().flat_map(|a| a.targets.iter().copied()).chain(DIRECT).collect();
    starts.push(REGION.start);
    starts.sort();
    starts.dedup();
    assert!(starts.iter().all(|a| REGION.contains(a) && (a - REGION.start).is_multiple_of(4)), "speed table outside the region");
    let tables = starts
        .iter()
        .enumerate()
        .map(|(i, &at)| {
            let end = starts.get(i + 1).copied().unwrap_or(REGION.end);
            let speeds = (at..end).step_by(4).map(|a| (rom.rd16(a) as i16, rom.rd16(a + 2) as i16)).collect();
            SpeedTable { rom: at, speeds }
        })
        .collect();
    Speeds { arrays, tables }
}

pub fn extract(roms: &Path, path: &Path) -> io::Result<()> {
    let rom = Program::load(roms)?;
    std::fs::create_dir_all(path.parent().unwrap())?;
    json::save(path, &speeds(&rom), 2)
}
