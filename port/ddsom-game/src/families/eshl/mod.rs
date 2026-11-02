//! Enemy shots and attacks (ESHL class, pool of 38 objects of 0x60 bytes, table 0x39b96).
//!
//! An ESHL object is created by an enemy, a boss or an effect: `+0x50` designates its owner (RAM address on 16 bits),
//! `+0x03` is the variant chosen by the creator, `+0x2c` the orientation inherited from the owner. The collision boxes
//! (set `+0x2e`, power `+0x44/+0x45`) are read by `combat`. Each type is a state machine: `+4` main state, `+5` / `+6`
//! substates (even bytes, indices of a jump table of the original).

use crate::families::cx::Bench;
use super::cx::{handlers, Cx};
use super::shell::*;
use crate::objects::Obj;

use crate::objects::pools;
use crate::objects::Current;

// ---------------------------------------------------------------------------------------------
// Type 0 (0x9dbfc): horizontal projectile, two variants (+3)
// ---------------------------------------------------------------------------------------------

/// Projectile that leaves from the owner's side at 5 pixels per frame. It stops when it hits something (flags `+0x57`)
/// or certain terrains, then falls back blinking before disappearing.
mod shot0;

/// Shot 1 (0x9de10): ball thrown in a lob. Two modes according to `+0x2f`: 0, the owner throws it in front of itself;
/// otherwise it springs out with a speed drawn from an 8-row table (variant `+3`). It falls back, bounces on the ground
/// (Z < 0: Z reset to 0), waits 80 frames in an animation then blinks for 80 frames before disappearing.
mod shot1;

/// Shot 2 (0x9e0aa): horizontal projectile without its own animation (the sprite is that of the owner's variant).
/// Destroyed on contact (sound), or on a blocking terrain.
mod shot2;

/// Shot 3 (0x9e1b6): projectile attached to an owner (the creator is alive as long as its state `+4` is below 4). Four
/// variants launched in a fan: offset (dx, dz) and speeds (vx, vy) read from an 8-byte table per variant. Disappears
/// with the end of its animation or when the owner is no longer in normal play.
mod shot3;

/// Shot 4 (0x9e2dc): bomb thrown by an enemy (no owner: position and direction set by the creator). It falls back
/// according to a speed table, bounces once on the ground with the speeds set aside in `+0x42` / `+0x50`, then explodes
/// at the end of the animation.
mod shot4;

/// Shot 6 (0x9ebca): contact effect stuck in front of the owner (2 variants of power 6 and 7). It stays as long as its
/// animation lasts and the owner is in normal play.
mod shot6;

/// Shot 7 (0x9ecc0): spark placed on the owner (offset read from a 4-byte table per variant); disappears at the end of
/// the animation.
mod shot7;

/// Shot 8 (0x9ee00): horizontal thrust in front of the owner. Advances (X, Y, Z) until the animation sets `+0x33`, then
/// finishes the animation.
mod shot8;

/// Shot 9 (0x9ef46): attack with three forms according to the variant `+3` (ball that falls back, breath stuck to an
/// enemy, spark placed on an enemy).
mod shot9;

/// Shot 10 (0x9f376): edge projectile (power 0x10), placed according to a table row chosen by the register D5 of the
/// dispatch (peculiarity of the original). Waits for a state of the owner, then leaves 240 frames before disappearing.
mod shot10;

/// Shot 11 (0x9f426): halo stuck to an enemy (displacement read from a 4-byte table per variant). Its animation
/// advances according to bits 0 and 1 of `+0xb0` of the owner; it can also put the owner in state 4 (stunned) after
/// having been frozen (`+0x53`).
mod shot11;

/// Shot 12 (0x9f5a8): mark placed on the attachment point `+0x5a` (offset in X and Z read from a table, according to
/// the variant); the animation comes from a byte table in ROM.
mod shot12;

/// Shot 13 (0x9f67c): effect stuck to the owner (position, orientation and offset per variant read from an 8-byte
/// table), whose animation (0xc or 0xe) follows byte `+0xa1` of the owner. When paused, it only follows the position.
mod shot13;

/// Shot 14 (0x9f786): attack part of a composite enemy (variant `+3` from 0 to 7, 9 types of forms) attached to its
/// owner (`+0x50`). It reads the owner's attack flags (`+0xa8`: attack launched, `+0xa9`: attack in preparation, one
/// bit per variant) and pushes it into the corresponding state, plays the phase's animation and creates shots of type
/// 15.
mod shot14;

/// Shot 5 (0x9e458): contact zone of a large enemy (link `+0x46` to that enemy). It copies the enemy's visibility
/// flags, chooses its sprite frame at each frame (two tables, one for even frames, the other, with flipping, for odd
/// frames when `+0xb2` of the enemy is zero) and its collision boxes (body, push, attack) according to the enemy's
/// attack state `+0xa0/+0xa1`. It disappears with it.
mod shot5;

handlers! {
    0x9dbfc "eshl_00" shot0::run,
    0x9de10 "eshl_01" shot1::run,
    0x9e0aa "eshl_02" shot2::run,
    0x9e1b6 "eshl_03" shot3::run,
    0x9e2dc "eshl_04" shot4::run,
    0x9ebca "eshl_06" shot6::run,
    0x9ecc0 "eshl_07" shot7::run,
    0x9ee00 "eshl_08" shot8::run,
    0x9ef46 "eshl_09" shot9::run,
    0x9f376 "eshl_10" shot10::run,
    0x9f426 "eshl_11" shot11::run,
    0x9f5a8 "eshl_12" shot12::run,
    0x9f67c "eshl_13" shot13::run,
    0x9f786 "eshl_14" shot14::run,
    0x9e458 "eshl_05" shot5::run,
}
