//! Tools: the natives of `crate::modes::demo` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::modes::demo::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::scene::fading;
use crate::modes::{mode, Block, Flow, Sequence};
use crate::effects_aids::scenery_6;
use crate::families::creators::set_15_pure;
use crate::objects::Obj;
use crate::video::{clear_layer, Layer};
use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::objects::pools;
use crate::scene::Ctx;

pub const RECORDER_RESET: Native = Native {
    entry: 0xe0538,
    name: "demo_recorder_start",
    run: crate::Body::WithCpu(|c, b| {
        let leftovers = reset_recorder_by(b);
        if let Some(dsw) = leftovers.settings {
            c.d[5] = c.d[5] & 0xffff_0000 | dsw as u32;
        }
        if let Some(demo) = leftovers.demo {
            c.a[0] = demo;
        }
        crate::tools::frames::rts(c, b);
    }),
    touches_sound: false,
    outputs: crate::regs::d(5) | crate::regs::a(0),
};

/// The demonstration game: AI inputs, world, gauges, collisions, spawn script.
pub const DEMO_PHASE_12: Native = Native {
    entry: 0x1fa38,
    name: "demo_phase_12",
    run: crate::Body::WithCpu(|c, b| {
        // entry for the original code (tools): returns only if the state did not cede control
        if demo_phase_12(&mut Ctx::new(b)) == Flow::Done {
            crate::tools::frames::rts(c, b);
        }
    }),
    touches_sound: false,
    outputs: 0,
};
