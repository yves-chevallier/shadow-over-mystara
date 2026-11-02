//! Tools: the natives of `crate::player::overlays` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::player::overlays::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::player::checks::team_score_reached;
use crate::player::init::overlay;
use crate::player::{Ctx, Player, PLAYER_COUNT};
use crate::mem::{rd16, rd32, rd8, set_low_byte, set_low_word, wr16, wr32, wr8, A5};
use crate::objects::anim::load_entry;
use crate::objects::Obj;
use crate::player::ram;
use crate::machine::Machine;

fn update_native(c: &mut Cpu, b: &mut cps2::Cps2Bus) {
    super::on_bench(c, b, update);
    rts(c, b);
}

pub const UPDATE: Native = Native {
    entry: 0x392e4,
    name: "player_overlays_update",
    run: crate::Body::WithCpu(update_native),
    touches_sound: false,
    outputs: 0,
};
