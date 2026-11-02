//! Tools: the natives of `crate::families::enemy_common` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::families::enemy_common::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::families::cx::Bench;
use crate::families::cx::Cx;
use crate::families::enemy_lib as lib;
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Current;
use crate::machine::Machine;

/// True if the allocation failed (carry set by the allocation routine).
pub fn alloc_failed(x: &Cx<'_, &mut Cpu>) -> bool {
    x.c.sr & SR_C != 0
}

/// `move.w #v,Dn`: changes only the low word of the register.
pub fn set_dw(x: &mut Cx<'_, &mut Cpu>, n: usize, v: u16) {
    x.c.d[n] = x.c.d[n] & 0xffff_0000 | v as u32;
}

pub const NATIVES: &[crate::Native] = &[crate::Native {
    entry: 0x27ee,
    name: "enemy_detach_host",
    run: crate::Body::WithCpu(|c, b| {
        // D0.w and A1: the link and the host, like the original
        let host = crate::mem::rd16(b, c.a[0] + f::HOST);
        c.d[0] = c.d[0] & 0xffff_0000 | host as u32;
        if host != 0 {
            c.a[1] = host as i16 as i32 as u32;
        }
        detach_from_host(&mut Cx::new(c, b));
        crate::tools::frames::rts(c, b);
    }),
    touches_sound: false,
    outputs: crate::regs::d(0) | crate::regs::a(1),
}];
