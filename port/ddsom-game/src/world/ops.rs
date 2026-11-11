//! 68000 arithmetic that the game computes as the original did.

/// Byte ABCD: `dst + src + X` in binary-coded decimal. Returns the result and the carry
/// (which is also X). Algorithm of the real 68000 (also valid for invalid nibbles).
pub fn abcd(src: u8, dst: u8, x: bool) -> (u8, bool) {
    let (sv, dv, x) = (src as u32, dst as u32, x as u32);
    let sum = sv + dv + x;
    let mut adjust = 0;
    if (sv & 0x0f) + (dv & 0x0f) + x > 9 {
        adjust |= 0x06;
    }
    let carry = sum > 0x99;
    if carry {
        adjust |= 0x60;
    }
    ((sum + adjust) as u8, carry)
}

/// Operand size of a 68000 instruction: what the hooks of `families::effects_creators::leftovers::Regs` receive (the
/// game ignores the flags; the tools write them on the processor).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Size {
    Byte,
    Word,
    Long,
}

impl Size {
    /// Mask of an operand of this size.
    pub const fn mask(self) -> u32 {
        match self {
            Size::Byte => 0xff,
            Size::Word => 0xffff,
            Size::Long => 0xffff_ffff,
        }
    }
}

/// Condition flags (CCR bits): extend, negative, zero, overflow, carry.
pub const X: u16 = 0x10;
pub const N: u16 = 0x08;
pub const Z: u16 = 0x04;
pub const V: u16 = 0x02;
pub const C: u16 = 0x01;
/// All condition flags (X N Z V C).
pub const ALL: u16 = X | N | Z | V | C;

/// Registers and flags of the original, for the routines transcribed instruction by instruction whose results are
/// still registers and flags (`terrain::Push`, `objects::lists::Reinsert`). The game runs them on [`Registers`] and
/// keeps what is read afterwards (the leftovers D2-D5 of the following handlers, Z, C); in the tools, the processor
/// receives everything the original leaves (`tools::ops`).
pub trait Alu {
    /// Data register `n`.
    fn data(&self, n: usize) -> u32;
    fn d(&mut self) -> &mut [u32; 8];
    fn a(&mut self) -> &mut [u32; 8];
    /// MOVE / TST / AND / OR: N and Z of the value, V and C cleared, X kept.
    fn logic(&mut self, v: u32, size: Size);
    /// ADD: result (of the operand size) and flags (X included).
    fn add(&mut self, src: u32, dst: u32, size: Size) -> u32;
    /// SUB: `dst - src` (of the operand size), result and flags (X included).
    fn sub(&mut self, src: u32, dst: u32, size: Size) -> u32;
    /// CMP: flags of `dst - src`, X kept.
    fn cmp(&mut self, src: u32, dst: u32, size: Size);
    /// Sets the flags of `mask` (`X | N | Z | V | C`) to those of `values`.
    fn flags(&mut self, mask: u16, values: u16);
    fn zero(&self) -> bool;
    fn carry(&self) -> bool;
}

/// The registers that the game keeps for [`Alu`]: data and address registers, Z and C (the other flags are read by
/// nobody).
#[derive(Clone, Copy, Debug, Default)]
pub struct Registers {
    pub d: [u32; 8],
    pub a: [u32; 8],
    z: bool,
    c: bool,
}

impl Alu for Registers {
    fn data(&self, n: usize) -> u32 {
        self.d[n]
    }
    fn d(&mut self) -> &mut [u32; 8] {
        &mut self.d
    }
    fn a(&mut self) -> &mut [u32; 8] {
        &mut self.a
    }
    fn logic(&mut self, v: u32, size: Size) {
        self.z = v & size.mask() == 0;
        self.c = false;
    }
    fn add(&mut self, src: u32, dst: u32, size: Size) -> u32 {
        let m = size.mask() as u64;
        let r = (dst as u64 & m) + (src as u64 & m);
        self.c = r > m;
        self.z = r & m == 0;
        (r & m) as u32
    }
    fn sub(&mut self, src: u32, dst: u32, size: Size) -> u32 {
        self.cmp(src, dst, size);
        let m = size.mask();
        dst.wrapping_sub(src) & m
    }
    fn cmp(&mut self, src: u32, dst: u32, size: Size) {
        let m = size.mask();
        self.c = src & m > dst & m;
        self.z = dst.wrapping_sub(src) & m == 0;
    }
    fn flags(&mut self, mask: u16, values: u16) {
        if mask & Z != 0 {
            self.z = values & Z != 0;
        }
        if mask & C != 0 {
            self.c = values & C != 0;
        }
    }
    fn zero(&self) -> bool {
        self.z
    }
    fn carry(&self) -> bool {
        self.c
    }
}

/// `d - s - X` in BCD on a byte: result and carry (that of `sbcd`).
pub(crate) fn sbcd(d: u8, s: u8, x: bool) -> (u8, bool) {
    let (d, s, x) = (d as i32, s as i32, x as i32);
    let unadjusted = d - s - x;
    let mut adjust = 0;
    if (d & 0x0f) - (s & 0x0f) - x < 0 {
        adjust |= 0x06;
    }
    let borrow = unadjusted < 0 || unadjusted - adjust < 0;
    if unadjusted < 0 {
        adjust |= 0x60;
    }
    ((unadjusted - adjust) as u8, borrow)
}
