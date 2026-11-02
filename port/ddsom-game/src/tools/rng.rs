//! Tools: the natives of `crate::rng` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::rng::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::ram::RNG_SEED;

/// `random` with the registers of the original (native): D0.w = new state (D0 high intact), N and Z of the word, X =
/// carry of the byte addition.
fn random_regs(c: &mut Cpu, b: &mut Cps2Bus) -> u16 {
    let (state, carry) = draw(b);
    c.d[0] = c.d[0] & 0xffff_0000 | state as u32;
    // final move.w: N and Z of the result; X comes from the byte addition
    c.flags_logic(state as u32, Size::Word);
    c.sr = c.sr & !SR_X | if carry { SR_X } else { 0 };
    state
}

/// `seed` with the flags of the original (`move.w`: N and Z of the seed).
#[track_caller]
pub fn seed_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    seed(b);
    c.flags_logic(DEFAULT_SEED as u32, Size::Word);
    crate::tools::diag::dead_regs(c, "random", regs::CCR);
}

pub const SEED: Native = Native {
    entry: 0x19a80,
    name: "rng_seed",
    run: crate::Body::WithCpu(|c, b| {
        seed(b);
        c.flags_logic(DEFAULT_SEED as u32, Size::Word);
        rts(c, b);
    }),
    touches_sound: false,
    outputs: regs::CCR,
};

pub const NEXT: Native = Native {
    entry: 0x19a88,
    name: "rng_next",
    run: crate::Body::WithCpu(|c, b| {
        random_regs(c, b);
        rts(c, b);
    }),
    touches_sound: false,
    outputs: regs::d(0) | regs::CCR,
};
