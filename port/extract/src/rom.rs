//! Reading the program ROM: data (68000 reads) and program space (decrypted).

use std::io;
use std::path::Path;

pub struct Program {
    data: Vec<u16>,
    prog: Vec<u16>,
}

impl Program {
    pub fn load(roms: &Path) -> io::Result<Self> {
        let (data, prog) = cps2::load_program(roms)?;
        Ok(Program { data, prog })
    }

    /// The program of a machine already loaded: data view and decrypted program view, in words.
    pub fn from_words(data: Vec<u16>, prog: Vec<u16>) -> Self {
        Program { data, prog }
    }

    pub fn len(&self) -> u32 {
        2 * self.data.len() as u32
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn rd8(&self, a: u32) -> u8 {
        let w = self.rd16(a & !1);
        if a & 1 == 0 {
            (w >> 8) as u8
        } else {
            w as u8
        }
    }

    pub fn rd16(&self, a: u32) -> u16 {
        self.data.get((a >> 1) as usize).copied().unwrap_or(0)
    }

    pub fn rd32(&self, a: u32) -> u32 {
        (self.rd16(a) as u32) << 16 | self.rd16(a.wrapping_add(2)) as u32
    }

    /// Word read in program space (tables read by the code with PC-relative addressing).
    pub fn prog16(&self, a: u32) -> u16 {
        self.prog.get((a >> 1) as usize).copied().unwrap_or(0)
    }

    /// Byte read in program space.
    pub fn prog8(&self, a: u32) -> u8 {
        let w = self.prog16(a & !1);
        if a & 1 == 0 {
            (w >> 8) as u8
        } else {
            w as u8
        }
    }

    /// Long read in program space.
    pub fn prog32(&self, a: u32) -> u32 {
        (self.prog16(a) as u32) << 16 | self.prog16(a + 2) as u32
    }

    /// Read of a long in the chosen space.
    pub fn long(&self, a: u32, program: bool) -> u32 {
        if program {
            self.prog32(a)
        } else {
            self.rd32(a)
        }
    }
}
