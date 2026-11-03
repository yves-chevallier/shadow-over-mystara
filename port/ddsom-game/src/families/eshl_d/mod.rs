//! Enemy shots and attacks (ESHL class), types 64 to 72 (end of table 0x39b96). See `docs/families/eshl_c.md`.
//!
//! Same framework as `eshl.rs`: one `mod` per type, `run` is called by the dispatch with the current object; `+4` is
//! the main state, `+5` / `+6` the substates (even bytes).

use crate::families::cx::Bench;
use super::cx::{handlers, Cx};
use super::shell::*;
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Current;
use crate::machine::Machine;

/// Allocates an ESHL object; `None` if the pool is full. `subq.w #1,(off,A0)`: true when the counter reaches zero.
fn count_down<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> bool {
    x.add16(off, u16::MAX);
    x.u16(off) == 0
}

// ---------------------------------------------------------------------------------------------
// Type 64: flame columns of the warrior (0xa6efa)
// ---------------------------------------------------------------------------------------------

/// Shot 64 (0xa6efa): flames placed by a creator (`+0x5c`, an enemy or another flame). Variant 0 counts its generations
/// in `+0x2f`: the fourth falls back in an arc and goes out on the ground (area damage 0x2860), the others light up in
/// place; at the end of their time (`+0x42`), they create three flames: one of variant 0 (next generation) and two
/// columns (variants 1 and 2) that rise or descend by 8 pixels per generation (`+0x46`: sound every third one).
pub mod flame64;

// ---------------------------------------------------------------------------------------------
// Type 65: rain of stones (0xa72c4)
// ---------------------------------------------------------------------------------------------

/// Shot 65 (0xa72c4): stone that falls from the top of the screen at a randomly drawn position (two draws: X within the
/// screen, depth within a band of 0x50), after a wait read from a table by rank `+0x2f`; on the ground it triggers area
/// damage (0x2860). Variant 0: impact burst (0xab7d8) and disappearance; variant 1: impact animation until its end.
pub mod rocks65;

// ---------------------------------------------------------------------------------------------
// Type 66: bursts held then dropped (0xa75c6)
// ---------------------------------------------------------------------------------------------

/// Shot 66 (0xa75c6): burst stuck to its carrier (`+0x46`, word) at an offset read from a table by variant and rank
/// `+0x2f`, which approaches it blinking for 8 frames, then is thrown upward, falls back (sound, area damage 0x2824),
/// waits 16 frames and blinks 16 frames before disappearing. Without a carrier, it restarts from initialization.
pub mod shard66;

// ---------------------------------------------------------------------------------------------
// Type 67: thrown barrel (0xa78c2)
// ---------------------------------------------------------------------------------------------

/// Shot 67 (0xa78c2): object thrown in an arc toward the owner (`+0x50`): the vertical speed is chosen to reach it (12
/// less per step of the horizontal distance, bounded to 16..256). It spins on itself (orientation flipped every 4
/// frames); on the ground it slides for 30 frames, slowing down (effect 0x1172e every 6 frames); hit or parried, it
/// bounces three times (effects 12, 13, 8). It blinks during its last 30 frames and disappears off the screen.
pub mod barrel67;

// ---------------------------------------------------------------------------------------------
// Type 68: lifted stones (0xa7b76)
// ---------------------------------------------------------------------------------------------

/// Shot 68 (0xa7b76): stone that rises from the ground (foreground) above an abscissa read by rank `+0x2f`, then stays
/// in the air as long as its carrier (`+0x46`) is not in state 0x204 beyond substate 6; once dropped, it flies off
/// (speed table 31, direction by rank) with an animation that depends on its height on the screen, and disappears when
/// passing below the top of the screen.
pub mod stone68;

// ---------------------------------------------------------------------------------------------
// Type 69: bouncing stone (0xa7e30)
// ---------------------------------------------------------------------------------------------

/// Shot 69 (0xa7e30): stone thrown in front of the creator (48 pixels, height 32) after a wait read by rank `+0x2f`; it
/// bounces at each contact with the ground (sound) and disappears when it leaves the screen (16 pixels to the left, 400
/// to the right). `+0x42` (byte, set by the creator) = 11: stronger hit (0x36).
pub mod bouncer69;

// ---------------------------------------------------------------------------------------------
// Type 70: gratings that rise according to the scroll (0xa801a)
// ---------------------------------------------------------------------------------------------

/// Shot 70 (0xa801a): dangerous scenery element linked to a SET (type 7, `+0x5c`) that signals it as long as the screen
/// has not reached a given abscissa (0x500 for variant 0, 0x650 for variant 1); past this point, the SET is notified
/// (`+0x2f` = 0) and the object disappears. Variant 1 alternates wait (`+0x42`, 4 frames) and animation.
pub mod gate70;

// ---------------------------------------------------------------------------------------------
// Type 71: rolling balls (0xa81ca)
// ---------------------------------------------------------------------------------------------

/// Shot 71 (0xa81ca): ball launched by an enemy (`+0x46`). Variant 0: held until `+0x58` releases it (disappears if the
/// launcher goes to state 4 other than 0x204), leaves in an arc (speed table 33, direction according to the
/// orientation), rolls on the ground; when hit (`+0x33`), it creates in front of it a ball of variant 1 if it is on the
/// screen. Variant 1: rolls until the end of its animation.
pub mod ball71;

// ---------------------------------------------------------------------------------------------
// Type 72: bursts thrown by a creator (0xa838e)
// ---------------------------------------------------------------------------------------------

/// Shot 72 (0xa838e): burst placed on its creator (`+0x5c`) with an offset and speeds read from a sheet by rank `+0x2f`
/// and variant; it finishes its animation, leaves in an arc, falls back (area damage 0x2860 of strength 40) and
/// disappears at the end of the impact animation. Without a creator, it frees itself.
pub mod splinter72;

handlers! {
    ESHL_64 0xa6efa "eshl_64" flame64::run,
    ESHL_65 0xa72c4 "eshl_65" rocks65::run,
    ESHL_66 0xa75c6 "eshl_66" shard66::run,
    ESHL_67 0xa78c2 "eshl_67" barrel67::run,
    ESHL_68 0xa7b76 "eshl_68" stone68::run,
    ESHL_69 0xa7e30 "eshl_69" bouncer69::run,
    ESHL_70 0xa801a "eshl_70" gate70::run,
    ESHL_71 0xa81ca "eshl_71" ball71::run,
    ESHL_72 0xa838e "eshl_72" splinter72::run,
}
