//! Lecture de la ROM programme : données (lectures du 68000) et espace programme (déchiffré).

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

    pub fn len(&self) -> u32 {
        2 * self.data.len() as u32
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

    /// Mot lu en espace programme (tables lues par le code en adressage relatif au PC).
    pub fn prog16(&self, a: u32) -> u16 {
        self.prog.get((a >> 1) as usize).copied().unwrap_or(0)
    }

    /// Long lu en espace programme.
    pub fn prog32(&self, a: u32) -> u32 {
        (self.prog16(a) as u32) << 16 | self.prog16(a + 2) as u32
    }

    /// Lecture d'un long dans l'espace choisi.
    pub fn long(&self, a: u32, program: bool) -> u32 {
        if program {
            self.prog32(a)
        } else {
            self.rd32(a)
        }
    }
}
