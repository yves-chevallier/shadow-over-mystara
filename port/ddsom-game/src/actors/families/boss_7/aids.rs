//! Boss 7: aids. Those shared with the other black warrior are in `boss_common::warrior`, called with the tables and
//! the play area of this boss ([`KIT`]).

use crate::actors::families::cx::Bench;
use super::*;
use crate::actors::families::boss_common::warrior::{self, Kit, Tables};

/// The tables and the play area of this boss.
const KIT: Kit = Kit {
    tables: Tables {
        sidesteps: "boss_7/sidesteps",
        dodge_spots: "boss_7/dodge_spots",
        front_attacks: "boss_7/front_attacks",
        front_attacks_slow: "boss_7/front_attacks_slow",
        back_attacks: "boss_7/back_attacks",
        back_attacks_slow: "boss_7/back_attacks_slow",
        follow_ups: "boss_7/follow_ups",
    },
    x: (0x45f, 0x68f),
    y: (0x168, 0x19f),
    dodge_low: 0x64,
    repeats: 3,
    follow_box: (-0x38, 0x38, 0x18),
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

/// 0x87c9a (`far`) and 0x87ccc: is a living player in the box?
pub(super) fn player_near<R: Bench>(x: &mut Cx<'_, R>, far: bool) -> bool {
    if far { warrior::scan(x, false, -0x130, 0x40, 0x20) } else { warrior::scan(x, false, -0x20, 0x20, 8) }
}
