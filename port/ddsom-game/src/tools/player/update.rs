//! Tools: the natives of `crate::player::update` for the original code that calls them (arguments and results in the registers
//! of the original).


use crate::player::update::*;
use crate::Native;

/// 0x35292: update of the players, for the tools (the game calls `update_all`).
pub(crate) const UPDATE_PLAYERS: Native = Native {
    entry: 0x35292,
    name: "players_update",
    run: crate::Body::WithCpu(|c, b| {
        super::on_bench(c, b, update_all);
        crate::tools::frames::rts(c, b)
    }),
    touches_sound: false,
    outputs: 0,
};
