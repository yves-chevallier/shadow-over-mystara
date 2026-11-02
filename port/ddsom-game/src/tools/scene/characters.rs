//! Tools: the natives of `crate::scene::characters` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::scene::characters::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::scene::bonus::{dispatch, fade};
use crate::video::{clear_layer, Layer};
use crate::scene::Ctx;
use crate::mem::{prog8, rd16, rd8, wr16, wr32, wr8, A5};
use crate::modes::{Flow, Sequence};

pub const CHARACTERS: Native = Native {
    entry: 0xf6492,
    name: "scene_characters",
    run: crate::Body::WithCpu(|c, b| {
        // entry for the original code (tools): returns only if the state did not cede control
        if run(&mut Ctx::new(b)) == Flow::Done {
            crate::tools::frames::rts(c, b);
        }
    }),
    touches_sound: false,
    outputs: 0,
};
