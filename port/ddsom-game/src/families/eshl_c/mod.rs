//! Enemy shots and attacks (ESHL class), types 40 to 63. See `docs/families/eshl_c.md`.
//!
//! Same framework as `eshl.rs`: one `mod` per type, `run` is called by the dispatch with the current object; `+4` is
//! the main state, `+5` / `+6` / `+7` the substates (even bytes).

use crate::families::shell::vars;
use crate::families::cx::Bench;
use super::cx::{handlers, Cx};
use super::shell::*;
use crate::mem::wr32;
use crate::objects::Obj;

use crate::objects::pools;
use crate::objects::Current;
use crate::machine::Machine;

/// Word (16-bit RAM address) seen as an object: sign extension, like `movea.w`.
fn obj_of(word: u16) -> Obj {
    Obj(word as i16 as i32 as u32 & 0xff_ffff)
}

/// Starts an animation through the AI routine (`0x103d6`).
fn ai_anim<R: Bench>(x: &mut Cx<'_, R>, set: u32, n: u32) {
    crate::families::enemy_lib::start(x, set, n);
}

/// One animation step through the AI routine; true at the end of the script.
fn ai_step<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    crate::families::enemy_lib::step(x)
}

/// The three 16.16 coordinates (X, Y, Z) of `from`, copied as longs.
fn copy_xyz32<R: Bench>(x: &mut Cx<'_, R>, from: Obj) {
    for off in [0x8, 0x10, 0xc] {
        let v = from.u32(x.b, off);
        x.set32(off, v);
    }
}

/// Same thing in words (integer parts only).
fn copy_xyz16<R: Bench>(x: &mut Cx<'_, R>, from: Obj) {
    for off in [0x8, 0x10, 0xc] {
        let v = from.u16(x.b, off);
        x.set16(off, v);
    }
}

/// Orientation copied from another object.
fn copy_flip<R: Bench>(x: &mut Cx<'_, R>, from: Obj) -> u8 {
    let flip = from.u8(x.b, Base::FLIP);
    x.set8(Base::FLIP, flip);
    flip
}

/// Opposite of a word.
fn neg(v: u16) -> u16 {
    v.wrapping_neg()
}

/// Byte seen as signed.
fn s8(v: u8) -> i8 {
    v as i8
}

/// Word seen as signed.
fn s16(v: u16) -> i16 {
    v as i16
}

// ---------------------------------------------------------------------------------------------
// Types 40 and 41: volley of projectiles launched by a creator (the creator is in `+0x5c`)
// ---------------------------------------------------------------------------------------------

/// Shot 40 (0xa31e4): projectile that appears at the position of its creator (`+0x5c`; variants 0 to 2: X offset of 0 /
/// -10 / +10 pixels), chooses an animation at random from a 16-byte table and lasts 90 frames (`+0x42`), hidden every
/// other frame according to the parity of its variant. It disappears when the owner (`+0x50`) is in state 0x406.
pub mod shot40;

/// Shot 41 (0xa336c): ray launched toward the target of the owner of its creator (type 40, in `+0x5c`). Direction and Z
/// speed read according to the X distance to the target, reloads a 32-byte palette, then, on reaching the ground,
/// creates three shots of type 40 (creation function 0xa31c2) and finishes its animation.
pub mod shot41;

// ---------------------------------------------------------------------------------------------
// Types 42 and 43: burst attack launched from a host (`+0x42`)
// ---------------------------------------------------------------------------------------------

/// Shot 42 (0xa3568): attack stuck to its host (enemy in `+0x42`), 32 pixels in front of it and 64 in height. It
/// follows the host's animation (`+0x33`), then, when the enemy triggers it (`+0x59`), launches a salvo of 5 bursts
/// (type 43) every 8 frames whose angle (`+0x5e`) rotates by `+0x5f` (modulo 32), waits 20 frames then finishes its
/// animation. Disappears when the host goes to state 4 or when it has fallen back (Z <= 0, last substate).
pub mod shot42;

/// Shot 43 (0xa3762): burst of attack 42, which leaves in the direction `+0x2d` (speed table 0x40) in animation 0x10 +
/// a value of table 0xa37ba indexed by the direction / 2. Destroyed on contact (hit or parried) or on leaving the
/// screen window.
pub mod shot43;

// ---------------------------------------------------------------------------------------------
// Types 44 and 45: beam between two objects and debris
// ---------------------------------------------------------------------------------------------

