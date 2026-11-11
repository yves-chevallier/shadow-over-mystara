//! Tools: the natives of `crate::popups` for the original code that calls them (arguments and results in the registers
//! of the original).

use crate::world::World;
use m68k::Cpu;

use crate::popups::*;
use crate::tools::frames::*;
use crate::Native;
use crate::mem::A5;

/// `animate_obj` with the registers of the original: A0 = the current pop-up, A2 = the queue, D0 = the index read.
/// `animate_obj` with the registers of the original (A0 = the current pop-up, A2 = the queue, D0 = the index read).
pub(crate) fn animate_68k(c: &mut Cpu, b: &mut World) {
    c.a[0] = A5.wrapping_add(CURRENT);
    c.a[2] = A5 + INPUTS;
    if let Some(index) = animate_obj(b) {
        c.d[0] = index as u32;
    }
}

pub(crate) const ANIMATE: Native = Native {
    entry: 0x19e20,
    name: "popups_animer",
    run: crate::Body::WithCpu(|c, b| {
        animate_68k(c, b);
        rts(c, b);
    }),
    touches_sound: false,
    outputs: 0,
};
