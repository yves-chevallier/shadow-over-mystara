//! ITEM family (class 0xC): pickup objects. Dispatch `0x39f34`, type table `0x39f5a` (10 types, 32 slots of 0x60
//! bytes).
//!
//! An ITEM object is born (created by an enemy, a chest or scenery) at the position of its creator; it falls if it is
//! in the air, stays on the ground for a limited time blinking before disappearing, and produces its effect when a
//! player picks it up (the player's code sets `PICKUP` and `OWNER`). See `docs/familles.md` for the description of
//! each type.

use cps2::Cps2Bus;

use crate::mem::{rd16, wr16, wr32, A5};
use crate::objects::pools;
use crate::objects::Obj;

use super::{add_u8, effects_creators, handler, pointer};
use super::effects_creators::leftovers::Regs;
use super::cx::{Bench, Cx};
use crate::objects::Current;
use crate::machine::Machine;

/// Values of the state byte `+4`.
mod state {
    pub const INIT: u8 = 0;
    pub const ACTIVE: u8 = 2;
    pub const FREED: u8 = 4;
    /// Set by `obj_cull_offscreen`: treated as `FREED`.
    pub const OUTSIDE_SCREEN: u8 = 6;
}

/// Values of the substate byte `+5` of the active state (falling types).
mod sub {
    pub const FALL: u8 = 0;
    pub const WAIT: u8 = 2;
    pub const PICKUP: u8 = 4;
}

/// Fields specific to ITEMs.
mod field {
    pub const SUB_STATE: u32 = 0x05;
    pub const SUB_SUB_STATE: u32 = 0x06;
    pub const FLIP: u32 = 0x2c;
    pub const GAME_SHADOWS: u32 = 0x2e;
    pub const MODE: u32 = 0x2f;
    pub const DURATION: u32 = 0x42;
    pub const OWNER: u32 = 0x50;
    pub const SHIMMER: u32 = 0x54;
    pub const PICKUP: u32 = 0x58;
    pub const INDEX: u32 = 0x5d;
}

/// Shadow set common to ITEMs.
const SHADOWS: u8 = 0x3c;
/// "Collectible picked up" flags (one byte per object), `A5 - 0x5b88`.
const PICKED: i32 = -0x5b88;

/// Subroutines not rewritten.
mod call {
    /// Common end of the potion effects.
    pub const END_EFFECT: crate::sound::Sound = crate::sound::CMD_132_232;
}

/// `0xae3ec`: sparkle (EFFCT 15) above the object, according to `SHIMMER`.
fn shimmer<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    effects_creators::twinkle(ctx.b, o);
}

/// `0xb12c0` (D0 = `variant`): pickup effect (EFFCT 27) attached to the player owning the object.
fn effect_pickup<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, variant: u8) {
    effects_creators::pickup(ctx.b, o, variant);
}

/// `0xc8312`: SET object of the scene; `None` if the pool is full.
fn scene_set_of<R: Bench>(ctx: &mut Cx<'_, R>, variant: u8) -> Option<Obj> {
    super::creators::scene_set(ctx.b, variant)
}

/// `0xaba16`: single-animation burst (EFFCT 7) on the object.
fn shard<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    effects_creators::flash(ctx.b, o);
}

/// Goes to the next state with a given substate (`addq.b #2,(4,A0)`, sub-substate zero).
fn state_next<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, sub_state: u8) {
    add_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, field::SUB_STATE, sub_state);
    o.set_u16(ctx.b, field::SUB_SUB_STATE, 0);
}

/// An object created above the ground (non-zero altitude) starts by falling.
fn airborne<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    let z = o.u16(ctx.b, Obj::Z);
    !(z == 0) // tst.w (0xc,A0) ; beq: on the ground
}

/// Marks the collectible object `+0x5d` as picked up.
fn mark_pickup<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let index = o.u8(ctx.b, field::INDEX) as i32;
    ctx.set_ram8(PICKED + index, 1);
}

