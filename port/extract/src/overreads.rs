//! Overreads of the original: `data/overreads.json` (`ddsom_data::Overreads`, read by `ddsom-game/src/data.rs` before
//! the residue). The list is `port/extract/overreads.txt` (addresses, no ROM byte): one line per read range,
//! `<space> <start>-<end> <key> <table> # <reason>`, with `space` `d` (data) or `p` (decrypted program), the end
//! exclusive, `key` and `table` those of the reader (`Missing`). It comes from `DDSOM_RESIDUE_BYTES=1` over the
//! scenarios (`tools/overreads.py`, docs/sans-rom.md "Overreads").

use std::io;
use std::path::Path;

use ddsom_data::overreads::{Overread, Overreads};

use crate::rom::Program;

const LIST: &str = include_str!("../overreads.txt");

/// The overreads, with their bytes taken from `rom`.
pub fn overreads(rom: &Program) -> Overreads {
    let mut reads = Vec::new();
    for l in LIST.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
        let (fields, reason) = l.split_once('#').map_or((l, ""), |(f, r)| (f, r.trim()));
        let mut t = fields.split_whitespace();
        let (Some(space), Some(range), Some(key)) = (t.next(), t.next(), t.next()) else {
            panic!("overreads.txt: unreadable line '{l}'")
        };
        let table = t.collect::<Vec<_>>().join(" ");
        let (start, end) = range.split_once('-').unwrap_or_else(|| panic!("overreads.txt: range '{range}'"));
        let hex = |s: &str| u32::from_str_radix(s.trim_start_matches("0x"), 16).unwrap_or_else(|_| panic!("overreads.txt: '{s}'"));
        let (start, end, key) = (hex(start), hex(end), hex(key));
        let program = match space {
            "d" => false,
            "p" => true,
            _ => panic!("overreads.txt: space '{space}'"),
        };
        let bytes = (start..end).map(|a| format!("{:02x}", if program { rom.prog8(a) } else { rom.rd8(a) })).collect();
        reads.push(Overread { table, key, reason: reason.to_string(), program, start, bytes });
    }
    Overreads {
        description: "Bytes that the original reads outside every table on the paths the scenarios play (glitches), by \
                      reader: table and key of the read (docs/sans-rom.md)"
            .into(),
        reads,
    }
}

pub fn extract(roms: &Path, path: &Path) -> io::Result<()> {
    let rom = Program::load(roms)?;
    ddsom_data::json::save(path, &overreads(&rom), 2)
}
