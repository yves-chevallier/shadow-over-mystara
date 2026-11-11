use crate::world::objects::player::Player;
use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::vars;
use crate::actors::families::shell::records::{Base};
use crate::actors::families::shell::tables::{tables, Read};

// ---------------------------------------------------------------------------------------------
// Type 3: equipment (0xdb7de)

pub const ANIMATIONS: RomAddr = RomAddr(0x1bf156);
pub const FIRST_EQUIPPED: u32 = 0x10d;
pub const SECOND_EQUIPPED: u32 = 0x10e;

/// `0xdba96`: temporary bonus given by variants 0x24 to 0x27.
pub(super) fn equipment_bonus<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let player = pointer(ctx, o, field::OWNER);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if !(variant != 0x24) {
        return player.set_u8(ctx.b, Player::GLOW_SHIELD, 0x14);
    }
    if !(variant != 0x25) {
        return player.set_u8(ctx.b, Player::GLOW_WEAPON, 0x14);
    }
    if !(variant != 0x26) {
        player.set_u8(ctx.b, Player::GLOW_RING, 0x14);
    }
    if !(variant != 0x27) {
        player.set_u8(ctx.b, Player::GLOW_OTHER, 0x14);
    }
}

/// `0xdbb0e`: successful pickup: bonus, score event and sound. Returns false (accepted).
pub(super) fn equipment_accepted<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, player: Obj) -> bool {
    equipment_bonus(ctx, o);
    let event = 0x11 + ((player.u8(ctx.b, Obj::KIND) as u16) << 8);
    crate::world::events::enqueue_points(ctx.b, event);
    effect_pickup(ctx, ctx.obj(), 5);
    false
}

/// `0xdbafa`: pickup refused: the object will be thrown off.
pub(super) fn equipment_refused<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    o.set_u8(ctx.b, field::SUB_STATE, 6);
    o.set_u8(ctx.b, field::SUB_SUB_STATE, 0);
    o.set_u8(ctx.b, inventory::DELAY, 0xc);
    true
}

/// Group of an equipment (byte 7 of its sheet).
pub(super) fn equipment_group<R: Bench>(ctx: &mut Cx<'_, R>, id: u8) -> u8 {
    let rank = ((id.wrapping_sub(0x2e) as u32) << 3) & 0xffff;
    tables().item_equipment_sheets.at8(ctx.b, 7 + rank)
}

/// `0xdbad8`: stores the equipment in the owner's inventory. Returns true if refused.
pub(super) fn equipment_add<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    let player = pointer(ctx, o, field::OWNER);
    let line = ((o.u8(ctx.b, Obj::VARIANT) as u32) << 3) & 0xffff;
    let class = (player.u8(ctx.b, Obj::VARIANT) >> 1) as u32;
    let allowed = tables().item_equipment_sheets.at8(ctx.b, line + class);
    if !(allowed != 0) {
        return equipment_refused(ctx, o);
    }
    let mut id = tables().item_equipment_sheets.at8(ctx.b, 6 + line);
    // already equipped?
    for off in [FIRST_EQUIPPED, SECOND_EQUIPPED] {
        let equipped = player.u8(ctx.b, off).wrapping_add(0x2e);
        if equipped == id {
            return equipment_accepted(ctx, o, player);
        }
    }
    // is the identifier already in the inventory?
    for i in 0..6u32 {
        let slot = player.u8(ctx.b, inventory::IDS + i);
        let empty = slot == inventory::EMPTY;
        if !empty {
            let object = slot <= 0x2d;
            if object {
                // inventory object: only if identical
                if slot == id {
                    return equipment_accepted(ctx, o, player);
                }
            } else {
                let (g_slot, g_new) = (equipment_group(ctx, slot), equipment_group(ctx, id));
                if !(g_new != g_slot) {
                    // same group: keep the better one
                    if !(id as i8 > slot as i8) {
                        id = slot;
                    }
                    player.set_u8(ctx.b, inventory::IDS + i, id);
                    return equipment_accepted(ctx, o, player);
                }
            }
        }
    }
    // equipment held in hand (byte 0x10d, or 0x10e for class 4)
    let mut door = player.u8(ctx.b, FIRST_EQUIPPED);
    let class_main = player.u8(ctx.b, Obj::VARIANT) >> 1;
    if !(class_main != 4) {
        door = player.u8(ctx.b, SECOND_EQUIPPED);
    }
    let group_door = tables().item_equipment_sheets.at8(ctx.b, 7 + ((door as u32) << 3));
    let group_new = equipment_group(ctx, id);
    if !(group_new != group_door) {
        let new = id.wrapping_sub(0x2e);
        if !(door as i8 > new as i8) {
            player.set_u8(ctx.b, FIRST_EQUIPPED, new);
        }
        return equipment_accepted(ctx, o, player);
    }
    // a free slot
    for i in 0..6u32 {
        let slot = player.u8(ctx.b, inventory::IDS + i);
        if !(slot != inventory::EMPTY) {
            player.set_u8(ctx.b, inventory::IDS + i, id);
            equipment_bonus(ctx, o);
            return false;
        }
    }
    // swap with the first slot, unless it is locked for this class
    let first = player.u8(ctx.b, inventory::IDS);
    for k in 0..8u32 {
        let lock = tables().item_equipment_locked.at8(ctx.b, (class << 3) + k);
        if first == lock {
            return equipment_refused(ctx, o);
        }
    }
    let old_quantity = player.u8(ctx.b, inventory::QUANTITIES);
    // D0.w: the class × 8 (`lsr.w #1` then `lsl.w #3` of byte +3) keeps its high byte under the identifier; it is
    // non-zero only if the owner is not a player (null pointer: ROM bytes)
    let index = ((class << 3) as u16 & 0xff00) | first as u16;
    object_ejected(ctx, o, player, index, old_quantity);
    player.set_u8(ctx.b, inventory::IDS, id);
    player.set_u8(ctx.b, inventory::QUANTITIES, 0);
    equipment_bonus(ctx, o);
    false
}

