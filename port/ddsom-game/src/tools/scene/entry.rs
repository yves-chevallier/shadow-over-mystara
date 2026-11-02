//! Tools: the natives of `crate::scene::entry` for the original code that calls them (arguments and results in the registers
//! of the original).


use crate::scene::entry::*;
use crate::tools::frames::*;
use crate::Native;
use crate::scene::{PLAYERS, PLAYER_STRIDE};

/// 0x20de6 called by the original code (respawn of a player, `families/creators.rs`): `place_players`, with
/// the original's registers (D6 = -1, A1 after the last player).
pub(crate) const PLACE_PLAYERS: Native = Native {
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

pub(crate) const PHASE_8: Native = Native {
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
