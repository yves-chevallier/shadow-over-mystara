//! Tools: the natives of `crate::scene::bonus` for the original code that calls them (arguments and results in the registers
//! of the original).


use crate::scene::bonus::*;
use crate::Native;
use crate::scene::Ctx;
use crate::modes::Flow;

/// Original entry (sequences still in 68000).
pub(crate) const BONUS: Native = Native {
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