pub(super) fn equipment_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let mode = o.u8(ctx.b, field::MODE);
    if mode != 0 {
        let delay = o.u8(ctx.b, inventory::DELAY);
        if !(delay == 0) {
            o.set_u8(ctx.b, inventory::DELAY, delay - 1);
            return;
        }
        add_u8(ctx, o, Obj::STATE, 2);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
    } else if airborne(ctx, o) {
        state_next(ctx, o, sub::FALL);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
    } else {
        state_next(ctx, o, sub::WAIT);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        o.set_u16(ctx.b, field::DURATION, 0x4b0);
    }
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let index = tables().item_equipment_indices.at8(ctx.b, variant as u32);
    o.set_u8(ctx.b, field::INDEX, index);
    animate_and_sort(ctx, ANIMATIONS.0, variant);
}

/// Substate 6: the refused equipment is thrown off in an arc; on landing, a variant 0x0f picked up by class 1 more than
/// seven times triggers a scene.
pub(super) fn equipment_reject<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let launches = o.u8(ctx.b, field::SUB_SUB_STATE) != 0;
    if !launches {
        let delay = o.u8(ctx.b, inventory::DELAY).wrapping_sub(1);
        o.set_u8(ctx.b, inventory::DELAY, delay);
        if delay != 0 {
            return;
        }
        add_u8(ctx, o, field::SUB_SUB_STATE, 2);
        let player = pointer(ctx, o, field::OWNER);
        let y = player.u16(ctx.b, Obj::Y).wrapping_sub(1);
        o.set_u16(ctx.b, Obj::Y, y);
        o.set_u16(ctx.b, Obj::Z, 0x10);
        o.set_u16(ctx.b, Obj::VZ, 0x200);
        o.set_u16(ctx.b, Obj::AZ, 0xffc0);
        o.set_u32(ctx.b, Obj::VX, 0x0280_0000);
        let (cam_x, _) = ctx.camera();
        let limit = cam_x.wrapping_add(0xc0);
        let x = o.u16(ctx.b, Obj::X);
        if !(limit as i16 > x as i16) {
            let v = o.u32(ctx.b, Obj::VX).wrapping_neg();
            o.set_u32(ctx.b, Obj::VX, v);
        }
        return ctx.reinsert();
    }
    ctx.move_xz();
    let z = o.u16(ctx.b, Obj::Z);
    if z as i16 >= 0 {
        return;
    }
    o.set_u8(ctx.b, field::SUB_STATE, sub::WAIT);
    o.set_u8(ctx.b, field::SUB_SUB_STATE, 0);
    o.set_u32(ctx.b, Obj::Z, 0);
    o.set_u8(ctx.b, field::PICKUP, 0);
    let player = pointer(ctx, o, field::OWNER);
    let class = player.u8(ctx.b, Obj::VARIANT) >> 1;
    if class != 1 {
        return;
    }
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if variant != 0xf {
        return;
    }
    let n = vars::SECRET_PICKUPS.get(ctx.b).wrapping_add(1);
    vars::SECRET_PICKUPS.set(ctx.b, n);
    if n <= 7 {
        return;
    }
    o.set_u16(ctx.b, field::DURATION, 0x4b0);
    o.set_u8(ctx.b, Obj::FLAGS, 1);
    o.set_u8(ctx.b, Obj::VARIANT, 0xc);
    ctx.anim_start(ANIMATIONS.0, 0xc);
    effects_creators::posed_flash(ctx.b, o);
    vars::PICKED_5.set(ctx.b, 1);
    scene_set_of(ctx, 0x29);
}

pub(super) fn equipment_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    match sub_state {
        sub::FALL => {
            let in_flight = ctx.move_xyz().gt();
            if !in_flight {
                add_u8(ctx, o, field::SUB_STATE, 2);
                o.set_u16(ctx.b, field::DURATION, 0x4b0);
                o.set_u32(ctx.b, Obj::Z, 0);
                let mode = o.u8(ctx.b, field::MODE);
                if !(mode != 0) {
                    in_view(ctx, o);
                }
            }
            fall_end(ctx);
        }
        sub::WAIT => reserve_wait(ctx, o),
        sub::PICKUP => {
            let player = pointer(ctx, o, field::OWNER);
            let ready = player.u8(ctx.b, Base::ANIM_EVENT) != 0;
            if !ready {
                return;
            }
            let refuse = equipment_add(ctx, o);
            if !refuse {
                add_u8(ctx, o, Obj::STATE, 2);
            }
            mark_pickup(ctx, o);
        }
        6 => equipment_reject(ctx, o),
        _ => crate::actors::families::effects::bad_index(sub_state as u32),
    }
}

pub(super) fn equipment_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    match state {
        state::INIT => equipment_init(ctx, o),
        state::ACTIVE => equipment_active(ctx, o),
        state::FREED | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::actors::families::effects::bad_index(state as u32),
    }
}
