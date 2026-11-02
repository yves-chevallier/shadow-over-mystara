//! Tools: the natives of `crate::sprites` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::sprites::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8};

/// `build` with the registers of the original (D4, A4, A6 when the table is built).
pub fn build_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    if let Some(r) = build(b, c.d[4]) {
        (c.d[4], c.a[4], c.a[6]) = (r.d4, r.end, r.camera);
    }
}

fn build_sprite_list(c: &mut Cpu, b: &mut Cps2Bus) {
    build_68k(c, b);
    rts(c, b);
}

/// Object frame without a piece (zero count). The original then loops with `dbf` on a counter of -1: 65,536 entries
/// written one after another, going around the object RAM several times, over several frames (the task is
/// interrupted by IRQs in the middle of the loop, at cycles that only cycle-exact routines
/// would reproduce). This only happens in states that the game does not produce (scene exit forced at any
/// instant, coverage scenarios): no piece is drawn, as here (`emit_frame`). Same patch in the original
/// code (reference machine), at the four entries of the emission loop.
pub const EMPTY_FRAME_PATCHES: [cps2::Patch; 4] = [
    // object: count read from D6 (0x1e22a), attached object (0x1e1aa); individual pieces: count in D4 (0x1e110);
    // object with individual pieces: D6 already decremented (0x1e2aa)
    cps2::Patch { entry: 0x1e22a, apply: |c, b| return_if(c, b, c.d[6] as u16 == 0) },
    cps2::Patch { entry: 0x1e1aa, apply: |c, b| return_if(c, b, c.d[6] as u16 == 0) },
    cps2::Patch { entry: 0x1e110, apply: |c, b| return_if(c, b, c.d[4] as u16 == 0) },
    cps2::Patch { entry: 0x1e2aa, apply: |c, b| return_if(c, b, c.d[6] as u16 == 0xffff) },
];

/// `rts` if `empty`.
fn return_if(c: &mut Cpu, b: &mut Cps2Bus, empty: bool) -> bool {
    if empty {
        c.pc = crate::mem::rd32(b, c.a[7]);
        c.a[7] = c.a[7].wrapping_add(4);
    }
    empty
}

pub const BUILD_SPRITE_LIST: Native = Native {
    entry: 0x1de96,
    name: "build_sprite_list",
    run: crate::Body::WithCpu(build_sprite_list),
    touches_sound: false,
    outputs: 0,
};
