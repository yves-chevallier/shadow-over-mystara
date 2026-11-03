//! Overreads of the original (`data/overreads.json`): the bytes that a reader takes outside every table, on a path
//! that the scenarios play (original glitches: an invalid frame pointer, an animation script that runs into what is
//! not a script, a speed table 0...). Each one is named by the read that makes it (the table and the key of the
//! reader, `ddsom-game/src/data.rs`, `Missing`) and holds the bytes it takes: the game reads them instead of the
//! residue. The list (`port/extract/overreads.txt`) comes from a measurement (`DDSOM_RESIDUE_BYTES=1`), with the
//! reason of each case (docs/sans-rom.md, "Overreads").

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::json::hex;

#[derive(Serialize, Deserialize, Default, Debug, PartialEq, Eq)]
pub struct Overreads {
    pub description: String,
    pub reads: Vec<Overread>,
}

/// The bytes that one read takes outside the tables.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Overread {
    /// Table and key of the reader (`"frame"` and the frame address...).
    pub table: String,
    #[serde(with = "hex")]
    pub key: u32,
    /// Why the original reads there.
    pub reason: String,
    /// Program space (read PC-relative), otherwise data space.
    pub program: bool,
    /// Address of the first byte.
    #[serde(with = "hex")]
    pub start: u32,
    /// The bytes, in hexadecimal.
    pub bytes: String,
}

impl Overread {
    /// Byte at address `a`, if this overread holds it.
    fn byte(&self, a: u32) -> Option<u8> {
        let i = 2 * a.checked_sub(self.start)? as usize;
        u8::from_str_radix(self.bytes.get(i..i + 2)?, 16).ok()
    }
}

impl Overreads {
    pub fn load(path: &Path) -> io::Result<Self> {
        crate::json::load(path)
    }

    /// Byte at `a` of the space `program` that the read `(table, key)` takes, if it is a named overread.
    pub fn byte(&self, table: &str, key: u32, a: u32, program: bool) -> Option<u8> {
        self.reads
            .iter()
            .filter(|r| r.key == key && r.program == program && r.table == table)
            .find_map(|r| r.byte(a))
    }
}
