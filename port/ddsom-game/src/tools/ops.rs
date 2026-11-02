//! Tools: 68000 instructions with flags, for the natives that must leave exactly the same flags as the original
//! (the original code that calls them may read them on return), and the processor as an [`Alu`].

use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_X, SR_Z};

use crate::ops::Alu;

impl From<crate::ops::Size> for Size {
    fn from(s: crate::ops::Size) -> Self {
        match s {
            crate::ops::Size::Byte => Size::Byte,
            crate::ops::Size::Word => Size::Word,
            crate::ops::Size::Long => Size::Long,
        }
    }
}

/// The processor: all the flags of the original (the CCR bits of `crate::ops` are those of the SR).
impl Alu for Cpu {
    fn data(&self, n: usize) -> u32 {
        self.d[n]
    }
    fn d(&mut self) -> &mut [u32; 8] {
        &mut self.d
    }
    fn a(&mut self) -> &mut [u32; 8] {
        &mut self.a
    }
    fn logic(&mut self, v: u32, size: crate::ops::Size) {
        logic(self, v, size.into());
    }
    fn add(&mut self, src: u32, dst: u32, size: crate::ops::Size) -> u32 {
        add(self, src, dst, size.into())
    }
    fn sub(&mut self, src: u32, dst: u32, size: crate::ops::Size) -> u32 {
        sub(self, src, dst, size.into())
    }
    fn cmp(&mut self, src: u32, dst: u32, size: crate::ops::Size) {
        cmp(self, src, dst, size.into());
    }
    fn flags(&mut self, mask: u16, values: u16) {
        flags(self, mask, values);
    }
    fn zero(&self) -> bool {
        self.sr & SR_Z != 0
    }
    fn carry(&self) -> bool {
        self.sr & SR_C != 0
    }
}

/// CMP / CMPI: flags of `dst - src`; X is preserved.
pub(crate) fn cmp(c: &mut Cpu, src: u32, dst: u32, size: Size) {
    let x = c.sr & SR_X;
    c.addsub(true, src, dst, false, size, false);
    c.sr = c.sr & !SR_X | x;
}

/// ADD / ADDQ: result and flags (X included).
pub(crate) fn add(c: &mut Cpu, src: u32, dst: u32, size: Size) -> u32 {
    c.addsub(false, src, dst, false, size, false)
}

/// SUB / SUBQ / SUBI: `dst - src`, result and flags (X included).
pub(crate) fn sub(c: &mut Cpu, src: u32, dst: u32, size: Size) -> u32 {
    c.addsub(true, src, dst, false, size, false)
}

/// Condition codes of `Cpu::cond` (Bcc / Scc).
pub(crate) mod cc {
    pub(crate) const PL: u8 = 10;
    pub(crate) const GE: u8 = 12;
    pub(crate) const GT: u8 = 14;
    pub(crate) const LE: u8 = 15;
}

/// MOVE / TST / AND / OR...: N and Z from the value, V and C cleared, X preserved.
pub(crate) fn logic(c: &mut Cpu, v: u32, size: Size) {
    c.flags_logic(v, size);
}

/// Sets the N, Z, V, C, X flags at once (mask `SR_N | ...`).
pub(crate) fn flags(c: &mut Cpu, mask: u16, values: u16) {
    c.sr = c.sr & !mask | values & mask;
}
