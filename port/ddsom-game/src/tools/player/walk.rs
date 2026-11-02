//! Tools: the natives of `crate::player::walk` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::player::walk::*;
use crate::tools::frames::*;
use crate::{regs, Native};
use crate::player::Player;

/// 68000 entry (0x35c0): D0 = 1 if the abscissa was corrected, A1 = camera.
fn clamp_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let cam = crate::objects::cull::camera(b);
    c.d[0] = clamp_to_screen(b, Player(c.a[0])) as u32;
    c.a[1] = cam;
    rts(c, b);
}

pub(crate) const CLAMP: Native = Native {
    entry: 0x35c0,
    name: "player_clamp_to_screen",
    run: crate::Body::WithCpu(clamp_native),
    touches_sound: false,
    outputs: regs::d(0) | regs::a(1),
};
