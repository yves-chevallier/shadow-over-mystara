//! Tools: the natives of `crate::ai` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::ai::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::mem::{prog32, prog8, rd16, rd32, rd8};
use crate::objects::Obj;
use crate::player::Player;
use crate::ram::{self, Word, Byte, PLAYERS, PLAYERS_ALIVE};
use crate::rng;

/// `keep_on_screen` that also leaves the D5 of the original (`moveq #0,D5`, then 1 if the object moved).
/// Original glitch: D5 passes through the handlers of the dispatch loop, and the row of shards of a crate
/// (0xa17d0, `eshl_creators::row_1a`) rereads its high byte: so it is written in the clear.
pub fn keep_on_screen_d5(c: &mut m68k::Cpu, b: &mut Cps2Bus, o: Obj, margin: u16) -> bool {
    let moved = keep_on_screen(b, o, margin);
    c.d[5] = moved as u32;
    moved
}

/// `keep_in_zone` with the D5 of the original (see `keep_on_screen_d5`).
pub fn keep_in_zone_d5(c: &mut m68k::Cpu, b: &mut Cps2Bus, o: Obj) -> bool {
    keep_on_screen_d5(c, b, o, ZONE_MARGIN)
}
