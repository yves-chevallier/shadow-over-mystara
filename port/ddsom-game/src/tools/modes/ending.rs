//! Tools: the natives of `crate::screens::modes::ending` for the original code that calls them (arguments and results in the registers
//! of the original).


use crate::screens::modes::ending::*;
use crate::tools::frames::*;
use crate::Native;
use crate::screens::modes::Flow;
use crate::screens::scene::Ctx;

pub(crate) const PHASE_28: Native = Native {
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