/// Brings the object back into the visible area: at most as high as `camera.y + 0x20`, between `camera.x + 0x18` and
/// `camera.x + 0x168` (block copied into each type).
fn in_view<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let (cam_x, cam_y) = ctx.camera();
    let y_min = cam_y.wrapping_add(0x20);
    let y = o.u16(ctx.b, Obj::Y);
    if !(y_min < y) {
        o.set_u16(ctx.b, Obj::Y, y_min);
    }
    let x_min = cam_x.wrapping_add(0x18);
    let x = o.u16(ctx.b, Obj::X);
    if !(x_min as i16 <= x as i16) {
        o.set_u16(ctx.b, Obj::X, x_min);
    } else {
        let x_max = cam_x.wrapping_add(0x168);
        if !(x_max > x) {
            o.set_u16(ctx.b, Obj::X, x_max);
        }
    }
}

/// Lifetime countdown; below 0x3c frames the object blinks (bit 1 of `FLAGS` hides the object every other frame, the
/// phase is given by `D7`); at zero it is freed.
fn expire_and_blinks<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let remainder = o.u16(ctx.b, field::DURATION).wrapping_sub(1);
    o.set_u16(ctx.b, field::DURATION, remainder);
    if !(remainder as i16 >= 0) {
        o.set_u8(ctx.b, Obj::STATE, state::FREED);
        o.set_u8(ctx.b, Obj::ACTIVE, 2);
    }
    if remainder > 0x3c {
        return;
    }
    let phase = remainder.wrapping_add(ctx.counter());
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
    if phase & 1 == 0 {
        return;
    }
    o.set_u8(ctx.b, Obj::FLAGS, flags & !2);
}

/// Common end of the fall state: bring back into the area, shadow, sort by depth.
fn fall_end<R: Bench>(ctx: &mut Cx<'_, R>) {
    ctx.keep_in_zone();
    ctx.push_terrain();
    ctx.reinsert();
}

/// Starts script `number` of the animation set `game`, then sorts the object (end of the initializations).
fn animate_and_sort<R: Bench>(ctx: &mut Cx<'_, R>, game: u32, number: u8) {
    ctx.anim_start(game, number as u16);
    ctx.reinsert();
}

/// "Picked up" substate: waits for the player to finish its gesture (owner ready) or a forced pickup (`PICKUP` = 2).
/// Returns false as long as it must wait.
fn pickup_ready<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    let pickup = o.u8(ctx.b, field::PICKUP);
    if pickup == 2 {
        return true;
    }
    let player = pointer(ctx, o, field::OWNER);
    player.u8(ctx.b, 0x33) != 0
}

mod treasure;
mod consumable;
mod inventory;
mod bonus;
mod scene;
mod reserve;
mod escort;
mod contact;
mod equipment;
use {treasure::*, consumable::*, inventory::*, bonus::*, scene::*, reserve::*, escort::*, contact::*, equipment::*};


handler!(ITEM_3, equipment::ENTRY, "item_equipment", equipment_handler);
handler!(ITEM_0, treasure::ENTRY, "item_treasure", treasure);
handler!(ITEM_1, consumable::ENTRY, "item_consumable", consumable_handler);
handler!(ITEM_4, inventory::ENTRY, "item_inventory", inventory_handler);
handler!(ITEM_5, bonus::ENTRY, "item_bonus", bonus_handler);
handler!(ITEM_6, scene::ENTRY, "item_scene", scene_handler);
handler!(ITEM_7, reserve::ENTRY, "item_reserve", reserve_handler);
handler!(ITEM_8, contact::ENTRY, "item_contact", contact_handler);
handler!(ITEM_9, escort::ENTRY, "item_accompagnement", escort_handler);

// ---------------------------------------------------------------------------------------------
// Subroutines called outside the handlers

