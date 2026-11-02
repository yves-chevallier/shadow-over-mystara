//! Tools: the natives of `crate::scene::tally` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::scene::tally::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::scene::tiles::{self, Number, COLUMN, LAYER};
use crate::scene::fading;
use crate::scene::Ctx;
use crate::mem::{prog16, prog32, prog8, rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::modes::{Flow, Sequence};
use crate::ops::abcd;
use crate::objects::Obj;

/// Original entry (exit sequences left in 68000).
pub const TALLY: Native = Native {
    entry: 0xf590e,
    name: "scene_tally",
    run: crate::Body::WithCpu(|c, b| {
        // entry for the original code (tools): returns only if the state did not cede control
        if run(&mut Ctx::new(b)) == Flow::Done {
            crate::tools::frames::rts(c, b);
        }
    }),
    touches_sound: false,
    outputs: 0,
};
