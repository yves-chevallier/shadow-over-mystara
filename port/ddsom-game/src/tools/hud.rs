//! Tools: the natives of `crate::hud` for the original code that calls them (arguments and results in the registers
//! of the original).


use crate::hud::*;
use crate::Native;
use crate::scene::Ctx;

/// Original subroutines rewritten inside the task body, presented as natives for the
/// differential fuzz (`examples/fuzz_couv`); the game does not call them by their address.
pub const TEST_NATIVES: &[Native] = &[Native {
    // 0x15216: a slot (A0 panel, A6 player, A4 coin counter)
    entry: 0x15216,
    name: "hud_slot",
    run: crate::Body::WithCpu(|c, b| {
        let s = Slot { panel: c.a[0], player: c.a[6], coins: c.a[4] };
        slot::step(&mut Ctx::new(b), s);
        crate::tools::frames::rts(c, b);
    }),
    touches_sound: false,
    outputs: 0,
}];
