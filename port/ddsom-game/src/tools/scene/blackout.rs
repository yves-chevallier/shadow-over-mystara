//! Tools: the natives of `crate::screens::scene::blackout` for the original code that calls them (arguments and results in the registers
//! of the original).


use crate::screens::scene::blackout::*;
use crate::Native;
use crate::screens::scene::Ctx;
use crate::screens::modes::Flow;

pub(crate) const BLACKOUT: Native = Native {
    entry: 0xf7ecc,
    name: "scene_blackout",
    run: crate::Body::WithCpu(|c, b| {
        // entry for the original code (tools): returns only if the state did not cede control
        if run(&mut Ctx::new(b)) == Flow::Done {
            crate::tools::frames::rts(c, b);
        }
    }),
    touches_sound: false,
    outputs: 0,
};
