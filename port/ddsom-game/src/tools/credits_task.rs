//! Tools: the natives of `crate::screens::credits_task` for the original code that calls them (arguments and results in the registers
//! of the original).


use crate::screens::credits_task::*;
use crate::world::tasks::Flow;
use crate::world::tasks::Ctx;

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
