//! Tools: the natives of `crate::scene::exit` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::scene::exit::*;
use crate::tools::frames::*;
use crate::Native;
use crate::mem::{rd8, wr16};

pub(crate) const PHASE_16: Native = Native {
    entry: 0xf3142,
    name: "scene_phase_16",
    run: crate::Body::WithCpu(|c, b| {
        if !phase_16(b) {
            rts(c, b);
        }
    }),
    touches_sound: false,
    outputs: 0,
};

pub(crate) fn patch_shared_next(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    if rd8(b, c.a[6] + 0xc) < SHARED_NEXT_TABLE_LEN {
        return false;
    }
    wr16(b, c.a[6] + 0x12, 0x65);
    c.d[0] &= 0xffff_0000;
    c.a[1] = 0xf7b88;
    c.pc = SHARED_NEXT_NONE;
    true
}

const SHARED_NEXT_TABLE_LEN: u8 = 20;

const SHARED_NEXT_NONE: u32 = 0xf792c;
