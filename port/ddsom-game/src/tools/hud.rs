//! Tools: the natives of `crate::hud` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::hud::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::hud::slot::forget_displayed;
use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::modes::Flow;
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
