//! Tools: the natives of `crate::player::update` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::player::update::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::player::init::{clear_overlays_of, clear_player, UNLOCKED_FLAGS};
use crate::player::states::sub;
use crate::player::{actions, checks, combos, effects, hurt, ram, scripted, Ctx, Player};
use crate::mem::{wr32, wr8};
use crate::objects::lists;
use crate::machine::Machine;

/// 0x35292: update of the players, for the tools (the game calls `update_all`).
pub const UPDATE_PLAYERS: Native = Native {
    entry: 0x35292,
    name: "players_update",
    run: crate::Body::WithCpu(|c, b| {
        super::on_bench(c, b, update_all);
        crate::tools::frames::rts(c, b)
    }),
    touches_sound: false,
    outputs: 0,
};
