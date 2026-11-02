//! Sound driver ROM: program and data of MAME's "audiocpu" region.
//!
//! The Z80 sees a fixed 0000-7FFF and a 16 KB window at 8000-BFFF whose bank is selected at
//! D003. The driver handles "linear" pointers: the Z80 address of a byte of bank `b` at window
//! offset `o` is `0x8000 + 0x4000 * b + o`, i.e. the offset in the file `dd2.01` (followed by
//! `dd2.02`). Reading through a linear pointer amounts to what the Z80 does when switching to
//! the next bank each time it reaches C000.

use std::io;
use std::path::Path;

/// Size of MAME's audiocpu region.
const REGION_SIZE: usize = 0x50000;

#[derive(Clone)]
pub struct Rom {
    region: Vec<u8>,
}

impl Rom {
    /// Loads `dd2.01` and `dd2.02` like MAME: dd2.01 = 0x8000 fixed bytes then 0x18000 bytes
    /// of banks starting at 0x10000; dd2.02 starting at 0x28000.
    pub fn load(dir: &Path) -> io::Result<Self> {
        let mut region = vec![0u8; REGION_SIZE];
        let d1 = std::fs::read(dir.join("dd2.01"))?;
        region[..0x8000].copy_from_slice(&d1[..0x8000]);
        region[0x10000..0x28000].copy_from_slice(&d1[0x8000..0x20000]);
        let d2 = std::fs::read(dir.join("dd2.02"))?;
        region[0x28000..0x48000].copy_from_slice(&d2);
        Ok(Rom { region })
    }

    /// Builds the ROM from an already assembled audiocpu region.
    pub fn from_region(region: Vec<u8>) -> Self {
        Rom { region }
    }

    /// Complete audiocpu region (test tools: reference Z80).
    pub fn region(&self) -> &[u8] {
        &self.region
    }

    /// Byte at linear address `p` (0xFF outside the ROM, like the Z80's open bus).
    pub fn byte(&self, p: u32) -> u8 {
        let p = p & 0xff_ffff;
        let i = if p < 0x8000 { p } else { p + 0x8000 } as usize;
        self.region.get(i).copied().unwrap_or(0xff)
    }

    /// Little-endian word at linear address `p`.
    pub fn word(&self, p: u32) -> u16 {
        u16::from(self.byte(p)) | u16::from(self.byte(p + 1)) << 8
    }

    /// Entry `i` of a table of little-endian words located at `base`.
    pub fn table(&self, base: u32, i: u8) -> u16 {
        self.word(base + 2 * u32::from(i))
    }
}
