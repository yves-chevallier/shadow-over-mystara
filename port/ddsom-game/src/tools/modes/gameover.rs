//! Tools: the natives of `crate::modes::gameover` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::modes::gameover::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::modes::{mode, Flow, Sequence};
use crate::mem::{rd16, wr16, wr32, wr8, A5};
use crate::scene::Ctx;

pub const PHASE_20: Native = Native {
    entry: 0x1c80c,
    name: "gameover_phase_20",
    run: crate::Body::WithCpu(|c, b| {
        // entry for the original code (tools): returns only if the phase did not cede control
        if phase_20(&mut Ctx::new(b)) == Flow::Done {
            rts(c, b);
        }
    }),
    touches_sound: false,
    outputs: 0,
};
