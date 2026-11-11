//! Tools: the natives of `crate::actors::player::overlays` for the original code that calls them (arguments and results in the registers
//! of the original).

use m68k::Cpu;

use crate::actors::player::overlays::*;
use crate::tools::frames::*;
use crate::Native;

fn update_native(c: &mut Cpu, b: &mut crate::world::World) {
    super::on_bench(c, b, update);
    rts(c, b);
}

pub(crate) const UPDATE: Native = Native {
    entry: 0x392e4,
    name: "player_overlays_update",
    run: crate::Body::WithCpu(update_native),
    touches_sound: false,
    outputs: 0,
};
