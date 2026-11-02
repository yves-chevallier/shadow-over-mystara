use super::*;

// ---------------------------------------------------------------------------------------------
// Type 4: inventory objects (0xdbdf6)

/// Type 4, inventory objects (vials, secondary weapons, keys...). The player carries six slots (identifiers at
/// `+0x140`, quantities at `+0x148`; 0x56: empty slot). On pickup: refusal if the player's class cannot carry the
/// object (eight-byte table per variant); otherwise quantity + `+0x5a` (capped at 9, or 1 for variants 7, 0x10 and
/// 0x11), empty slot, or swap with the first slot, whose replaced object is recreated in the air (mode 1). A refused
/// object is thrown off in the air then becomes pickable again. The creation mode `+0x2f` is 1 for an object thrown by
/// a player: it waits `+0x59` frames before appearing.
pub const ENTRY: u32 = 0xdbdf6;
pub const TABLE_STATES: u32 = 0xdbe0c;
pub const TABLE_SUB_STATES: u32 = 0xdbf36;
pub const ANIMATIONS: u32 = 0x1bf3c6;
/// Sparkle per variant; pickup index per variant.
pub const SHIMMERS: u32 = 0xdbf04;
pub const INDICES: u32 = 0xdbf16;
/// Per variant, 8 bytes: object identifier then one prohibition byte per class.
pub const SHEETS: u32 = 0xdc1fa;
/// Per player class, 8 identifiers of objects that cannot be replaced.
pub const LOCKED: u32 = 0xdc1ca;
/// Per object identifier: (type, variant) of the ITEM recreated when it is replaced.
pub const RECREATED: u32 = 0xdc2d6;
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
    n.set_u8(ctx.b, inventory::QUANTITY, quantity);
    // `add.w D0,D0` then `(d8,PC,D0.w)`: sign-extended word
    let rank = index.wrapping_mul(2) as i16 as i32 as u32;
    let kind = ctx.prog8(inventory::RECREATED.wrapping_add(rank));
    n.set_u8(ctx.b, Obj::KIND, kind);
    let variant = ctx.prog8(inventory::RECREATED.wrapping_add(rank).wrapping_add(1));
    n.set_u8(ctx.b, Obj::VARIANT, variant);
    n.set_u16(ctx.b, field::OWNER, player.0 as u16);
    let x = o.u16(ctx.b, Obj::X);
    n.set_u16(ctx.b, Obj::X, x);
    let y = player.u16(ctx.b, Obj::Y).wrapping_sub(1);
    n.set_u16(ctx.b, Obj::Y, y);
    n.set_u16(ctx.b, Obj::Z, 0x10);
    n.set_u16(ctx.b, Obj::AZ, 0xffc0);
    n.set_u8(ctx.b, inventory::DELAY, 0xc);
}

/// Pickup refused (`0xdc19a`): the object will be thrown off (substate 6).
pub(super) fn pickup_refused<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    o.set_u8(ctx.b, field::SUB_STATE, 6);
    o.set_u8(ctx.b, field::SUB_SUB_STATE, 0);
    o.set_u8(ctx.b, inventory::DELAY, 0xc);
    true
}

/// Score event and sound of an object that changes nothing else (`0xdc124`).
pub(super) fn event_inventory<R: Bench>(ctx: &mut Cx<'_, R>, player: Obj) {
    let character = player.u8(ctx.b, 2) as u32;
    crate::events::enqueue_points(ctx.b, ((character << 8) + 0x11) as u16);
    effect_pickup(ctx, ctx.obj(), 5);
}

