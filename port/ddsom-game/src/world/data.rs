//! Game data: the typed tables of `ddsom-data` ([`data()`]), and the residue.
//!
//! The residue is the bytes of the program ROM that no typed table describes (an entry the extraction does not know,
//! an index past the end of a table on a path that no scenario plays). The game machine has no ROM: these bytes come
//! from `ddsom_data::residue` (the program of the ROMs, or `data/natives.json` and `data/reste.json` of the assets).
//! Each consultation is counted with what it was looking for ([`Missing`], [`residue`]); with `DDSOM_STRICT=1` it
//! stops the game, to find what a table still lacks (docs/fin.md, L14). `DDSOM_RESIDUE_BYTES=1` prints each byte read
//! once (`residue byte: table|key|space|address`), the input of `port/extract/overreads.txt`.
//!
//! The overreads of the original that the scenarios play (glitches: invalid frame pointers, incoherent animation
//! scripts...) are not residue: their bytes are a typed table (`data/overreads.json`, `GameData::overreads`), read
//! first.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, OnceLock};

use crate::world::World;

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

/// Consultations of the residue since the start of the process, per missing key.
pub fn residue() -> Vec<(Missing, u64)> {
    RESIDUE.lock().unwrap().iter().map(|(&m, &n)| (m, n)).collect()
}

/// The program ROM ends here: above are the RAM and the hardware, read on the bus.
const ROM_END: u32 = 0x40_0000;

/// Byte `a` (24 bits, below `ROM_END`) of the space `program` from the residue, for the key `m` (see the module).
fn consult(m: Missing, a: u32, program: bool) -> u8 {
    static STRICT: OnceLock<bool> = OnceLock::new();
    static BYTES: OnceLock<bool> = OnceLock::new();
    static SEEN: Mutex<BTreeSet<(Missing, bool, u32)>> = Mutex::new(BTreeSet::new());
    if *STRICT.get_or_init(|| std::env::var("DDSOM_STRICT").is_ok_and(|v| v == "1")) {
        panic!("DDSOM_STRICT: {} {:#x} unknown, read of the residue at {a:#x}", m.table, m.key);
    }
    if *BYTES.get_or_init(|| std::env::var("DDSOM_RESIDUE_BYTES").is_ok_and(|v| v == "1"))
        && SEEN.lock().unwrap().insert((m, program, a))
    {
        let space = if program { "program" } else { "data" };
        eprintln!("residue byte: {}|{:#x}|{space}|{a:#x}", m.table, m.key);
    }
    ddsom_data::residue::residue().byte(a, program).unwrap_or(0)
}

/// `n` bytes from `a` (big-endian) for the key `m`: the bus above the ROM (some readers follow a pointer that may
/// designate the RAM); below, an overread of the original named in the data (`GameData::overreads`), otherwise the
/// residue, counted once per read. A word or a long is read at an even address, as on the bus.
fn read(m: Missing, b: &mut World, a: u32, n: u32, program: bool) -> u32 {
    let a = a & 0xff_ffff;
    if a >= ROM_END {
        return match n {
            1 => b.read8(a) as u32,
            2 => b.read16(a) as u32,
            _ => (b.read16(a) as u32) << 16 | b.read16(a.wrapping_add(2) & 0xff_ffff) as u32,
        };
    }
    let start = if n == 1 { a } else { a & !1 };
    let overreads = &data().overreads;
    let mut residual = false;
    let v = (0..n).fold(0, |v, k| {
        let a = (start + k) & 0xff_ffff;
        let x = overreads.byte(m.table, m.key, a, program).unwrap_or_else(|| {
            residual = true;
            consult(m, a, program)
        });
        v << 8 | x as u32
    });
    if residual {
        *RESIDUE.lock().unwrap().entry(m).or_default() += 1;
    }
    v
}

impl Missing {
    /// Byte of the residue (data space).
    pub fn read8(self, b: &mut World, a: u32) -> u8 {
        read(self, b, a, 1, false) as u8
    }

    /// Word of the residue (data space).
    pub fn read16(self, b: &mut World, a: u32) -> u16 {
        read(self, b, a, 2, false) as u16
    }

    /// Long of the residue (data space).
    pub fn read32(self, b: &mut World, a: u32) -> u32 {
        read(self, b, a, 4, false)
    }

    /// Byte of the residue in the program space.
    pub fn pc_byte(self, b: &mut World, a: u32) -> u8 {
        read(self, b, a, 1, true) as u8
    }

    /// Word of the residue in the program space.
    pub fn pc_word(self, b: &mut World, a: u32) -> u16 {
        read(self, b, a, 2, true) as u16
    }

    /// Long of the residue in the program space (tables in the code, read PC-relative).
    pub fn pc_long(self, b: &mut World, a: u32) -> u32 {
        read(self, b, a, 4, true)
    }

    /// Byte of the residue, in program space if `program` (tables read PC-relative), otherwise in data space.
    pub fn byte_in(self, b: &mut World, a: u32, program: bool) -> u8 {
        read(self, b, a, 1, program) as u8
    }

    /// Word of the residue in the space chosen by `program`.
    pub fn word_in(self, b: &mut World, a: u32, program: bool) -> u16 {
        read(self, b, a, 2, program) as u16
    }

    /// Long of the residue in the space chosen by `program`.
    pub fn long_in(self, b: &mut World, a: u32, program: bool) -> u32 {
        read(self, b, a, 4, program)
    }
}
