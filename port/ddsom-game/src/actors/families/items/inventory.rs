use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::records::Base;
use crate::actors::families::shell::tables::{tables, Read};

// ---------------------------------------------------------------------------------------------
// Type 4: inventory objects (0xdbdf6)

pub const ANIMATIONS: RomAddr = RomAddr(0x1bf3c6);
pub const DURATION_TO_GROUND: u16 = 0x4b0;
pub const EMPTY: u8 = 0x56;
/// Player fields.
pub const IDS: u32 = 0x140;
pub const QUANTITIES: u32 = 0x148;
/// Quantity of the object (`+0x5a`) and delay before appearing (`+0x59`).
pub const QUANTITY: u32 = 0x5a;
pub const DELAY: u32 = 0x59;

/// `0xdc28a`: recreates in the air the object `old` (replaced in the inventory) with its quantity. `index`: D0.w of the
/// caller, the identifier in the low byte (the high byte is non-zero only with a real player, see `equipment_add`).
pub(super) fn object_ejected<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, player: Obj, index: u16, quantity: u8) {
    let Some(n) = ctx.alloc(&pools::ITEM) else {
        return;
    };
    n.set_u8(ctx.b, Obj::ACTIVE, 1);
    n.set_u8(ctx.b, field::MODE, 1);
    n.set_u8(ctx.b, QUANTITY, quantity);
    // `add.w D0,D0` then `(d8,PC,D0.w)`: sign-extended word
    let rank = index.wrapping_mul(2) as i16 as i32 as u32;
    let kind = tables().item_inventory_recreated.at8(ctx.b, rank);
    n.set_u8(ctx.b, Obj::KIND, kind);
    let variant = tables().item_inventory_recreated.at8(ctx.b, rank.wrapping_add(1));
    n.set_u8(ctx.b, Obj::VARIANT, variant);
    n.set_u16(ctx.b, field::OWNER, player.0 as u16);
    let x = o.u16(ctx.b, Obj::X);
    n.set_u16(ctx.b, Obj::X, x);
    let y = player.u16(ctx.b, Obj::Y).wrapping_sub(1);
    n.set_u16(ctx.b, Obj::Y, y);
    n.set_u16(ctx.b, Obj::Z, 0x10);
    n.set_u16(ctx.b, Obj::AZ, 0xffc0);
    n.set_u8(ctx.b, DELAY, 0xc);
}

/// Pickup refused (`0xdc19a`): the object will be thrown off (substate 6).
pub(super) fn pickup_refused<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    o.set_u8(ctx.b, field::SUB_STATE, 6);
    o.set_u8(ctx.b, field::SUB_SUB_STATE, 0);
    o.set_u8(ctx.b, DELAY, 0xc);
    true
}

/// Score event and sound of an object that changes nothing else (`0xdc124`).
pub(super) fn event_inventory<R: Bench>(ctx: &mut Cx<'_, R>, player: Obj) {
    let character = player.u8(ctx.b, Obj::KIND) as u32;
    crate::world::events::enqueue_points(ctx.b, ((character << 8) + 0x11) as u16);
    effect_pickup(ctx, ctx.obj(), 5);
}

/// `0xdc0aa`: stores the object in the owner's inventory. Returns true if the pickup is refused.
pub(super) fn inventory_add<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    let player = pointer(ctx, o, field::OWNER);
    let line = (o.u8(ctx.b, Obj::VARIANT) as u32) << 3 & 0xffff;
    let id = tables().item_inventory_sheets.at8(ctx.b, line);
    let class = player.u8(ctx.b, Obj::VARIANT) >> 1;
    let forbidden = tables().item_inventory_sheets.at8(ctx.b, 1 + (class.wrapping_add(line as u8)) as u32);
    if forbidden != 0 {
        return pickup_refused(ctx, o);
    }
    if !(id != 0x23) && (class == 2) {
        event_inventory(ctx, player);
        return false;
    }
    let quantity = o.u8(ctx.b, QUANTITY);
    // 1. the object is already carried: the quantity increases
    for i in 0..6u32 {
        let door = player.u8(ctx.b, IDS + i);
        if door != id {
            continue;
        }
        let n = player.u8(ctx.b, QUANTITIES + i).wrapping_add(quantity);
        player.set_u8(ctx.b, QUANTITIES + i, n);
        let variant = o.u8(ctx.b, Obj::VARIANT);
        // single-copy variants: 7, 0x10, 0x11
        let unique = (variant == 7) || (variant == 0x10) || (variant == 0x11);
        if unique {
            player.set_u8(ctx.b, QUANTITIES + i, 1);
        } else if !(n <= 9) {
            player.set_u8(ctx.b, QUANTITIES + i, 9);
        } else {
            return false;
        }
        event_inventory(ctx, player);
        return false;
    }
    // 2. an empty slot
    for i in 0..6u32 {
        let door = player.u8(ctx.b, IDS + i);
        if door != EMPTY {
            continue;
        }
        player.set_u8(ctx.b, IDS + i, id);
        player.set_u8(ctx.b, QUANTITIES + i, quantity);
        return false;
    }
    // 3. swap with the first slot, unless it is locked for this class
    let first = player.u8(ctx.b, IDS);
    for k in 0..8u32 {
        let lock = tables().item_inventory_locked.at8(ctx.b, ((class as u32) << 3) + k);
        if !(first != lock) {
            return pickup_refused(ctx, o);
        }
    }
    let old_quantity = player.u8(ctx.b, QUANTITIES);
    player.set_u8(ctx.b, IDS, id);
    player.set_u8(ctx.b, QUANTITIES, quantity);
    object_ejected(ctx, o, player, first as u16, old_quantity);
    false
}

