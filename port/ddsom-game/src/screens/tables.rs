//! ROM data of the screens: the tables of `data/screens.json`, `data/scenes.json` and `data/misc.json` and the texts
//! of `texts.json` (`data().screens`, `.scenes`, `.misc`, `.texts`; docs/fin.md, L11).
//!
//! The screens reach most of their data through an address: a constant of the original code (a table, a text
//! descriptor), a pointer read from another table (banners, letter tables, portraits) or kept in the state (the
//! script of a demonstration, a text being chained). So the data are read by address inside the tables ([`byte`],
//! [`word`], [`long`], and their program-space twins [`pc_byte`], [`pc_word`], [`pc_long`] for the tables the original
//! reads PC-relative). An address of the RAM goes to the bus (some routines read either, as the original does).
//!
//! A read outside every table is a read of the residue (`Missing`, table `"screens"`): counted, fatal with
//! `DDSOM_STRICT=1`.

use std::sync::OnceLock;

use cps2::Cps2Bus;

use crate::data::{data, Missing, RomAddr};

/// Runs of contiguous bytes (start address, bytes) sorted by address.
type Runs = Vec<(u32, Vec<u8>)>;

/// Bytes of the tables by space (data, program).
fn index() -> &'static [Runs; 2] {
    static INDEX: OnceLock<[Runs; 2]> = OnceLock::new();
    INDEX.get_or_init(|| {
        let d = data();
        let mut zones: [Runs; 2] = [Vec::new(), Vec::new()];
        for f in [&d.screens, &d.scenes, &d.misc] {
            for t in &f.tables {
                let bytes = t.bytes().unwrap_or_else(|e| panic!("screen tables: {e}"));
                zones[t.program as usize].push((t.rom, bytes));
            }
            for t in &f.texts {
                zones[0].push((t.rom, t.bytes()));
            }
        }
        for (rom, program, bytes) in d.texts.regions() {
            zones[program as usize].push((rom, bytes));
        }
        zones.map(runs)
    })
}

/// Merges the tables that touch or overlap into runs (their common bytes are the same ROM bytes).
fn runs(mut zones: Runs) -> Runs {
    zones.sort_by_key(|&(rom, _)| rom);
    let mut out: Runs = Vec::new();
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

/// `n` bytes from `a` (big-endian) in the tables of one space, if they hold them all.
fn bytes(program: bool, a: u32, n: u32) -> Option<u32> {
    let zones = &index()[program as usize];
    let i = zones.partition_point(|&(rom, _)| rom <= a).checked_sub(1)?;
    let (rom, run) = &zones[i];
    let off = (a - rom) as usize;
    let s = run.get(off..off + n as usize)?;
    Some(s.iter().fold(0u32, |v, &x| v << 8 | x as u32))
}

/// The program ROM ends here: above are the RAM and the hardware.
const ROM_END: u32 = 0x40_0000;

fn missing(a: u32) -> Missing {
    Missing { table: "screens", key: a }
}

/// Byte of the data space at `a` (ROM table, or RAM).
pub fn byte(b: &mut Cps2Bus, a: u32) -> u8 {
    if a & 0xff_ffff >= ROM_END {
        return crate::mem::rd8(b, a);
    }
    bytes(false, a, 1).map_or_else(|| missing(a).read8(b, a), |v| v as u8)
}

/// Word of the data space at `a`.
pub fn word(b: &mut Cps2Bus, a: u32) -> u16 {
    if a & 0xff_ffff >= ROM_END {
        return crate::mem::rd16(b, a);
    }
    bytes(false, a, 2).map_or_else(|| missing(a).read16(b, a), |v| v as u16)
}

/// Long of the data space at `a`.
pub fn long(b: &mut Cps2Bus, a: u32) -> u32 {
    if a & 0xff_ffff >= ROM_END {
        return crate::mem::rd32(b, a);
    }
    bytes(false, a, 4).unwrap_or_else(|| missing(a).read32(b, a))
}

/// Byte of the program space (table read PC-relative).
pub fn pc_byte(b: &mut Cps2Bus, a: u32) -> u8 {
    bytes(true, a, 1).map_or_else(|| missing(a).byte_in(b, a, true), |v| v as u8)
}

/// Word of the program space.
pub fn pc_word(b: &mut Cps2Bus, a: u32) -> u16 {
    bytes(true, a, 2).map_or_else(|| missing(a).word_in(b, a, true), |v| v as u16)
}

/// Long of the program space.
pub fn pc_long(b: &mut Cps2Bus, a: u32) -> u32 {
    bytes(true, a, 4).unwrap_or_else(|| missing(a).long_in(b, a, true))
}

/// A ROM table or text of the screens, designated by its address (the key the original code uses).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Rom(pub RomAddr);

impl Rom {
    pub const fn at(a: u32) -> Rom {
        Rom(RomAddr(a))
    }
    /// The address, as the original keeps it in its state or passes it on.
    pub const fn addr(self) -> u32 {
        self.0 .0
    }
    pub fn byte(self, b: &mut Cps2Bus, off: u32) -> u8 {
        byte(b, self.addr().wrapping_add(off))
    }
    pub fn word(self, b: &mut Cps2Bus, off: u32) -> u16 {
        word(b, self.addr().wrapping_add(off))
    }
    pub fn long(self, b: &mut Cps2Bus, off: u32) -> u32 {
        long(b, self.addr().wrapping_add(off))
    }
    pub fn pc_byte(self, b: &mut Cps2Bus, off: u32) -> u8 {
        pc_byte(b, self.addr().wrapping_add(off))
    }
    pub fn pc_word(self, b: &mut Cps2Bus, off: u32) -> u16 {
        pc_word(b, self.addr().wrapping_add(off))
    }
    pub fn pc_long(self, b: &mut Cps2Bus, off: u32) -> u32 {
        pc_long(b, self.addr().wrapping_add(off))
    }
}
