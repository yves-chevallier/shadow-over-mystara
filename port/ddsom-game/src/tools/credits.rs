//! Tools: the natives of `crate::credits` for the original code that calls them (arguments and results in the registers
//! of the original).


use crate::credits::*;
use crate::Native;

/// Original subroutine rewritten in the body of the credits task, presented as a native for the differential
/// fuzz (`examples/fuzz_couv`); the game does not call it by its address.
pub const TEST_NATIVES: &[Native] = &[Native {
    // 0x1422e: start button of a coin slot (A0 coin slot, D1.b byte of IN2+1)
    entry: 0x1422e,
    name: "credits_start_line",
    run: crate::Body::WithCpu(|c, b| {
        let (a0, service) = (c.a[0], c.d[1] as u8);
        line_start(b, a0, service);
        crate::tools::frames::rts(c, b);
    }),
    touches_sound: false,
    outputs: 0,
}];
