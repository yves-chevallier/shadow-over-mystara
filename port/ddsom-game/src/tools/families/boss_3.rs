//! Tools: the natives of `crate::families::boss_3` for the original code that calls them (arguments and results in the registers
//! of the original).

pub(crate) mod body;


use crate::families::boss_3::*;
use crate::families::boss_common::*;

/// Natives for the original code: counterattack and body shapes (called by 0x81aa4 of `boss_common`).
pub(crate) const NATIVES: &[crate::Native] = &[
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
