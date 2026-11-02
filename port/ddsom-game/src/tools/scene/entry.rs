//! Tools: the natives of `crate::scene::entry` for the original code that calls them (arguments and results in the registers
//! of the original).


use crate::scene::entry::*;
use crate::tools::frames::*;
use crate::Native;
use crate::mem::{rd16, rd8, A5};
use crate::scene::{player, var, PLAYERS, PLAYER_STRIDE};
use m68k::decode::Size;

/// 0x20de6 called by the original code (respawn of a player, `families/creators.rs`): `place_players`, with
/// the original's registers (D6 = -1, A1 after the last player).
pub(crate) const PLACE_PLAYERS: Native = Native {
    entry: 0x20de6,
    name: "scene_place_players",
    run: crate::Body::WithCpu(|c, b| {
        let scene = rd8(b, A5 + var::SCENE) as u32;
        let rank = place_players(b) as u32;
        // D0/D1: the camera added to the last player placed, otherwise the table offsets of the scene
        (c.d[0], c.d[1]) = if rank == 0 {
            (scene << 4, scene << 2)
        } else {
            let camera = A5 + if rd8(b, A5 + var::USE_ALT_CAMERA) != 0 { 0x556 } else { 0x506 };
            (rd16(b, camera) as u32, rd16(b, camera + 4) as u32)
        };
        (c.d[2], c.d[6]) = (rank, 0xffff);
        // flags: the `addq.b #1,D2` of the fourth player if placed, otherwise the `tst` that skipped them (Z)
        let fourth = PLAYERS + 3 * PLAYER_STRIDE;
        if rd8(b, fourth + player::ACTIVE) != 0 && rd16(b, fourth + player::HEALTH) != 0 {
            crate::tools::ops::add(c, 1, rank - 1, Size::Byte);
        } else {
            c.flags_logic(0, Size::Byte);
        }
        c.a[1] = PLAYERS + 4 * PLAYER_STRIDE;
        c.a[2] = table::START_POSITIONS + (scene << 4) + 4 * rank;
        c.a[3] = table::START_STATES + (scene << 2) + rank;
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