/// `0xdc0aa`: stores the object in the owner's inventory. Returns true if the pickup is refused.
pub(super) fn inventory_add<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    let player = pointer(ctx, o, field::OWNER);
    let line = (o.u8(ctx.b, Obj::VARIANT) as u32) << 3 & 0xffff;
    let id = ctx.data8(inventory::SHEETS + line);
    let class = player.u8(ctx.b, 3) >> 1;
    let forbidden = ctx.data8(inventory::SHEETS + 1 + (class.wrapping_add(line as u8)) as u32);
    if forbidden != 0 {
        return pickup_refused(ctx, o);
    }
    if !(id != 0x23) && (class == 2) {
        event_inventory(ctx, player);
        return false;
    }
    let quantity = o.u8(ctx.b, inventory::QUANTITY);
    // 1. the object is already carried: the quantity increases
    for i in 0..6u32 {
        let door = player.u8(ctx.b, inventory::IDS + i);
        if door != id {
            continue;
        }
        let n = player.u8(ctx.b, inventory::QUANTITIES + i).wrapping_add(quantity);
        player.set_u8(ctx.b, inventory::QUANTITIES + i, n);
        let variant = o.u8(ctx.b, Obj::VARIANT);
        // single-copy variants: 7, 0x10, 0x11
        let unique = (variant == 7) || (variant == 0x10) || (variant == 0x11);
        if unique {
            player.set_u8(ctx.b, inventory::QUANTITIES + i, 1);
        } else if !(n <= 9) {
            player.set_u8(ctx.b, inventory::QUANTITIES + i, 9);
        } else {
            return false;
        }
        event_inventory(ctx, player);
        return false;
    }
    // 2. an empty slot
    for i in 0..6u32 {
        let door = player.u8(ctx.b, inventory::IDS + i);
        if door != inventory::EMPTY {
            continue;
        }
        player.set_u8(ctx.b, inventory::IDS + i, id);
        player.set_u8(ctx.b, inventory::QUANTITIES + i, quantity);
        return false;
    }
    // 3. swap with the first slot, unless it is locked for this class
    let first = player.u8(ctx.b, inventory::IDS);
    for k in 0..8u32 {
        let lock = ctx.data8(inventory::LOCKED + ((class as u32) << 3) + k);
        if !(first != lock) {
            return pickup_refused(ctx, o);
        }
    }
    let old_quantity = player.u8(ctx.b, inventory::QUANTITIES);
    player.set_u8(ctx.b, inventory::IDS, id);
    player.set_u8(ctx.b, inventory::QUANTITIES, quantity);
    object_ejected(ctx, o, player, first as u16, old_quantity);
    false
}

pub(super) fn inventory_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let mode = o.u8(ctx.b, field::MODE);
    if mode != 0 {
        // thrown object: waits for the end of the delay
        let delay = o.u8(ctx.b, inventory::DELAY);
        if !(delay == 0) {
            o.set_u8(ctx.b, inventory::DELAY, delay - 1);
            return;
        }
        add_u8(ctx, o, Obj::STATE, 2);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        let index = ctx.data8(inventory::INDICES + variant as u32);
        o.set_u8(ctx.b, field::INDEX, index);
        animate_and_sort(ctx, inventory::ANIMATIONS, variant);
        return;
    }
    if airborne(ctx, o) {
        state_next(ctx, o, sub::FALL);
        o.set_u8(ctx.b, inventory::QUANTITY, 1);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        let shimmering = ctx.data8(inventory::SHIMMERS + variant as u32);
        o.set_u8(ctx.b, field::SHIMMER, shimmering);
        let index = ctx.data8(inventory::INDICES + variant as u32);
        o.set_u8(ctx.b, field::INDEX, index);
    } else {
        state_next(ctx, o, sub::WAIT);
        o.set_u8(ctx.b, inventory::QUANTITY, 1);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        o.set_u16(ctx.b, field::DURATION, inventory::DURATION_TO_GROUND);
        let shimmering = ctx.data8(inventory::SHIMMERS + variant as u32);
        o.set_u8(ctx.b, field::SHIMMER, shimmering);
        let index = ctx.data8(inventory::INDICES + variant as u32);
        o.set_u8(ctx.b, field::INDEX, index);
        shimmer(ctx, o);
    }
    animate_and_sort(ctx, inventory::ANIMATIONS, variant);
}

/// Substate 6: the refused object is thrown off in an arc toward the outside of the screen then falls back.
pub(super) fn inventory_reject<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let launches = o.u8(ctx.b, field::SUB_SUB_STATE) != 0;
    if !launches {
        // wait for the delay, then throw
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
    let target = crate::families::effects::target(ctx, inventory::TABLE_SUB_STATES, sub_state as u32);
    match sub_state {
        sub::FALL => {
            let in_flight = ctx.move_xyz().gt();
            if !in_flight {
                add_u8(ctx, o, field::SUB_STATE, 2);
                o.set_u16(ctx.b, field::DURATION, inventory::DURATION_TO_GROUND);
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
            let ready = player.u8(ctx.b, 0x33) != 0;
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
        _ => crate::families::effects::outside_table(target),
    }
}

pub(super) fn inventory_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    let target = crate::families::effects::target(ctx, inventory::TABLE_STATES, state as u32);
    match state {
        state::INIT => inventory_init(ctx, o),
        state::ACTIVE => inventory_active(ctx, o),
        state::FREED | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
    }
}
