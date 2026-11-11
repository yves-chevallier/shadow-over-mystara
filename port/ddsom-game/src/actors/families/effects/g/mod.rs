//! EFFCT family (continued): types that no scenario exercises, rewritten so that the `0x39ce0` type table
//! no longer has an entry in original code (validation by `examples/fuzz_effct.rs`).
//!
//! - 43: rain of debris (`0xb4bcc`): at the top of the screen (setting 0, with a dust cloud at the start) or
//!   from the creator (`+0x5c`), speeds and animation drawn at random; it falls, blinks and
//!   disappears, or disappears with the creator;
//! - 46: armor pieces of an enemy (`0xb53f4`): they follow the carrier and replay its received hits,
//!   fall when its life drops below their threshold and break into shards (variant 1) that bounce;
//! - 48: flying machine of an enemy (`0xb59a6`, `0xb59c8`): cannon that turns around its carrier and
//!   fires (1, with its body, variant 0), sweeper placed at the screen edge that fires (2);
//! - 80: fireworks of a character's scene (`0x23606`): the hero (0) fires rockets during a
//!   fade to black, then flies away and ends the scene; spectators and sidekicks (1 to 6) wait for step
//!   0x12 of the scene to react, and leave with the fade;
//! - 63: chunk thrown by an enemy (ENEMY 22 and 23): leaves in an arc from its creator (`+0x42`),
//!   falls back, scatters debris, waits, blinks then disappears;
//! - 81: spark thrown by a character (`0xb96c0`): leaves the player's hand (offset per gesture
//!   `+0x42`), glides in an arc then blinks for the last frames.

use crate::actors::families::cx::Bench;
use crate::actors::families::cx::{handlers, Cx, Gx};
use crate::world::objects::Obj;
use crate::world::objects::pools;
use crate::world::objects::Current;
use crate::world::machine::Machine;

mod thrown;
use thrown::*;
mod enemy;
use enemy::*;
mod rockets;
use rockets::*;
use crate::actors::families::effects::tables::{held16, tables, Read, Table};
use crate::actors::families::effects::records::*;
use crate::actors::families::effects::vars;
pub use rockets::Step;

/// Shared fields.
mod f {
    pub const STATE: u32 = 0x04;
    pub const SUB_STATE: u32 = 0x05;
    pub const VARIANT: u32 = 0x03;
    pub const FLAGS: u32 = 0x01;
    pub const FACING: u32 = 0x2c;
}

/// Routines d'origine.
mod rom {
    /// Sound of the impact on the ground.
    pub const SOUND_LANDING: crate::hw::sound::Sound = crate::hw::sound::CMD_363_363;
}

fn free<R: Bench>(x: &mut Cx<'_, R>) {
    x.free(&pools::EFFCT);
}

/// Bit 1 of `FLAGS`: object hidden (blinking).
fn set_hidden<R: Bench>(x: &mut Cx<'_, R>, on: bool) {
    x.set_bit(f::FLAGS, 1, on);
}

handlers! {
    EFFCT_FIREWORK 0xb8ef6 "effct_firework" fireworks,
    EFFCT_MACHINE_VOLANTE 0xb59ea "effct_machine_volante" flyer,
    EFFCT_ARMURE 0xb5454 "effct_armure" armour,
    EFFCT_RAIN_DEBRIS 0xb4bec "effct_rain_debris" debris_rain,
    EFFCT_ENEMY_PIECE 0xb7866 "effct_enemy_piece" chunk,
    EFFCT_SHARD_THROWN 0xb96dc "effct_shard_thrown" shard,
}
