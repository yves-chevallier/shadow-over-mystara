//! Tools: the natives of `crate::families::boss_common` for the original code that calls them (arguments and results in the registers
//! of the original).

use m68k::decode::Size;
use m68k::Cpu;

use crate::families::boss_common::*;
use crate::tools::frames::*;
use crate::{regs, Native};
use crate::families::cx::Cx;

/// Native of a boss function called by address (the RTS is added).
macro_rules! native {
    ($entry:expr, $name:expr, $outputs:expr, $f:expr) => {
        Native {
            entry: $entry,
            name: $name,
            run: crate::Body::WithCpu(|c, b| {
                $f(&mut Cx::new(c, b));
                rts(c, b);
            }),
            touches_sound: false,
            outputs: $outputs,
        }
    };
}

/// `push_out_of_boss` with the original's registers: D0 = 1 / 0 (flags of `tst.l`), A0 intact, A1 = A6 =
/// l'objet.
fn push_out_of_boss_68k(x: &mut Cx<'_, &mut Cpu>) {
    let moved = push_out_of_boss(x);
    let me = x.o.0;
    x.c.a[6] = me;
    x.c.a[1] = me;
    x.c.d[0] = moved as u32;
    x.c.flags_logic(x.c.d[0], Size::Long);
    x.c.a[0] = me;
}

pub(crate) const NATIVES: &[Native] = &[
    native!(0x81aa4, "boss_repousse", regs::d(0) | regs::a(0) | regs::a(1) | regs::a(6) | regs::CCR, push_out_of_boss_68k),
    native!(0x833b6, "boss_oscillation", regs::d(0), bob),
    native!(0x87048, "boss_hp_threshold", regs::d(0) | regs::d(1), hp_threshold),
];
