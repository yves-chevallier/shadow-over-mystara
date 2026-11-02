//! Tools: the natives of `crate::scene::entry` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::scene::entry::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::scene::fading;
use crate::scene::{player, scratch, script, var, Ctx, PLAYERS, PLAYER_STRIDE};
use crate::families::effects_creators as creators;
use crate::game;
use crate::mem::{rd16, rd8, wr16, wr32, wr8, A5};
use crate::objects::Obj;
use crate::objects::pools;

/// 0x20de6 called by the original code (respawn of a player, `families/creators.rs`): `place_players`, with
/// the original's registers (D6 = -1, A1 after the last player).
pub const PLACE_PLAYERS: Native = Native {
    entry: 0x20de6,
    name: "scene_place_players",
    run: crate::Body::WithCpu(|c, b| {
        let r = place_players_leftovers(b);
        (c.d[0], c.d[1], c.d[2], c.d[6]) = (r.d0, r.d1, r.rank as u32, 0xffff);
        (c.a[1], c.a[2], c.a[3]) = (PLAYERS + 4 * PLAYER_STRIDE, r.positions, r.states);
        crate::tools::frames::rts(c, b);
    }),
    touches_sound: false,
    outputs: crate::regs::ALL,
};

pub const PHASE_8: Native = Native {
    entry: 0x20c38,
    name: "scene_phase_8",
    run: crate::Body::WithCpu(|c, b| {
        // called by the original code (tools): A0 = the phase handler
        phase_8(b, c.a[0]);
        rts(c, b);
    }),
    touches_sound: false,
    outputs: 0,
};
