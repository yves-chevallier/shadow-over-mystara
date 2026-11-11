//! Tools: the natives of `crate::actors::player::moves` for the original code that calls them (arguments and results in the registers
//! of the original).


use crate::actors::player::moves::*;
use crate::world::objects::player::Player;

/// Natives of the characters' actions 0x12 (table `characters::Slot::Action12`): the Rust function receives
/// the player (A0); the callers read no register.
pub(crate) const LEAP: crate::Native = crate::Native {
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

pub(crate) const HOP_BACK: crate::Native = crate::Native {
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
