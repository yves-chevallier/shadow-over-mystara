//! Tools: the natives of `crate::families::enemy_common` for the original code that calls them (arguments and results in the registers
//! of the original).


use crate::families::enemy_common::*;
use crate::families::cx::Cx;

pub(crate) const NATIVES: &[crate::Native] = &[crate::Native {
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
