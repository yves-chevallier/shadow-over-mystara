//! Tools: the natives of `crate::modes::ending` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::modes::ending::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::modes::{Flow, Sequence};
use crate::scene::fading;
use crate::effects_aids::scenery_6;
use crate::families::creators::set_15_pure;
use crate::families::effects_creators::end_screen;
use crate::palette::{load_palette_set, load_palette_page};
use crate::video::{load_grid, clear_layer, Layer, Grid};
use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::scene::Ctx;
use crate::objects::{pools, Obj};

pub const PHASE_28: Native = Native {
    entry: 0xfb292,
    name: "ending_phase_28",
    run: crate::Body::WithCpu(|c, b| {
        // entry for the original code (tools): returns only if the phase did not cede control
        if phase_28(&mut Ctx::new(b)) == Flow::Done {
            rts(c, b);
        }
    }),
    touches_sound: false,
    outputs: 0,
};
