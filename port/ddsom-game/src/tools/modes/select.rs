//! Tools: the natives of `crate::modes::select` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::modes::select::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::modes::{Flow, Sequence};
use crate::scene::fading;
use crate::dispatch;
use crate::families::effects_creators::{banner, element_d_interface};
use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::objects::{pools, Obj};
use crate::scene::{Ctx, PLAYERS, PLAYER_STRIDE};
use crate::video::{load_grid, clear_layer, Layer, Grid};

pub const PHASE_4: Native = Native {
    entry: 0x1ca20,
    name: "select_phase_4",
    run: crate::Body::WithCpu(|c, b| {
        // entry for the original code (tools): returns only if the phase did not cede control
        if phase_4(&mut Ctx::new(b)) == Flow::Done {
            rts(c, b);
        }
    }),
    touches_sound: false,
    outputs: 0,
};