/// Shot 44 (0xa380a): beam stretched between two linked objects (variants 0 and 1). `+0x46` is the creator (which
/// counts its beams in its byte `+0x42`) and the creator of the creator is the other end. The beam moves from end A2
/// toward end A1 in 16 frames (position = A2 + t * (A1 - A2) / 16, `+0x14` = t); when hit (bit 3 of `+0x57`) it plays
/// its end animation. Variant 1 starts placed on A2 (50 pixels higher), the start animation depends on `+0x2f` of A1,
/// and the end animation keeps the beam alive until its end.
pub mod beam44;

/// Shot 45 (0xa3b88): debris thrown by an object (`+0x46`), two variants. Variant 0: springs out of the object with a
/// speed drawn at random from an 8-row table, bounces once then disappears at the end of the animation. Variant 1:
/// burst placed on the object (offset according to `+0x2f`), animated as long as the object is in state `+0xa4 = 4`,
/// then falls and disappears on the ground. The debris are kept on the screen (0xfbb0).
pub mod debris45;

// ---------------------------------------------------------------------------------------------
// Types 46 to 50
// ---------------------------------------------------------------------------------------------

/// Shot 46 (0xa3e72): mobile scenery object (animation chosen by the variant) that scrolls by 4 pixels per frame to the
/// left and disappears when it passes the left edge of the screen (`A5+0x506` - 0x50).
pub mod scroller46;

/// Shot 47 (0xa3ebe): invisible appearance, randomly offset around the edge of the camera (`A5+0x5a6` in X, `A5+0x5aa`
/// in Y), which waits for a delay drawn from a table (`+0x58`), becomes visible then plays its animation to the end.
pub mod spawn47;

/// Shot 48 (0xa4088): projectile launched in a lob by a launcher (`+0x5c`, whose fallen shots it counts in its byte
/// `+0x1d`). Waits 16 frames, rises (VZ = -0x100) until Z <= 0x20, waits 16 frames, then descends back to the starting
/// height (`+0x4e`); it can then destroy the object spotted in `+0x58`. It then frees its slot in the launcher (word
/// `+8+variant`).
pub mod lob48;

/// Shot 49 (0xa418e): blow dealt 0x6e pixels in front of the enemy that created it (`+0x42`), at height 0x35. It plays
/// the start animation as long as the host has not gone to substate 6 (`+7`); it finishes its animation afterwards, or
/// disappears if the host is stunned (state 4) without the animation having reached its mark (`+0x33`).
pub mod strike49;

/// Shot 50 (0xa4266): strikes of an enemy (`+0x42`) in two variants (0: ball that leaves in a lob 0x70 pixels in front
/// of the host; 1: slow ball at 0x76 pixels). The animation advances by phases (`+0x5a`, phase duration `+0x5b` read
/// from table 0xa44aa) with the animation value `phase + 3`. When the host is stunned (state 4) or its trajectory ends,
/// goes to the animation `phase + 6` (substate 4) then disappears. Kept on the screen and tested by 0x33fe4 at each
/// frame.
pub mod bullet50;

// ---------------------------------------------------------------------------------------------
// Types 51 and 52
// ---------------------------------------------------------------------------------------------

/// Shot 51 (0xa4522): projectile falling from the top of the screen, launched by a launcher (`+0x5c`, whose fallen
/// shots it counts in its byte `+0x1d`). Waits `+0x42` frames, appears 0x100 higher than its depth, waits 30 frames
/// then falls (VZ = -0x100, AZ = -0x40) to the ground, where it triggers two impact subroutines (0xab7d8 with 3, 0x2860
/// with 8) and disappears.
pub mod drop51;

/// Shot 52 (0xa46c6): elements of a boss's final attack, attached to it (`+0x46`), four variants that exist only as
/// long as the boss is in the expected state `+0xa0`:
///  0 (state 0x201): ball placed on the boss, which launches three sub-elements (variants 1 to 3 of
///    `+0x2f`); hurts the players it touches;
///  1 (state 0x214): ray stuck beside the boss;
///  2 (state 0x215): full-screen veil that follows the screen and hurts the players outside the safe zones
///    of stage 0x2e or 0x69;
///  3: variant of the previous one that lasts 256 frames (`+0x42`).
/// The screen locking states are `A5+0x43` (0x3f taken, 0xff released) and `A5+0x4ba`.
pub mod finale52;

