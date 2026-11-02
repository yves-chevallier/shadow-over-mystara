//! Tools: the natives of `crate::scene::tally` for the original code that calls them (arguments and results in the registers
//! of the original).


use crate::scene::tally::*;
use crate::Native;
use crate::scene::Ctx;
use crate::modes::Flow;

/// Original entry (exit sequences left in 68000).
pub(crate) const TALLY: Native = Native {
    entry: 0xf590e,
    name: "scene_tally",
    run: crate::Body::WithCpu(|c, b| {
        // entry for the original code (tools): returns only if the state did not cede control
        if run(&mut Ctx::new(b)) == Flow::Done {
            crate::tools::frames::rts(c, b);
        }
    }),
    touches_sound: false,
    outputs: 0,
};
