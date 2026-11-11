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

use crate::actors::families::cx::Bench;
use crate::actors::families::cx::{handlers, Cx};
use crate::world::mem::{rd16, rd8, wr16, wr32, wr8, A5};
use crate::world::objects::Obj;
use crate::world::objects::pools;
use crate::world::objects::Current;
use crate::world::machine::Machine;

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
pub mod records;
use records::*;
use crate::actors::families::effects::records::*;
use crate::actors::families::effects::vars;
use crate::actors::families::effects::tables::{held16, held8, speed_word, tables, Read, Table};

/// Shared fields.
mod f {
    pub const STATE: u32 = 0x04;
    pub const SUB_STATE: u32 = 0x05;
    pub const VARIANT: u32 = 0x03;
    pub const HIDDEN: u32 = 0x40;
}


fn players() -> impl Iterator<Item = Obj> {
    (0..4).map(|k| crate::world::ram::PLAYERS.nth(k))
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
    EFFCT_FIXED_SCREEN 0xaa8e8 "effct_fixed_screen" screen_piece,
    EFFCT_DALLE 0xabccc "effct_dalle" floor_tile,
    EFFCT_MAILLON 0xad476 "effct_maillon" chain_link,
    EFFCT_DOUBLE 0xae52a "effct_double" double,
    EFFCT_BOURSE 0xaed4c "effct_bourse" purse,
    EFFCT_SHOP 0xaf3f2 "effct_shop" shop,
    EFFCT_VOTE 0xafe20 "effct_vote" vote,
    EFFCT_POISSON 0xb1818 "effct_poisson" fish,
    EFFCT_PLATEAU 0xb2dea "effct_plateau" ferry,
    EFFCT_FLOTTEUR 0xb3050 "effct_flotteur" floater,
    EFFCT_TITLE 0xb33ba "effct_title" title_card,
    EFFCT_RESULT 0xb3454 "effct_result" result_card,
    EFFCT_PROJETE 0xb3570 "effct_projete" lob,
    EFFCT_HIT 0xb37ac "effct_hit" blow,
    EFFCT_THROWN 0xb3c48 "effct_thrown" toss,
    EFFCT_SCENERY 0xb40f8 "effct_scenery" scenery,
    EFFCT_SPELL 0xb4f88 "effct_spell" spell,
    EFFCT_MARKER 0xb5764 "effct_marker" beacon,
    EFFCT_ACCESSORY_CARRIED 0xb5dbe "effct_accessory_carried" prop,
    EFFCT_ATTACHE 0xb5fb6 "effct_attache" attached,
    EFFCT_MACHINE 0xb6d48 "effct_machine" machine,
    EFFCT_SURGISSANT 0xb6ef2 "effct_surgissant" riser,
    EFFCT_DOUBLE_B 0xb7248 "effct_double_b" double2,
    EFFCT_NAGEUR 0xb751e "effct_nageur" swimmer,
    EFFCT_SHARD_FALLING 0xb7a14 "effct_shard_falling" spark_fall,
    EFFCT_MESSAGER 0xb7ac8 "effct_messager" courier,
    EFFCT_LETTERS 0xb7c40 "effct_letters" big_letters,
    EFFCT_COLUMN 0xb7e26 "effct_column" pillar,
    EFFCT_SILHOUETTE 0xb802a "effct_silhouette" pacer,
    EFFCT_COMPTOIR 0xb80bc "effct_comptoir" counter,
    EFFCT_MINUTEUR 0xb88a8 "effct_minuteur" scene_timer,
    EFFCT_SHARD_PLACED 0xb97d8 "effct_shard_placed" spark,
    EFFCT_RAIN 0xb997c "effct_rain" shard_rain,
    EFFCT_FINALE 0xb9aba "effct_finale" finale_trigger,
    EFFCT_END_PIECE 0xb9bbc "effct_end_piece" ending_piece,
    EFFCT_EMBER 0xb9ca6 "effct_ember" ending_wisp,
    EFFCT_INTERRUPTEUR 0xb9db8 "effct_interrupteur" barrier_switch,
    EFFCT_BARRIER 0xb9f36 "effct_barrier" barrier,
    EFFCT_DUST_HIGH 0xba1aa "effct_dust_high" drift_dust,
    EFFCT_FIXED_SCREEN_B 0xba244 "effct_fixed_screen_b" screen_piece_b,
}
