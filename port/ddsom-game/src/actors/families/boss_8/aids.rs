//! Boss 8: aids. Those shared with the other black warrior are in `boss_common::warrior`, called with the tables and
//! the play area of this boss ([`KIT`]).

use crate::actors::families::cx::Bench;
use super::*;
use crate::actors::families::boss_common::warrior::{self, Kit, Tables};

/// The tables and the play area of this boss.
const KIT: Kit = Kit {
    tables: Tables {
        sidesteps: "boss_8/sidesteps",
        dodge_spots: "boss_8/dodge_spots",
        front_attacks: "boss_8/front_attacks",
        front_attacks_slow: "boss_8/front_attacks_slow",
        back_attacks: "boss_8/back_attacks",
        back_attacks_slow: "boss_8/back_attacks_slow",
        follow_ups: "boss_8/follow_ups",
    },
    x: (0x20, 0x160),
    y: (0x148, 0x186),
    dodge_low: 0x164,
    repeats: 4,
    follow_box: (-0x30, 0x100, 0x40),
};

/// Randomly picks a living player not yet designated (bit in `PICKED`) as target `+0x50`.
pub(super) fn pick_new<R: Bench>(x: &mut Cx<'_, R>) {
    warrior::pick(x, true)
}

/// Same without remembering the choice.
pub(super) fn pick_any<R: Bench>(x: &mut Cx<'_, R>) {
    warrior::pick(x, false)
}

pub(super) use warrior::{burst_back, burst_front, burst_small, must_turn};

/// Sidestep drawn from the table of sidesteps.
pub(super) fn random_sidestep<R: Bench>(x: &mut Cx<'_, R>) {
    warrior::sidestep(x, &KIT)
}

/// Brings the boss back into its play area.
pub(super) fn clamp<R: Bench>(x: &mut Cx<'_, R>) {
    warrior::clamp(x, &KIT)
}

/// If a player is very close, the boss jumps elsewhere.
pub(super) fn dodge_nearby<R: Bench>(x: &mut Cx<'_, R>) {
    warrior::dodge_nearby(x, &KIT)
}

/// A landing point drawn according to the quarter of the screen where the boss is.
pub(super) fn dodge_spot<R: Bench>(x: &mut Cx<'_, R>) {
    warrior::dodge_spot(x, &KIT)
}

/// Draws the next attack (`front`: the target is in front).
pub(super) fn choose_attack<R: Bench>(x: &mut Cx<'_, R>, front: bool) {
    warrior::choose_attack(x, &KIT, front)
}

/// End of an attack: follow-up drawn if a player is in the box, otherwise back to combat.
pub(super) fn after_attack<R: Bench>(x: &mut Cx<'_, R>) {
    warrior::after_attack(x, &KIT)
}

/// 0x8a432 (`far`) and 0x8a466: is an active player (activity byte at 1) in the box?
pub(super) fn player_near<R: Bench>(x: &mut Cx<'_, R>, far: bool) -> bool {
    if far { warrior::scan(x, true, -0x130, 0x40, 0x20) } else { warrior::scan(x, true, -0x20, 0x20, 8) }
}

/// 0x8a368: reappearance at one of the 32 points of the table of spots.
pub(super) fn random_spot<R: Bench>(x: &mut Cx<'_, R>) {
    let rng = x.random();
    let i = rng & 0x7c;
    let px = word(x, "boss_8/spots", i as u32);
    let py = word(x, "boss_8/spots", i as u32 + 2);
    x.set16(Obj::X, px);
    x.set16(Obj::Y, py);
}
