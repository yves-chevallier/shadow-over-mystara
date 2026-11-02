//! Tools: the natives of `crate::combat` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::combat::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::dispatch::Leftovers;
use crate::mem::{rd16, rd32, rd8, wr16, wr8, A5};
use crate::objects::Obj;
use crate::objects::pools;
use crate::combat::cov::enable as enable_coverage;
use crate::combat::cov::report as coverage;
use crate::combat::contact::*;
use crate::combat::damage::*;
use crate::combat::shots::*;

/// 0x260ee: the frame's collisions, for a caller still on the processor (game loop, natives for the tools): the
/// leftovers D2-D5 go from the register bench to `World::leftovers` and back; nothing else is read or written there.
pub fn on_bench(c: &mut Cpu, b: &mut Cps2Bus) {
    let leftovers = Leftovers { d2: c.d[2], d3: c.d[3], d4: c.d[4], d5: c.d[5] };
    let mut w = World { b, a2: 0, leftovers };
    combat_update(&mut w);
    let l = w.leftovers;
    (c.d[2], c.d[3], c.d[4], c.d[5]) = (l.d2, l.d3, l.d4, l.d5);
}

fn combat_update_native(c: &mut Cpu, b: &mut Cps2Bus) {
    on_bench(c, b);
    crate::tools::frames::rts(c, b);
}

pub const COMBAT_UPDATE: Native = Native {
    entry: 0x260ee,
    name: "combat_update",
    run: crate::Body::WithCpu(combat_update_native),
    touches_sound: false,
    outputs: 0,
};
