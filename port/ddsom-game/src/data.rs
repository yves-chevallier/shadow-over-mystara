//! Game data: the typed tables of `ddsom-data` ([`data()`]), and the residue.
//!
//! The residue is the bytes of the program ROM that no typed table describes yet (an entry the extraction does not
//! know, an index past the end of a table...): they are still read by address on the bus. Each such read is counted
//! with what it was looking for ([`Missing`], [`residue`]); with `DDSOM_STRICT=1` it stops the game, to find what a
//! table still lacks (docs/fin.md).

use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

use cps2::Cps2Bus;

pub use ddsom_data::{data, GameData, RomAddr};

/// What a read of the residue was looking for: the table (`"anim entry"`, `"anim set"`...) and the key it was
/// looked up with (entry address, set address...).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Missing {
    pub table: &'static str,
    pub key: u32,
}

/// Reads of the residue per missing key, since the start of the process.
static RESIDUE: Mutex<BTreeMap<Missing, u64>> = Mutex::new(BTreeMap::new());

/// Reads of the residue since the start of the process, per missing key.
pub fn residue() -> Vec<(Missing, u64)> {
    RESIDUE.lock().unwrap().iter().map(|(&m, &n)| (m, n)).collect()
}

fn note(m: Missing, a: u32) {
    static STRICT: OnceLock<bool> = OnceLock::new();
    *RESIDUE.lock().unwrap().entry(m).or_default() += 1;
    if *STRICT.get_or_init(|| std::env::var("DDSOM_STRICT").is_ok_and(|v| v == "1")) {
        panic!("DDSOM_STRICT: {} {:#x} unknown, read of the program ROM at {a:#x}", m.table, m.key);
    }
}

impl Missing {
    /// Byte of the residue (data space).
    pub fn read8(self, b: &mut Cps2Bus, a: u32) -> u8 {
        note(self, a);
        crate::mem::rd8(b, a)
    }

    /// Word of the residue (data space).
    pub fn read16(self, b: &mut Cps2Bus, a: u32) -> u16 {
        note(self, a);
        crate::mem::rd16(b, a)
    }

    /// Long of the residue (data space).
    pub fn read32(self, b: &mut Cps2Bus, a: u32) -> u32 {
        note(self, a);
        crate::mem::rd32(b, a)
    }

    /// Byte of the residue in the program space.
    pub fn pc_byte(self, b: &mut Cps2Bus, a: u32) -> u8 {
        note(self, a);
        crate::mem::prog8(b, a)
    }

    /// Word of the residue in the program space.
    pub fn pc_word(self, b: &mut Cps2Bus, a: u32) -> u16 {
        note(self, a);
        crate::mem::prog16(b, a)
    }

    /// Long of the residue in the program space (tables in the code, read PC-relative).
    pub fn pc_long(self, b: &mut Cps2Bus, a: u32) -> u32 {
        note(self, a);
        crate::mem::prog32(b, a)
    }

    /// Byte of the residue, in program space if `program` (tables read PC-relative), otherwise in data space.
    pub fn byte_in(self, b: &mut Cps2Bus, a: u32, program: bool) -> u8 {
        if program { self.pc_byte(b, a) } else { self.read8(b, a) }
    }

    /// Word of the residue in the space chosen by `program`.
    pub fn word_in(self, b: &mut Cps2Bus, a: u32, program: bool) -> u16 {
        if program { self.pc_word(b, a) } else { self.read16(b, a) }
    }

    /// Long of the residue in the space chosen by `program`.
    pub fn long_in(self, b: &mut Cps2Bus, a: u32, program: bool) -> u32 {
        if program { self.pc_long(b, a) } else { self.read32(b, a) }
    }
}
