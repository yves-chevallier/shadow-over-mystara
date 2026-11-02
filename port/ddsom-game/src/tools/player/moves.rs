//! Tools: the natives of `crate::player::moves` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::player::moves::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::player::actions::to_stand;
use crate::player::checks::{attack_button, item_curse, jump_button};
use crate::player::spawn::Thrown;
use crate::player::states::State;
use crate::player::walk::{clamp_x, crawl_speed, move_x, move_xy, move_xyz, move_xz, move_z, run_speed, walk_speed};
use crate::player::{animation, ram, Ctx, Player};
use crate::mem::{rd16, rd32, rd8, wr16, wr32, wr8};
use crate::objects::{pools, Obj};
use crate::machine::Machine;
use crate::player::external as ext;

/// Natives of the characters' actions 0x12 (table `characters::Slot::Action12`): the Rust function receives
/// the player (A0); the callers read no register.
pub const LEAP: crate::Native = crate::Native {
    entry: 0x35a78,
    name: "leap_attack",
    run: crate::Body::WithCpu(|c, b| {
        let p = Player(c.a[0]);
        super::on_bench(c, b, |g| leap_attack(g, p));
        crate::tools::frames::rts(c, b)
    }),
    touches_sound: false,
    outputs: 0,
};

pub const HOP_BACK: crate::Native = crate::Native {
    entry: 0x35b96,
    name: "magic_user_hop_back",
    run: crate::Body::WithCpu(|c, b| {
        let p = Player(c.a[0]);
        super::on_bench(c, b, |g| hop_back(g, p));
        crate::tools::frames::rts(c, b)
    }),
    touches_sound: false,
    outputs: 0,
};
