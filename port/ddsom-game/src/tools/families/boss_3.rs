//! Tools: the natives of `crate::families::boss_3` for the original code that calls them (arguments and results in the registers
//! of the original).

pub mod body;

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::families::boss_3::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::families::cx::Bench;
use crate::families::boss_common::*;
use crate::objects::Obj;
use crate::families::boss_3::g::*;
use crate::objects::pools;
use crate::objects::Current;

/// Natives for the original code: counterattack and body shapes (called by 0x81aa4 of `boss_common`).
pub const NATIVES: &[crate::Native] = &[
    crate::Native {
        entry: 0x81ae0,
        name: "boss_3_riposte",
        run: crate::Body::WithCpu(|c, b| {
            let mut x = Cx::new(c, b);
            let (r, mask) = counter_reaction(&mut x);
            // original's output contract: D0 = the draw, D1.w = the mask
            x.c.d[0] = r;
            x.c.d[1] = x.c.d[1] & 0xffff_0000 | mask as u32;
            x.ret();
        }),
        touches_sound: false,
        outputs: crate::regs::d(0) | crate::regs::d(1),
    },
    body::SHOVE_FRONT,
    body::SHOVE_SIDE,
];