// ---------------------------------------------------------------------------------------------
// Types 53 to 55
// ---------------------------------------------------------------------------------------------

/// Hidden every other frame according to the parity of the variant compared with the global phase `A5+0x6e`.
fn blink_by_variant<R: Bench>(x: &mut Cx<'_, R>) {
    let hidden = (x.u8(Obj::VARIANT) & 1) as u16 != vars::FRAME_LOW.get(x.b) & 1;
    x.set_bit(Obj::FLAGS, 1, hidden);
}

/// Top of the screen (`A5+0x50a`, or `A5+0x55a` in two-player mode `A5+0x36`).
fn screen_top<R: Bench>(x: &mut Cx<'_, R>) -> u16 {
    camera(x.b).1.get(x.b)
}

/// Shot 53 (0xa4d82): projectile aiming at the player targeted by the creator of its creator. It leaves from the
/// position of its creator (`+0x5c`, itself linked to an enemy whose `+0x50` designates the target), in the direction
/// (0 to 8 approximately) given by routine 0x1376; animation and orientation read from a 2-byte table per direction.
/// Destroyed on contact (hit or parried); removed off the screen.
pub mod aimed53;

/// Shot 54 (0xa4eaa): salvo of projectiles launched by an object (`+0x5c`, which has a creator), 17 copies numbered by
/// the variant `+3` (created by 0xa4e84). Sub-type `+0x2f` 0: leaves the object with the offsets and speeds of row `+3`
/// of table 0xa4fe6 (18 bytes); other sub-type: 20-byte row of table 0xa51b4 (including the animation). Moves for 60
/// (or 30) frames, then 10 more frames, then its animation ends; blinks according to its variant. The shot disappears
/// as soon as its launcher no longer exists.
pub mod burst54;

/// Shot 55 (0xa5358): projectile that falls from the top of the screen, thrown to one side (variant 0: to the left; X
/// and depth come from `+0x58` / `+0x5a`, set by the creator). It goes diagonally (VX = ±0x800, VZ = -0xc00) to the
/// ground (Z <= 0), then stays 48 frames playing its second animation, blinking during the last 32. Motionless when
/// paused (`A5+0x3d`).
pub mod fall55;

// ---------------------------------------------------------------------------------------------
// Types 56 to 58
// ---------------------------------------------------------------------------------------------

/// The four players (records of 0x200 bytes starting at `A5+0x5de`).
fn players() -> impl Iterator<Item = Obj> {
    (0..4).map(|i| Obj(0xff_8000 + 0x5de + 0x200 * i))
}

/// Shot 56 (0xa5466): objects to pick up (12 variants, two per object type: the even variant uses palette 0x14 and
/// attack identifier 0x21, the odd one 0x13 and 0x22). Placed on the screen, they animate (blink according to `A5+0x6f`
/// and `D7`, except during `A5+0x33`); a player on the ground within 8 pixels in X and 6 in depth picks them up: the
/// object blinks for 30 frames (`+0x56`) then plays its pickup animation (types 0 to 5: animation 3 and sound 0xeb132
/// for variants 0 and 1), or, for variants 6 and 7, waits a delay drawn from a table before replaying animation 5;
/// variants 8 and 9 depend on the hit points (`+0x62`) of their creator (`+0x5c`). Disappears when it is more than
/// 0x200 pixels to the left of the screen (except stage 0x19).
pub mod pickup56;

/// Shot 57 (0xa593c): homing projectile of a creator (`+0x5c`): placed 0x4a pixels in front of it and 0x32 higher, it
/// aims at the creator's target (`+0x50` of the creator), heads toward it through the speed table (direction recomputed
/// every 20 frames) and explodes into 16 bursts (type 58) when it is within 24 pixels of the target or when its 360
/// frames (`+0x46`) have elapsed. If its creator dies or loses its target, it blinks and disappears.
pub mod seeker57;

/// Shot 58 (0xa5ad6): burst of explosion 57 (16 variants from the 8-byte-per-variant table: duration `+0x42`, sub-type
/// `+0x2f`, speed index `+0x2d`, animation, orientation, speed table). Leaves from the position of its creator and
/// moves through the speed table (XY or XZ according to `+0x2f`) for its duration, blinking according to its variant.
pub mod shard58;

