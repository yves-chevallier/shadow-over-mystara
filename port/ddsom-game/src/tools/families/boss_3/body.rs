//! Tools: the natives of `crate::families::boss_3::body` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use crate::families::boss_3::body::*;
use crate::tools::frames::*;
use crate::objects::Obj;

/// Adapter for the natives (fuzz against the original): A0 = the boss, A1 = the object; D0 = 1 / 0 and its flags. The
/// only original caller, 0x81aa4 (`boss_common::push_out_of_boss`), re-reads neither D1 nor A2, which the original
/// leaves.
fn native(c: &mut Cpu, b: &mut Cps2Bus, f: fn(&mut Cps2Bus, Obj, Obj) -> bool) {
    c.d[0] = f(b, Obj(c.a[0]), Obj(c.a[1])) as u32;
    c.flags_logic(c.d[0], Size::Long);
    rts(c, b);
}

pub(crate) const SHOVE_FRONT: crate::Native = crate::Native {
    entry: 0x818e6,
    name: "boss_3_pushes_front",
    run: crate::Body::WithCpu(|c, b| native(c, b, shove_front)),
    touches_sound: false,
    outputs: crate::regs::d(0) | crate::regs::CCR,
};

pub(crate) const SHOVE_SIDE: crate::Native = crate::Native {
    entry: 0x819e2,
    name: "boss_3_pushes_side",
    run: crate::Body::WithCpu(|c, b| native(c, b, shove_side)),
    touches_sound: false,
    outputs: crate::regs::d(0) | crate::regs::CCR,
};
