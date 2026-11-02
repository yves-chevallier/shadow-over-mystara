//! Tools: the natives of `crate::scene::shop` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::scene::shop::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::scene::Ctx;
use crate::video::{clear_layer, Layer};
use crate::mem::{prog32, prog8, rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::modes::{Flow, Sequence};
use crate::objects::{pools, Obj};

pub const SHOP: Native = Native {
    entry: 0xf792e,
    name: "scene_shop",
    run: crate::Body::WithCpu(|c, b| {
        // entry for the original code (tools): returns only if the state did not cede control
        if run(&mut Ctx::new(b)) == Flow::Done {
            crate::tools::frames::rts(c, b);
        }
    }),
    touches_sound: false,
    outputs: 0,
};
