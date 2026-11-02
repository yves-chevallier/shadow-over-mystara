//! EFFCT family (continued): scene types still exercised by the scenarios, rewritten on [`Cx`]
//! (type table `0x39ce0`). See `docs/familles.md` §4.
//!
//! - 3: screen element fixed to the camera;
//! - 10: diagonal slabs that sink under the players;
//! - 12: link of a chain that replays the previous one's directions with a delay;
//! - 16: double of a carrier, which glides into place then follows the carrier's steps;
//! - 20: shop gold-donation cursor (draws the life gauge and the player's gold);
//! - 22: shop counter (sales zone, which announces the item to the player who enters);
//! - 23: players' vote for the next path (ballot, cursors, countdown);
//! - 30: fish of the introduction scene (school that crosses, fish that swims and jumps);
//! - 34: platform that advances carrying the players away;
//! - 36: floating object of an introduction scene (undulates, is hit, fades; its reflection);
//! - 37, 38: panels of an end-of-trial screen (title that descends, message that falls and blinks);
//! - 39: chunk thrown toward a point on the screen, which bursts on touching the ground;
//! - 40: attack effects (sparks that follow the attacker, placed in front of it or falling from the top of the
//!   screen, debris);
//! - 41: object thrown at random (four kinds) that falls back then glides or bounces;
//! - 42: scene effects, eleven subtypes per variant (thrown or pushed-back debris, sparks, alternate camera that
//!   follows an object with a delay, sparkles, glow, puffs, dust);
//! - 44: effects of a magician enemy's spells (six modes);
//! - 47: marker that blinks slowly;
//! - 49: accessory held by a carrier, which falls when the carrier goes into a given state;
//! - 50: effects attached to a carrier object (reflections, sparks, shaking), sixteen variants;
//! - 58: machine of the introduction scene (same sequence as type 36) and its reflection;
//! - 59: creature that springs up, rises with the camera, fires then leaps off the screen;
//! - 60: second double of a carrier (like 16, with sparks and thrown chunk);
//! - 61: creature of the introduction scene that comes out of the water, undulates and leaves;
//! - 64: thrown spark that falls back below the horizon;
//! - 65: object that follows a path then spawns another object;
//! - 66: four large animated characters in background 2;
//! - 67: column that rises under an object, then releases the players;
//! - 69: silhouette that paces at the top of the screen;
//! - 70: counter of a scene shop (sale: vendor, sign and items, never exercised);
//! - 76: scene timer that launches a transition;
//! - 82: spark placed next to its creator; 83: rain of sparks from the top of the screen;
//! - 84: trigger of the ending sequence (fade, camera, script tasks);
//! - 85: piece of the ending sequence; 86: flame wisp that moves away from its creator;
//! - 87: barrier switch (placed by the variant, signals a player or a barrel touching it);
//! - 88: barrier, which creates its switches and opens when all are pressed;
//! - 90: dust drifting at the top of the screen; 91: another screen element fixed to the camera.
//!
//! The states that no scenario exercises stay in original code (`Cx::call` on their address).

use crate::families::cx::Bench;
use crate::families::cx::{handlers, Cx};
use crate::mem::{rd16, rd8, wr16, wr32, wr8, A5};
use crate::objects::Obj;
use crate::objects::pools;
use crate::objects::Current;
use crate::machine::Machine;

mod shops;
use shops::*;
mod carriers;
use carriers::*;
mod scenery_fx;
use scenery_fx::*;
mod intro;
use intro::*;
mod voting;
use voting::*;
mod attacks;
use attacks::*;
mod mechanisms;
use mechanisms::*;
mod ending;
use ending::*;
mod screen;
use screen::*;
mod paths;
use paths::*;

/// Shared fields.
mod f {
    pub const STATE: u32 = 0x04;
    pub const SUB_STATE: u32 = 0x05;
    pub const VARIANT: u32 = 0x03;
    pub const HIDDEN: u32 = 0x40;
}

/// Players: four records of 0x200 bytes.
const PLAYERS: u32 = A5 + 0x5de;
const PLAYER_STRIDE: u32 = 0x200;

fn players() -> impl Iterator<Item = Obj> {
    (0..4).map(|k| Obj(PLAYERS + k * PLAYER_STRIDE))
}

fn free<R: Bench>(x: &mut Cx<'_, R>) {
    x.free(&pools::EFFCT);
}

/// Tells whether `o` is in the box `(dx, half-width, dy, half-depth)` centered on the current object
/// offset by (dx, dy): unsigned 16-bit comparisons, like the original.
fn in_box<R: Bench>(x: &mut Cx<'_, R>, o: Obj, (dx, half_w, dy, half_d): (u16, u16, u16, u16)) -> bool {
    let (ox, oy) = (o.u16(x.b, Obj::X), o.u16(x.b, Obj::Y));
    let left = x.u16(Obj::X).wrapping_add(dx).wrapping_sub(half_w);
    if left > ox || left.wrapping_add(half_w.wrapping_mul(2)) < ox {
        return false;
    }
    let far = x.u16(Obj::Y).wrapping_add(dy).wrapping_add(half_d);
    far >= oy && far.wrapping_sub(half_d.wrapping_mul(2)) <= oy
}

handlers! {
    0xaa8e8 "effct_fixed_screen" screen_piece,
    0xabccc "effct_dalle" floor_tile,
    0xad476 "effct_maillon" chain_link,
    0xae52a "effct_double" double,
    0xaed4c "effct_bourse" purse,
    0xaf3f2 "effct_shop" shop,
    0xafe20 "effct_vote" vote,
    0xb1818 "effct_poisson" fish,
    0xb2dea "effct_plateau" ferry,
    0xb3050 "effct_flotteur" floater,
    0xb33ba "effct_title" title_card,
    0xb3454 "effct_result" result_card,
    0xb3570 "effct_projete" lob,
    0xb37ac "effct_hit" blow,
    0xb3c48 "effct_thrown" toss,
    0xb40f8 "effct_scenery" scenery,
    0xb4f88 "effct_spell" spell,
    0xb5764 "effct_marker" beacon,
    0xb5dbe "effct_accessory_carried" prop,
    0xb5fb6 "effct_attache" attached,
    0xb6d48 "effct_machine" machine,
    0xb6ef2 "effct_surgissant" riser,
    0xb7248 "effct_double_b" double2,
    0xb751e "effct_nageur" swimmer,
    0xb7a14 "effct_shard_falling" spark_fall,
    0xb7ac8 "effct_messager" courier,
    0xb7c40 "effct_letters" big_letters,
    0xb7e26 "effct_column" pillar,
    0xb802a "effct_silhouette" pacer,
    0xb80bc "effct_comptoir" counter,
    0xb88a8 "effct_minuteur" scene_timer,
    0xb97d8 "effct_shard_placed" spark,
    0xb997c "effct_rain" shard_rain,
    0xb9aba "effct_finale" finale_trigger,
    0xb9bbc "effct_end_piece" ending_piece,
    0xb9ca6 "effct_ember" ending_wisp,
    0xb9db8 "effct_interrupteur" barrier_switch,
    0xb9f36 "effct_barrier" barrier,
    0xba1aa "effct_dust_high" drift_dust,
    0xba244 "effct_fixed_screen_b" screen_piece_b,
}
