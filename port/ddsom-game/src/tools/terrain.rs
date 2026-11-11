//! Tools: the natives of `crate::terrain` for the original code that calls them (arguments and results in the registers
//! of the original).

use crate::world::World;
use m68k::Cpu;

use crate::terrain::*;
use crate::tools::frames::*;
use crate::{regs, Native};

fn run(variant: Variant, c: &mut Cpu, b: &mut World) {
    Push { c, b, variant }.run();
    rts(c, b);
}

pub(crate) const SHADOW: Native = Native {
    entry: 0x33fe4,
    name: "terrain_push",
    run: crate::Body::WithCpu(|c, b| run(FIRST, c, b)),
    touches_sound: false,
    outputs: regs::ALL,
};

pub(crate) const SHADOW_SECOND: Native = Native {
    entry: 0x3403c,
    name: "terrain_push_2",
    run: crate::Body::WithCpu(|c, b| run(SECOND, c, b)),
    touches_sound: false,
    outputs: regs::ALL,
};
