//! Tools: the natives of `crate::terrain` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::terrain::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::mem::{rd16, set_low_word, wr16, wr32, wr8, A5};
use crate::objects::Obj;
use crate::ops;

fn run(variant: Variant, c: &mut Cpu, b: &mut Cps2Bus) {
    Push { c, b, variant }.run();
    rts(c, b);
}

/// 0x33fe4 on object `o`, without registers: returns the original's Z flag (true: the last cell examined is
/// free, or the collision is disabled). The routine is transcribed on work registers (`Push`); here
/// these are private registers, discarded on return (the players' callers did not reread those of the original:
/// `DDSOM_POISON=terrain`, batch D).
/// Adapter: like `push`, but on the caller's registers, left as in the original (families of batch B,
/// registers not yet proven dead: `DDSOM_POISON=terrain`).
pub fn push_68k(c: &mut Cpu, b: &mut Cps2Bus, o: Obj) -> bool {
    c.a[0] = o.0;
    Push { c, b, variant: FIRST }.run();
    let z = c.sr & m68k::SR_Z != 0;
    crate::tools::diag::dead_regs(c, "terrain", regs::d(0) | regs::d(1) | regs::d(2) | regs::d(5) | regs::a(1) | regs::a(2) | regs::CCR);
    z
}

pub const SHADOW: Native = Native {
    entry: 0x33fe4,
    name: "terrain_push",
    run: crate::Body::WithCpu(|c, b| run(FIRST, c, b)),
    touches_sound: false,
    outputs: regs::ALL,
};

pub const SHADOW_SECOND: Native = Native {
    entry: 0x3403c,
    name: "terrain_push_2",
    run: crate::Body::WithCpu(|c, b| run(SECOND, c, b)),
    touches_sound: false,
    outputs: regs::ALL,
};
