//! Tools: the natives of `crate::player::actions` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_X, SR_Z};

use crate::player::actions::*;
use crate::tools::frames::*;
use crate::{regs, Body, Native};
use crate::player::characters::{self, Slot};
use crate::player::moves;
use crate::player::checks::{attack_button, jump_button, turn_check};
use crate::player::states::{action, state};
use crate::player::walk::{clamp_to_screen, move_xy, walk_speed};
use crate::player::{animation, ram, Ctx, Player};
use crate::mem::wr8;

/// Address entries of `to_stand` (0x35614, called by `aids.rs`) and `start_stand` (0x35620). The
/// registers left by the animation (D5, D6, A4) are read by nobody (`DDSOM_POISON=anim`).
fn stand_native(c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus, reset: bool) {
    let p = Player(c.a[0]);
    super::on_bench(c, b, |g| {
        if reset {
            to_stand(g, p);
        } else {
            start_stand(g, p);
        }
    });
    crate::tools::frames::rts(c, b);
}

pub const TO_STAND: crate::Native = crate::Native {
    entry: 0x35614,
    name: "player_to_stand",
    run: crate::Body::WithCpu(|c, b| stand_native(c, b, true)),
    touches_sound: false,
    outputs: crate::regs::ALL,
};

pub const START_STAND: crate::Native = crate::Native {
    entry: 0x35620,
    name: "player_start_stand",
    run: crate::Body::WithCpu(|c, b| stand_native(c, b, false)),
    touches_sound: false,
    outputs: crate::regs::ALL,
};
