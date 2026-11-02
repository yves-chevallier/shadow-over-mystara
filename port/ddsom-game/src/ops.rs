//! 68000 instructions with flags, for the native routines that must leave exactly the
//! same flags as the original (callers may read them on return).

use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

/// CMP / CMPI: flags of `dst - src`; X is preserved.
pub fn cmp(c: &mut Cpu, src: u32, dst: u32, size: Size) {
    let x = c.sr & SR_X;
    c.addsub(true, src, dst, false, size, false);
    c.sr = c.sr & !SR_X | x;
}

/// ADD / ADDQ: result and flags (X included).
pub fn add(c: &mut Cpu, src: u32, dst: u32, size: Size) -> u32 {
    c.addsub(false, src, dst, false, size, false)
}

/// SUB / SUBQ / SUBI: `dst - src`, result and flags (X included).
pub fn sub(c: &mut Cpu, src: u32, dst: u32, size: Size) -> u32 {
    c.addsub(true, src, dst, false, size, false)
}

/// Condition codes of `Cpu::cond` (Bcc / Scc).
pub mod cc {
    pub const HI: u8 = 2;
    pub const LS: u8 = 3;
    pub const CC: u8 = 4;
    pub const CS: u8 = 5;
    pub const NE: u8 = 6;
    pub const EQ: u8 = 7;
    pub const PL: u8 = 10;
    pub const MI: u8 = 11;
    pub const GE: u8 = 12;
    pub const LT: u8 = 13;
    pub const GT: u8 = 14;
    pub const LE: u8 = 15;
}

/// MOVE / TST / AND / OR...: N and Z from the value, V and C cleared, X preserved.
pub fn logic(c: &mut Cpu, v: u32, size: Size) {
    c.flags_logic(v, size);
}

/// Sets the N, Z, V, C, X flags at once (mask `SR_N | ...`).
pub fn drapeaux(c: &mut Cpu, mask: u16, values: u16) {
    c.sr = c.sr & !mask | values & mask;
}

/// All condition flags (X N Z V C).
pub const ALL: u16 = SR_X | SR_N | SR_Z | SR_V | SR_C;

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
