//! Tools: the natives of `crate::screens::modes::select` for the original code that calls them (arguments and results in the registers
//! of the original).


use crate::screens::modes::select::*;
use crate::tools::frames::*;
use crate::Native;
use crate::screens::modes::Flow;
use crate::screens::scene::Ctx;

pub(crate) const PHASE_4: Native = Native {
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
