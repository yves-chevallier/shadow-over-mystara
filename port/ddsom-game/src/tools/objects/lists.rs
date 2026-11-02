//! Tools: the natives of `crate::objects::lists` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::objects::lists::*;
use crate::tools::frames::*;
use crate::{regs, Native};
use crate::objects::Obj;

/// `unlink_all` with the original's register: A0 = last object removed (unchanged if the lists are empty).
#[track_caller]
pub(crate) fn unlink_all_68k(c: &mut Cpu, b: &mut Cps2Bus) -> Option<Obj> {
    let last = unlink_all_regs(c, b);
    crate::tools::diag::dead_regs(c, "lists", regs::a(0));
    last
}

fn unlink_all_regs(c: &mut Cpu, b: &mut Cps2Bus) -> Option<Obj> {
    let last = unlink_all(b);
    if let Some(o) = last {
        c.a[0] = o.0 | 0xff00_0000; // movea.w: sign-extended
    }
    last
}

fn reinsert_native(list: SortedList, c: &mut Cpu, b: &mut Cps2Bus) {
    let obj = Obj(c.a[0]);
    Reinsert { c, b, obj, list }.run();
    rts(c, b);
}

/// 0x1f0be: removes object A0 from its list. The original leaves no useful register: the Rust callers
/// call `unlink` directly.
fn unlink_native(c: &mut Cpu, b: &mut Cps2Bus) {
    unlink(b, Obj(c.a[0]));
    rts(c, b);
}

/// 0x1f184: empties both lists.
fn unlink_all_native(c: &mut Cpu, b: &mut Cps2Bus) {
    unlink_all_regs(c, b);
    rts(c, b);
}

pub(crate) const REINSERT_ACTORS: Native = Native {
    entry: 0x1ee1a,
    name: "list_reinsert_actors",
    run: crate::Body::WithCpu(|c, b| reinsert_native(ACTORS, c, b)),
    touches_sound: false,
    outputs: regs::ALL,
};

pub(crate) const REINSERT_FOREGROUND: Native = Native {
    entry: 0x1ef6c,
    name: "list_reinsert_foreground",
    run: crate::Body::WithCpu(|c, b| reinsert_native(FOREGROUND, c, b)),
    touches_sound: false,
    outputs: regs::ALL,
};

pub(crate) const UNLINK: Native = Native {
    entry: 0x1f0be,
    name: "list_unlink",
    run: crate::Body::WithCpu(unlink_native),
    touches_sound: false,
    outputs: regs::a(0),
};

pub(crate) const UNLINK_ALL: Native = Native {
    entry: 0x1f184,
    name: "list_unlink_all",
    run: crate::Body::WithCpu(unlink_all_native),
    touches_sound: false,
    outputs: regs::a(0),
};

/// Registers left by the original reinsertion (see `diag::dead_regs`, label `lists`).
pub(crate) const REGS_REINSERT: u32 =
    regs::d(0) | regs::d(1) | regs::d(2) | regs::a(1) | regs::a(2) | regs::a(3) | regs::a(4) | regs::CCR;

impl SortedList {
    /// `reinsert` with the original's registers, depending on the path followed (see `Reinsert`).
    #[track_caller]
    pub(crate) fn reinsert_68k(self, c: &mut Cpu, b: &mut Cps2Bus, obj: Obj) {
        Reinsert { c, b, obj, list: self }.run();
        crate::tools::diag::dead_regs(c, "lists", REGS_REINSERT);
    }
}
