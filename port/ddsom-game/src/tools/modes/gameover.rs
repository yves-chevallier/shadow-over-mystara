//! Tools: the natives of `crate::modes::gameover` for the original code that calls them (arguments and results in the registers
//! of the original).


use crate::modes::gameover::*;
use crate::tools::frames::*;
use crate::Native;
use crate::modes::Flow;
use crate::scene::Ctx;

pub(crate) const PHASE_20: Native = Native {
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
