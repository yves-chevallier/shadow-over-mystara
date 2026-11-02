//! Tools: the natives of `crate::scene::bonus` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::scene::bonus::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::scene::screen::{self, copy_page};
use crate::scene::fading;
use crate::scene::{Ctx, PLAYERS, PLAYER_STRIDE};
use crate::mem::{prog16, rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::modes::{Flow, Sequence};
use crate::families::effects_creators::{end_screen, or_rest};
use crate::objects::{pools, Obj};
use crate::video::{load_grid, clear_layer, Layer, Grid};

/// Original entry (sequences still in 68000).
pub const BONUS: Native = Native {
    entry: 0xf5938,
    name: "scene_bonus",
    run: crate::Body::WithCpu(|c, b| {
        // entry for the original code (tools): returns only if the state did not cede control
        if run(&mut Ctx::new(b)) == Flow::Done {
            crate::tools::frames::rts(c, b);
        }
    }),
    touches_sound: false,
    outputs: 0,
};
