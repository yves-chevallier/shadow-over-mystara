//! Character hook points (P5).
//!
//! The common core (`actions`) does not contain the attack code: six characters × two variants
//! each have their own jump, attack, special attack and two other actions. In the original program,
//! each `Slot` is a table of 12 addresses (one per value of `Player::CHARACTER`);
//! `actions::act` calls the address in the table.
//!
//! `run` dispatches to the character's Rust function (`routine`); the natives at the original addresses
//! (`tools::registry`) only serve the differential fuzz against the original routine.
//!
//! The common functions that the characters reuse are public: `checks::turn_check`,
//! `attack_button`, `jump_button`, `actions::to_stand`, `walk::walk_speed`, `walk::move_xy`,
//! `animation::start`, `animation::step`, `hurt::hurt_animation`...

pub mod cleric;
pub mod common;
pub mod dwarf;
pub mod elf;
pub mod fighter;
pub mod foes;
pub mod items;
pub mod magic_user;
pub mod thief;

use super::{Ctx, Player};

/// A character's Rust routine: runs for one frame of the player.
pub type CharacterFn = fn(&mut Ctx, Player);

/// The five character-specific actions (byte `+6`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    /// Jump (`action::JUMP`, 0x0a).
    Jump,
    /// Button 1 attack (`action::ATTACK`, 0x0c).
    Attack,
    /// Special attack (`action::SPECIAL`, 0x0e).
    Special,
    /// Action 0x10 (h: aerial attack or weapon throw).
    Action10,
    /// Action 0x12 (h).
    Action12,
}

/// Routine of character `character` (`Player::CHARACTER`) for action `slot`: original tables 0x35918 (jump),
/// 0x35972 (attack), 0x359cc (special), 0x35a0a (action 0x10) and 0x35a48 (action 0x12), one address per value
/// of `CHARACTER`; the two variants of a character have the same code.
pub fn routine(slot: Slot, character: u8) -> CharacterFn {
    use super::moves;
    use Slot::*;
    match (character >> 1, slot) {
        (0, Jump) => fighter::jump,
        (0, Attack) => fighter::attack,
        (0, Special) => fighter::special,
        (1, Jump) => cleric::jump,
        (1, Attack) => cleric::attack,
        (1, Special) => cleric::special,
        (2, Jump) => elf::jump,
        (2, Attack) => elf::attack,
        (2, Special) => elf::special,
        (3, Jump) => dwarf::jump,
        (3, Attack) => dwarf::attack,
        (3, Special) => dwarf::special,
        (3, Action12) => dwarf::roll,
        (4, Jump) => magic_user::jump,
        (4, Attack) => magic_user::attack,
        (4, Special) => magic_user::special,
        (4, Action10) => magic_user::dash,
        (4, Action12) => moves::hop_back,
        (5, Jump) => thief::jump,
        (5, Attack) => thief::attack,
        (5, Special) => thief::special,
        (5, Action10) => thief::dash,
        (5, Action12) => thief::roll,
        (0..=3, Action10) => common::dash,
        (0..=2, Action12) => moves::leap_attack,
        (class, _) => unreachable!("character {character} (class {class}) outside the tables 0x35918-0x35a48"),
    }
}

/// Runs action `slot` of player `p`'s character.
pub fn run(g: &mut Ctx, p: Player, slot: Slot) {
    let character = p.u8(g, Player::CHARACTER);
    routine(slot, character)(g, p)
}
