//! Tools: the natives of `crate::player` for the original code that calls them (arguments and results in the registers
//! of the original).

pub(crate) mod actions;
pub(crate) mod init;
pub(crate) mod moves;
pub(crate) mod overlays;
pub(crate) mod scripted;
pub(crate) mod spawn;
pub(crate) mod update;
pub(crate) mod walk;

use crate::world::World;
use m68k::Cpu;

use crate::player::*;
use crate::dispatch::Leftovers;

/// Runs the players' code `f` for a caller still on the processor (game loop, natives for the tools): the leftovers
/// D2-D5 go from the register bench to `Ctx::leftovers` and back; nothing else is read or written there.
pub(crate) fn on_bench<T>(c: &mut Cpu, b: &mut World, f: impl FnOnce(&mut Ctx) -> T) -> T {
    let leftovers = Leftovers { d2: c.d[2], d3: c.d[3], d4: c.d[4], d5: c.d[5] };
    let mut g = Ctx { b, jumped: Jumped::default(), leftovers };
    let r = f(&mut g);
    let l = g.leftovers;
    (c.d[2], c.d[3], c.d[4], c.d[5]) = (l.d2, l.d3, l.d4, l.d5);
    r
}
