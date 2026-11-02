//! Tools: the natives of `crate::player` for the original code that calls them (arguments and results in the registers
//! of the original).

pub mod actions;
pub mod init;
pub mod moves;
pub mod overlays;
pub mod scripted;
pub mod spawn;
pub mod update;
pub mod walk;

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::player::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::dispatch::Leftovers;
use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8, A5};
use crate::objects::Obj;

/// Runs the players' code `f` for a caller still on the processor (game loop, natives for the tools): the leftovers
/// D2-D5 go from the register bench to `Ctx::leftovers` and back; nothing else is read or written there.
pub fn on_bench<T>(c: &mut Cpu, b: &mut Cps2Bus, f: impl FnOnce(&mut Ctx) -> T) -> T {
    let leftovers = Leftovers { d2: c.d[2], d3: c.d[3], d4: c.d[4], d5: c.d[5] };
    let mut g = Ctx { b, jumped: Jumped::default(), leftovers };
    let r = f(&mut g);
    let l = g.leftovers;
    (c.d[2], c.d[3], c.d[4], c.d[5]) = (l.d2, l.d3, l.d4, l.d5);
    r
}
