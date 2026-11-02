//! Tools: the natives of `crate::player::actions` for the original code that calls them (arguments and results in the registers
//! of the original).


use crate::player::actions::*;
use crate::player::Player;

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

pub(crate) const TO_STAND: crate::Native = crate::Native {
    entry: 0x35614,
    name: "player_to_stand",
    run: crate::Body::WithCpu(|c, b| stand_native(c, b, true)),
    touches_sound: false,
    outputs: crate::regs::ALL,
};

pub(crate) const START_STAND: crate::Native = crate::Native {
    entry: 0x35620,
    name: "player_start_stand",
    run: crate::Body::WithCpu(|c, b| stand_native(c, b, false)),
    touches_sound: false,
    outputs: crate::regs::ALL,
};
