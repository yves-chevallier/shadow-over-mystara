//! Tools: the natives of `crate::combat` for the original code that calls them (arguments and results in the registers
//! of the original).

use crate::world::World;
use m68k::Cpu;

use crate::combat::*;
use crate::Native;
use crate::dispatch::Leftovers;

/// 0x260ee: the frame's collisions, for a caller still on the processor (game loop, natives for the tools): the
/// leftovers D2-D5 go from the register bench to `Combat::leftovers` and back; nothing else is read or written there.
pub(crate) fn on_bench(c: &mut Cpu, b: &mut World) {
    let leftovers = Leftovers { d2: c.d[2], d3: c.d[3], d4: c.d[4], d5: c.d[5] };
    let mut w = Combat { b, a2: 0, leftovers };
    combat_update(&mut w);
    let l = w.leftovers;
    (c.d[2], c.d[3], c.d[4], c.d[5]) = (l.d2, l.d3, l.d4, l.d5);
}

fn combat_update_native(c: &mut Cpu, b: &mut World) {
    on_bench(c, b);
    crate::tools::frames::rts(c, b);
}

pub(crate) const COMBAT_UPDATE: Native = Native {
    entry: 0x260ee,
    name: "combat_update",
    run: crate::Body::WithCpu(combat_update_native),
    touches_sound: false,
    outputs: 0,
};
