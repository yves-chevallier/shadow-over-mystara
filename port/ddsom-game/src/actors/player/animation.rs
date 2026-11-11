//! Player animations: animation sets specific to each character (long entries).
//!
//! The number of an animation (`D6` in the original) is an index into the character's set
//! (`Player::CHARACTER`). The sets are arrays of offsets in ROM, reached through a table of
//! pointers in the code (see `objects::anim` for the script format).


use super::{Ctx, Player};
use crate::world::objects::anim;

/// 0x2c16: starts animation `number` (shifted by 0x40 if the player has no weapon in hand).
pub fn start(g: &mut Ctx, p: Player, number: u16) {
    anim::start_by_character(g.b, p.obj(), &anim::SETS_ARMED, number);
}

/// 0x2c88: starts animation `number` of the second table (no shift).
pub fn start_plain(g: &mut Ctx, p: Player, number: u16) {
    anim::start_by_character(g.b, p.obj(), &anim::SETS_PLAIN, number);
}

/// 0x2d58: one animation step. Returns true when the original's carry is raised: script finished, or
/// zero duration restarting from 0xFFFF (original glitch, `anim::Step::Wrapped`).
pub fn step(g: &mut Ctx, p: Player) -> bool {
    anim::step(g.b, p.obj(), true).finished()
}