pub(super) fn inventory_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let mode = o.u8(ctx.b, field::MODE);
    if mode != 0 {
        // thrown object: waits for the end of the delay
        let delay = o.u8(ctx.b, DELAY);
        if !(delay == 0) {
            o.set_u8(ctx.b, DELAY, delay - 1);
            return;
        }
        add_u8(ctx, o, Obj::STATE, 2);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        let index = tables().item_inventory_indices.at8(ctx.b, variant as u32);
        o.set_u8(ctx.b, field::INDEX, index);
        animate_and_sort(ctx, ANIMATIONS.0, variant);
        return;
    }
    if airborne(ctx, o) {
        state_next(ctx, o, sub::FALL);
        o.set_u8(ctx.b, QUANTITY, 1);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        let shimmering = tables().item_inventory_shimmers.at8(ctx.b, variant as u32);
        o.set_u8(ctx.b, field::SHIMMER, shimmering);
        let index = tables().item_inventory_indices.at8(ctx.b, variant as u32);
        o.set_u8(ctx.b, field::INDEX, index);
    } else {
        state_next(ctx, o, sub::WAIT);
        o.set_u8(ctx.b, QUANTITY, 1);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        o.set_u16(ctx.b, field::DURATION, DURATION_TO_GROUND);
        let shimmering = tables().item_inventory_shimmers.at8(ctx.b, variant as u32);
        o.set_u8(ctx.b, field::SHIMMER, shimmering);
        let index = tables().item_inventory_indices.at8(ctx.b, variant as u32);
        o.set_u8(ctx.b, field::INDEX, index);
        shimmer(ctx, o);
    }
    animate_and_sort(ctx, ANIMATIONS.0, variant);
}

/// Substate 6: the refused object is thrown off in an arc toward the outside of the screen then falls back.
pub(super) fn inventory_reject<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let launches = o.u8(ctx.b, field::SUB_SUB_STATE) != 0;
    if !launches {
        // wait for the delay, then throw
        let delay = o.u8(ctx.b, DELAY).wrapping_sub(1);
        o.set_u8(ctx.b, DELAY, delay);
        if delay != 0 {
            return;
        }
        add_u8(ctx, o, field::SUB_SUB_STATE, 2);
        let player = pointer(ctx, o, field::OWNER);
        let y = player.u16(ctx.b, Obj::Y).wrapping_sub(1);
        o.set_u16(ctx.b, Obj::Y, y);
        o.set_u16(ctx.b, Obj::Z, 0x10);
        o.set_u16(ctx.b, Obj::VZ, 0);
        o.set_u16(ctx.b, Obj::AZ, 0xffc0);
        o.set_u32(ctx.b, Obj::VX, 0x0200_0000);
        let (cam_x, _) = ctx.camera();
        let limit = cam_x.wrapping_add(0xc0);
        let x = o.u16(ctx.b, Obj::X);
        if !(limit as i16 > x as i16) {
            let v = o.u32(ctx.b, Obj::VX).wrapping_neg();
            o.set_u32(ctx.b, Obj::VX, v);
        }
        ctx.reinsert();
        return;
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
}

pub(super) fn inventory_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    match sub_state {
        sub::FALL => {
            let in_flight = ctx.move_xyz().gt();
            if !in_flight {
                add_u8(ctx, o, field::SUB_STATE, 2);
                o.set_u16(ctx.b, field::DURATION, DURATION_TO_GROUND);
                o.set_u32(ctx.b, Obj::Z, 0);
                let mode = o.u8(ctx.b, field::MODE);
                if !(mode != 0) {
                    in_view(ctx, o);
                    shimmer(ctx, o);
                }
            }
            ctx.anim_step();
            fall_end(ctx);
        }
        sub::WAIT => {
            let pickup = o.u8(ctx.b, field::PICKUP);
            if !(pickup == 0) {
                add_u8(ctx, o, field::SUB_STATE, 2);
            }
            ctx.anim_step();
            expire_and_blinks(ctx, o);
        }
        sub::PICKUP => {
            let player = pointer(ctx, o, field::OWNER);
            let ready = player.u8(ctx.b, Base::ANIM_EVENT) != 0;
            if !ready {
                return;
            }
            let refuse = inventory_add(ctx, o);
            if !refuse {
                add_u8(ctx, o, Obj::STATE, 2);
            }
            mark_pickup(ctx, o);
        }
        6 => inventory_reject(ctx, o),
        _ => crate::actors::families::effects::bad_index(sub_state as u32),
    }
}

pub(super) fn inventory_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    match state {
        state::INIT => inventory_init(ctx, o),
        state::ACTIVE => inventory_active(ctx, o),
        state::FREED | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::actors::families::effects::bad_index(state as u32),
    }
}
