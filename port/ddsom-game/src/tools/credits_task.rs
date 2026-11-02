//! Tools: the natives of `crate::credits_task` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::credits_task::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::credits_screen::{self, FIRST_COIN_SLOT};
use crate::scene::fading;
use crate::modes::{Flow, Sequence};
use crate::mem::{rd16, rd8, wr16, wr32, wr8, A5};
use crate::objects::{pools, Obj};
use crate::scene::Ctx;
use crate::tasks;
use crate::video::{load_grid, Grid};

/// Original subroutine rewritten in the body of the task, presented as a native for the differential fuzz
/// (`examples/fuzz_couv`); the game does not call it by its address. If it cedes control, the trial is discarded
/// (panic).
pub const TEST_NATIVES: &[crate::Native] = &[crate::Native {
    // 0x13c2c: credits state (startup, resumption, rallying of the four slots)
    entry: 0x13c2c,
    name: "credits_state",
    run: crate::Body::WithCpu(|c, b| {
        if let Flow::Yield = credits(&mut Ctx::new(b)) {
            panic!("credits_etat: cedes control (outside the fuzz frame)");
        }
        crate::tools::frames::rts(c, b);
    }),
    touches_sound: false,
    outputs: 0,
}];
