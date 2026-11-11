//! Tables of the SET objects: `data/set.json` (`data().set`, extracted by `extract/src/set.rs`; docs/fin.md, L6), and
//! the dialog windows and texts of `data/dialogs.json` (`data().dialogs`).
//!
//! Every ROM table that the SET types read is a named table of that file, with its address, its format and, when
//! known, the names of its columns. The code reads them by address inside the tables ([`byte`], [`word`], [`long`]
//! and their program-space twins [`pc_byte`], [`pc_word`], [`pc_long`]): the original reaches most of them through
//! a pointer that it keeps in its state or in another table (scripts, texts, tile strings), or indexes them in bytes,
//! sometimes past their shape.
//!
//! A read outside every table is a read of the residue (`Missing`, table `"set"`): it is counted, and fatal with
//! `DDSOM_STRICT=1`. The original reads past some tables (an index out of range...): the bytes read that way are
//! extracted as part of the table when the reach is known.

use std::sync::OnceLock;

use crate::world::World;

use crate::data::{data, Missing};

/// Bytes of the tables, by space (data, program): runs of contiguous bytes sorted by address (tables that touch or
/// overlap, as a table of `set.json` and a table of `dialogs.json` can, make one run).
struct Index {
    zones: [Vec<(u32, Vec<u8>)>; 2],
}

fn index() -> &'static Index {
    static INDEX: OnceLock<Index> = OnceLock::new();
    INDEX.get_or_init(|| {
        let files = [&data().set, &data().dialogs];
        let mut zones: [Vec<(u32, Vec<u8>)>; 2] = [Vec::new(), Vec::new()];
        for t in files.iter().flat_map(|f| &f.tables) {
            let bytes = t.bytes().unwrap_or_else(|e| panic!("SET tables: {e}"));
            zones[t.program as usize].push((t.rom, bytes));
        }
        for t in files.iter().flat_map(|f| &f.texts) {
            zones[0].push((t.rom, t.bytes()));
        }
        Index { zones: zones.map(runs) }
    })
}

/// Merges the tables that touch or overlap into runs (their common bytes are the same ROM bytes).
fn runs(mut zones: Vec<(u32, Vec<u8>)>) -> Vec<(u32, Vec<u8>)> {
    zones.sort_by_key(|&(rom, _)| rom);
    let mut out: Vec<(u32, Vec<u8>)> = Vec::new();
    for (rom, bytes) in zones {
        match out.last_mut() {
            Some((start, run)) if rom <= *start + run.len() as u32 => {
                let skip = (*start + run.len() as u32 - rom) as usize;
                run.extend(bytes.iter().skip(skip));
            }
            _ => out.push((rom, bytes)),
        }
    }
    out
}

/// Byte at address `a` of the tables of one space, if a table holds it.
fn find(program: bool, a: u32) -> Option<u8> {
    let zones = &index().zones[program as usize];
    let i = zones.partition_point(|&(rom, _)| rom <= a).checked_sub(1)?;
    let (rom, bytes) = &zones[i];
    bytes.get((a - rom) as usize).copied()
}

/// `n` bytes from `a` (big-endian), if the tables hold them all.
fn bytes(program: bool, a: u32, n: u32) -> Option<u32> {
    (0..n).try_fold(0u32, |v, i| Some(v << 8 | find(program, a.wrapping_add(i))? as u32))
}

const MISSING: &str = "set";

/// The program ROM ends here: above are the RAM and the hardware, which some callers also read through these
/// functions (an address taken from the state can designate either).
const ROM_END: u32 = 0x40_0000;

/// What a read outside the tables was looking for.
fn missing(a: u32) -> Missing {
    Missing { table: MISSING, key: a }
}

/// Byte of the data space.
pub fn byte(b: &mut World, a: u32) -> u8 {
    if a >= ROM_END {
        return crate::mem::rd8(b, a);
    }
    match bytes(false, a, 1) {
        Some(v) => v as u8,
        None => missing(a).byte_in(b, a, false),
    }
}

/// Word of the data space.
pub fn word(b: &mut World, a: u32) -> u16 {
    if a >= ROM_END {
        return crate::mem::rd16(b, a);
    }
    bytes(false, a, 2).map_or_else(|| missing(a).word_in(b, a, false), |v| v as u16)
}

/// Long of the data space.
pub fn long(b: &mut World, a: u32) -> u32 {
    if a >= ROM_END {
        return crate::mem::rd32(b, a);
    }
    bytes(false, a, 4).unwrap_or_else(|| missing(a).long_in(b, a, false))
}

/// Byte of the program space (table read PC-relative).
pub fn pc_byte(b: &mut World, a: u32) -> u8 {
    match bytes(true, a, 1) {
        Some(v) => v as u8,
        None => missing(a).byte_in(b, a, true),
    }
}

/// Word of the program space.
pub fn pc_word(b: &mut World, a: u32) -> u16 {
    bytes(true, a, 2).map_or_else(|| missing(a).word_in(b, a, true), |v| v as u16)
}

/// Long of the program space.
pub fn pc_long(b: &mut World, a: u32) -> u32 {
    bytes(true, a, 4).unwrap_or_else(|| missing(a).long_in(b, a, true))
}