/// `0xdabba`: clears the array of collectible objects picked up (48 longs at `A5 - 0x5b88`).
pub fn clear_picked_up(b: &mut Cps2Bus) {
    let start = A5.wrapping_add(PICKED as u32);
    for k in 0..48 {
        wr32(b, start + 4 * k, 0);
    }
}

/// A player's equipment that can drop when it is hit: (byte of the carried object, hit counter, chance table per hit).
struct Door {
    object: u32,
    hits: u32,
    chances: u32,
}

/// The hit player loses at most one object per hit: shield (`+0x165`, 0 to 3), special weapon (`+0x163`) then ring
/// (`+0x162`, except 1). Each hit decrements a counter; as long as it stays positive, the drop depends on a draw (bit
/// `rng & 0xf` of the table word for this counter, in the clear); the object falls through `0xcc352`
/// (`creators::set_a`, D0.b = its object number).
const SHIELD: Door = Door { object: 0x165, hits: 0x16d, chances: 0xdb7c4 };
const WEAPON: Door = Door { object: 0x163, hits: 0x16c, chances: 0xdb7b8 };
const RING: Door = Door { object: 0x162, hits: 0x16b, chances: 0xdb7d0 };
/// Number of objects dropped at this hit (word).
const DROPPED: u32 = A5 - 0x5ac8;

/// The hit counter of `p` is exhausted, or the draw makes the object fall. The registers the original leaves (A4 = the
/// chance table, D0.w = the draw, D2.w = the mask) go to `r`.
fn falls(r: &mut impl Regs, b: &mut Cps2Bus, o: Obj, p: &Door) -> bool {
    let n = o.u8(b, p.hits).wrapping_sub(1);
    o.set_u8(b, p.hits, n);
    if (n as i8) < 0 {
        return true;
    }
    r.a(4, p.chances);
    let i = 2 * n as u32;
    r.d(0, i);
    let mask = rd16(b, p.chances + i);
    r.d_word(2, mask);
    let bit = crate::rng::random(b) & 0xf;
    r.d_word(0, bit);
    mask >> bit & 1 != 0
}

fn dropped(r: &mut impl Regs, b: &mut Cps2Bus, o: Obj, p: &Door, number: u8) {
    r.d_byte(0, number);
    super::creators::set_a_via(r, b, number, |b| o.u8(b, Obj::VARIANT));
    o.set_u8(b, p.object, 0xff);
    o.set_u8(b, p.hits, 0);
}

fn count_dropped(b: &mut Cps2Bus) {
    let n = rd16(b, DROPPED).wrapping_add(1);
    wr16(b, DROPPED, n);
}

/// `0xdb6c0`: equipment dropped by the hit player `o` (`player::hurt`); the registers the original leaves (D0, D2, A4)
/// go to `r`.
pub(crate) fn fall_equipment(r: &mut impl Regs, b: &mut Cps2Bus, o: Obj) {
    wr16(b, DROPPED, 0);
    let shield = o.u8(b, SHIELD.object);
    if shield <= 3 && falls(r, b, o, &SHIELD) {
        dropped(r, b, o, &SHIELD, shield + 4);
        count_dropped(b);
    }
    if (o.u8(b, WEAPON.object) as i8) >= 0 && falls(r, b, o, &WEAPON) && rd16(b, DROPPED) == 0 {
        r.d(0, 0);
        dropped(r, b, o, &WEAPON, 3);
        count_dropped(b);
    }
    let ring = o.u8(b, RING.object);
    if (ring as i8) >= 0 && ring != 1 && falls(r, b, o, &RING) && rd16(b, DROPPED) == 0 {
        r.d_byte(0, ring);
        super::creators::set_a_via(r, b, ring, |b| o.u8(b, Obj::VARIANT));
        o.set_u8(b, RING.object, 0xff);
        // character 5 keeps its original ring (1)
        let character = o.u8(b, 0x3) >> 1;
        r.d(0, character as u32);
        if character == 5 {
            o.set_u8(b, RING.object, 1);
        }
        o.set_u8(b, RING.hits, 0);
    }
}