// ---------------------------------------------------------------------------------------------
// Types 59 to 62
// ---------------------------------------------------------------------------------------------

/// Shot 59 (0xa5cc2): effect ring of a creator (`+0x5c`), two sub-types `+0x2f`. Sub-type 0: placed on the creator,
/// with the creator's owner (`+0x50`), plays animation 1, then 2 (300 frames), 3, and when it ends triggers effect
/// 0xe448c; disappears when the owner is in state 0x406. Sub-type 1: placed near the creator (offset per variant), it
/// plays its animation and sets the creator's byte `+0xae` to 1 at the end. The palette changes every 3 frames.
pub mod ring59;

/// Shot 60 (0xa5f5c): mobile platform that rises and descends with the screen (height corrected by the vertical scroll
/// `A5+0x50a`/`0x55a`), 4 variants. Cycle of `+5`: wait (variant 0: `+0x42` frames, variant 1: a player climbs on it
/// (0x7f19c), variant 3: same with `A5+0x51e` = 4, variant 2: the creator is in state 0x204), rise (VZ = -0x100, AZ =
/// -0x20, actually descend: Z decreases) until Z <= 0x20 (0x38 if sub-type `+0x2f`), wait 16 frames then go back up (VZ
/// = 0x180) to the starting height (`+0x4e`), and start over. The drawing layer `+0x3b` depends on the height. Creates
/// a player shot of type 0x18 (0x54872).
pub mod platform60;

/// Shot 61 (0xa63d8): marker stuck to its creator (`+0x5c`, of variant 0 to 2 which chooses one of the three offset
/// tables; 20 bytes per marker variant, 4 per sub-type `+0x2f`), which plays its animation and, at the end, sets the
/// creator's byte `+0x1c` to 1.
pub mod marker61;

/// Shot 62 (0xa66ea): meteor that falls on the terrain near the camera. Position drawn at random around the screen (X
/// within 0x1ff + 0x40, Y within 0x7f) at height 0x100, delayed by `+0x42` frames (table according to the variant),
/// falls (VZ = -0x200, AZ = -0x20) and, on touching the ground (Z < 0), triggers the impact effects (0xb9c88, 0xab7d8
/// with 3, 0x2860 with 8, 0xeafdc).
pub mod meteor62;

// ---------------------------------------------------------------------------------------------
// Type 63
// ---------------------------------------------------------------------------------------------

/// Shot 63 (0xa6864): hand of a boss (`+0x5c`, whose byte `+0xac` gives the phase: < 1 rest, 1 attack, > 1 end) that
/// follows the boss (2 pixels per frame toward the target position: 0x30 in front of it, same depth) and plays
/// animations 3 (rest) and 5 (attack). At rest, `+0x42` counts the frames before drawing lots (table 0xa6c54 according
/// to the number of players `A5+0x35` and the state `A5+0xff92`) whether it attacks when the target is on its side; an
/// attack launched by the boss (phase 1) creates a shot of type 53 at the end of its animation. Substates: `+5` (0
/// rest, 4 attack) and `+6` (steps of 4).
pub mod hand63;

handlers! {
    0xa31e4 "eshl_40" shot40::run,
    0xa336c "eshl_41" shot41::run,
    0xa3568 "eshl_42" shot42::run,
    0xa3762 "eshl_43" shot43::run,
    0xa380a "eshl_44" beam44::run,
    0xa3b88 "eshl_45" debris45::run,
    0xa3e72 "eshl_46" scroller46::run,
    0xa3ebe "eshl_47" spawn47::run,
    0xa4088 "eshl_48" lob48::run,
    0xa418e "eshl_49" strike49::run,
    0xa4266 "eshl_50" bullet50::run,
    0xa4522 "eshl_51" drop51::run,
    0xa46c6 "eshl_52" finale52::run,
    0xa4d82 "eshl_53" aimed53::run,
    0xa4eaa "eshl_54" burst54::run,
    0xa5358 "eshl_55" fall55::run,
    0xa5466 "eshl_56" pickup56::run,
    0xa593c "eshl_57" seeker57::run,
    0xa5ad6 "eshl_58" shard58::run,
    0xa5cc2 "eshl_59" ring59::run,
    0xa5f5c "eshl_60" platform60::run,
    0xa63d8 "eshl_61" marker61::run,
    0xa66ea "eshl_62" meteor62::run,
    0xa6864 "eshl_63" hand63::run,
}
