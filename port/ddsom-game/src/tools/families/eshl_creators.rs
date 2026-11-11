//! Tools: the natives of `crate::actors::families::eshl_creators` for the original code that calls them (arguments and results in the registers
//! of the original).

use m68k::Cpu;

use crate::actors::families::eshl_creators::*;
use crate::tools::frames::*;
use crate::{regs, Native};
use crate::world::objects::Obj;

/// Natives of the creators (remaining calls by address): A0 = the creator, parameters in the registers; the function on
/// the processor, then `rts`.
macro_rules! native {
    ($entry:literal, $name:literal, |$c:ident, $b:ident, $me:ident| $body:expr, $outputs:expr) => {
        Native {
            entry: $entry,
            name: $name,
            run: crate::Body::WithCpu(|$c, $b| {
                let $me = Obj($c.a[0]);
                $body;
                rts($c, $b)
            }),
            touches_sound: false,
            outputs: $outputs,
        }
    };
}

/// D0.w, D1.w: parameters of the original.
fn d0w(c: &Cpu) -> u16 {
    c.d[0] as u16
}

fn d1w(c: &Cpu) -> u16 {
    c.d[1] as u16
}

pub(crate) const NATIVES: &[Native] = &[
    native!(0xa7b0a, "eshl_shards_44", |c, b, me| boss11_shards(c, b, me), CREATED | regs::d(0) | regs::d(1) | regs::d(2)),
    native!(0xa7570, "eshl_fragments_42", |c, b, me| { let v = d0w(c); fragments_42(c, b, me, v) }, CREATED | regs::d(1)),
    native!(0xa7da2, "eshl_fragments_45", |c, b, me| fragments_45(c, b, me), CREATED | regs::d(0) | regs::d(1) | regs::d(2)),
    native!(0xa4e84, "eshl_series_36", |c, b, me| { let i = c.d[4] as u8; series_36(c, b, me, i) }, CREATED | regs::d(5)),
    native!(0xa5c9a, "eshl_series_3b", |c, b, me| series_3b(c, b, me), CREATED | regs::d(5)),
    native!(0xa5c7c, "eshl_lie_3b", |c, b, me| single_3b(c, b, me), CREATED),
    native!(0xa6e6a, "eshl_lie_40", |c, b, me| single_40(c, b, me), CREATED),
    native!(0x9dda8, "eshl_thrown_1", |c, b, me| thrown_1(c, b, me), CREATED | regs::d(0)),
    native!(0x9e062, "eshl_thrown_2", |c, b, me| thrown_2(c, b, me), CREATED | regs::d(0)),
    native!(0x9ed78, "eshl_balaye_8", |c, b, me| sweeping_8(c, b, me), CREATED | regs::d(0)),
    native!(0x9eba6, "eshl_shot_6", |c, b, me| { let v = c.d[0] as u8; shot_6(c, b, me, v) }, CREATED | regs::d(0)),
    native!(0x9e194, "eshl_four_3", |c, b, me| four_3(c, b, me), CREATED | regs::d(1)),
    native!(0x9e2a0, "eshl_dessus_4", |c, b, me| above_4(c, b, me), CREATED),
    native!(0x9eede, "eshl_pair_9", |c, b, me| pair_9(c, b, me), CREATED | regs::d(0) | regs::a(2)),
    native!(0x9dbd6, "eshl_shot_0", |c, b, me| shot_0(c, b, me), CREATED),
    native!(0x9e426, "eshl_twelve_5", |c, b, me| { let n = d0w(c); twelve_5(c, b, me, n) }, CREATED | regs::d(0) | regs::d(1) | regs::d(2)),
    native!(0xa1670, "eshl_rangee_19", |c, b, me| row_19(c, b, me), CREATED | regs::d(0) | regs::d(3) | regs::d(4) | regs::d(5)),
    native!(0xa17d0, "eshl_rangee_1a", |c, b, me| { let (s, d5) = (d1w(c), c.d[5] as u16); row_1a(c, b, me, s, d5) }, CREATED | regs::d(0) | regs::d(1) | regs::d(3) | regs::d(4) | regs::d(5) | regs::a(2)),
    native!(0xa1c6a, "eshl_four_1c", |c, b, me| four_1c(c, b, me), 0),
    native!(0xa31c2, "eshl_ring_28", |c, b, me| ring_28(c, b, me), CREATED | regs::d(1)),
    native!(0xa2f96, "eshl_ring_27", |c, b, me| ring_27(c, b, me), CREATED | regs::d(1)),
    native!(0xa6824, "eshl_pair_3f", |c, b, me| pair_3f(c, b, me), CREATED),
    native!(0xa3354, "eshl_lie_29", |c, b, me| single_29(c, b, me), CREATED),
    native!(0x9ddee, "eshl_shot_1", |c, b, me| { let v = c.d[1] as u8; single_1(c, b, me, v) }, CREATED),
    native!(0xa460a, "eshl_lie_34_2", |c, b, me| linked_34(c, b, me, 2), CREATED),
    native!(0xa464e, "eshl_lie_34_1", |c, b, me| linked_34(c, b, me, 1), CREATED),
    native!(0xa4676, "eshl_lie_34_0", |c, b, me| linked_34(c, b, me, 0), CREATED),
    native!(0xa462c, "eshl_lie_34_3", |c, b, me| linked_34(c, b, me, 3), CREATED),
    native!(0xa2b4a, "eshl_lie_25", |c, b, me| single_25(c, b, me), CREATED),
    native!(0xa5f38, "eshl_lie_3c", |c, b, me| single_3c(c, b, me), CREATED),
    native!(0xa1360, "eshl_lie_18", |c, b, me| single_18(c, b, me), CREATED),
    native!(0xa1a40, "eshl_lie_1b", |c, b, me| { let s = d1w(c); single_1b(c, b, me, s) }, CREATED),
    native!(0xa44bc, "eshl_poursuivants_33", |c, b, me| { let (v, t) = (c.d[0] as u8, Obj(c.a[2])); chasers(c, b, me, v, t) }, CREATED | regs::d(0) | regs::d(1) | regs::d(2) | regs::d(3) | regs::d(4) | regs::d(5)),
];

const CREATED: u32 = regs::a(1) | regs::CCR;
