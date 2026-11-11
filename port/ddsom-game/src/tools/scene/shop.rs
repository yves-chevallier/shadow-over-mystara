//! Tools: the natives of `crate::screens::scene::shop` for the original code that calls them (arguments and results in the registers
//! of the original).


use crate::screens::scene::shop::*;
use crate::Native;
use crate::world::tasks::Ctx;
use crate::world::tasks::Flow;

pub(crate) const SHOP: Native = Native {
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
