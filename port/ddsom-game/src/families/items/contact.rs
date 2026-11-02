use super::*;

// ---------------------------------------------------------------------------------------------
// Type 8: contact objects (0xdd132)

/// Type 8, objects picked up by simple contact (box test with the four players, `0xdd470`) that trigger a scene effect
/// through a SET object (`0xc8312`): three variants.
///
/// - 0: placed bonus that waits for `+0x58` (touched by a player), creates a SET (`+0x42`) and follows it;
/// - 1: bonus that oscillates horizontally (speed `+0x14`, about-turn every 0x3c frames) until a
///   player touches it;
/// - 2: same principle as 0, without oscillation.
///
/// `A5-0x5ac4` is non-zero as long as a player is picking one up: only one at a time.
pub const ENTRY: u32 = 0xdd132;
pub const TABLE_VARIANTS: u32 = 0xdd142;
pub const ANIMATIONS: u32 = 0x1bf8fe;
// variant 0
pub const V0_STATES: u32 = 0xdd156;
pub const V0_SUB_STATES: u32 = 0xdd190;
pub const V0_ANIMATIONS: u32 = 0xdd180;
pub const V0_ANIMATIONS_KEY: u32 = 0xdd1d2;
pub const V0_ANIMATIONS_SET: u32 = 0xdd22a;
pub const V0_ANIMATIONS_END: u32 = 0xdd246;
// variant 1
pub const V1_STATES: u32 = 0xdd280;
pub const V1_SUB_STATES: u32 = 0xdd2d4;
// variant 2
pub const V2_STATES: u32 = 0xdd3ca;
pub const V2_SUB_STATES: u32 = 0xdd400;
/// Lock: a player is picking up (A5-0x5ac4) and the event set in `A5+0x5e`.
pub const BUSY: i32 = -0x5ac4;
pub const SET_CREATED: u32 = 0x42;
pub const DELAY: u32 = 0x4c;
/// Contact boxes: one per type (`+0x39`) for the object, one per character and per `+0x38` for the player.
pub const BOXES_OBJECT: u32 = 0xdd590;
pub const BOXES_PLAYER: u32 = 0xdd578;
pub const PLAYERS: u32 = 0xff85de;

/// `0xdd470`: does a player touch the object? If so, the substate advances, the lock is lifted and the player becomes
/// the owner.
pub(super) fn contact_player<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    let busy = ctx.ram8(contact::BUSY) != 0;
    if busy {
        return false;
    }
    let hitbox = o.u8(ctx.b, 0x39) as u32;
    if hitbox == 0 {
        return false;
    }
    let my_box = contact::BOXES_OBJECT + (hitbox << 3);
    for i in 0..4u32 {
        let player = Obj(contact::PLAYERS + i * 0x200);
        if player_key(ctx, o, player, my_box) {
            add_u8(ctx, o, field::SUB_STATE, 2);
            ctx.set_ram8(contact::BUSY, 1);
            o.set_u16(ctx.b, field::OWNER, player.0 as u16);
            return true;
        }
    }
    false
}

/// Is the player present, alive, free, and does its box overlap the object's?
pub(super) fn player_key<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, player: Obj, my_box: u32) -> bool {
    let absent = player.u8(ctx.b, Obj::ACTIVE) != 1;
    if absent {
        return false;
    }
    let dead = player.u8(ctx.b, Obj::STATE) as i8 >= 4;
    if dead {
        return false;
    }
    let busy = player.u8(ctx.b, 0x182) != 0;
    if busy {
        return false;
    }
    let number = player.u8(ctx.b, 0x38) as u32;
    if number == 0 {
        return false;
    }
    let class = (player.u8(ctx.b, 3) >> 1) as u32;
    let table = ctx.data32(contact::BOXES_PLAYER + (class << 2));
    let its_box = table + (number << 3);
    overlaps(ctx, o, player, my_box, its_box)
}

/// The three box overlaps (X, Y then Z) between the object `o` (box `a`) and the player (box `b`).
pub(super) fn overlaps<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, player: Obj, a: u32, b: u32) -> bool {
    let byte = |ctx: &mut Cx<'_, R>, address: u32| ctx.data8(address) as i8 as i16 as u16;
    // X: the offsets invert with the orientation of each
    let mut da = byte(ctx, a);
    let turned = o.u8(ctx.b, field::FLIP) != 0;
    if turned {
        da = da.wrapping_neg();
    }
    let mut db = byte(ctx, b);
    let player_turned = player.u8(ctx.b, field::FLIP) != 0;
    if player_turned {
        db = db.wrapping_neg();
    }
    let gap = da.wrapping_add(o.u16(ctx.b, Obj::X)).wrapping_sub(db).wrapping_sub(player.u16(ctx.b, Obj::X));
    let half = ctx.data8(a + 1).wrapping_add(ctx.data8(b + 1)) as u16;
    if gap.wrapping_add(half) > half.wrapping_mul(2) {
        return false;
    }
    // Y then Z: fixed offsets
    for (offset, size, position) in [(4u32, 5u32, Obj::Y), (2, 3, Obj::Z)] {
        let da = byte(ctx, a + offset);
        let db = byte(ctx, b + offset);
        let gap = da.wrapping_add(o.u16(ctx.b, position)).wrapping_sub(db.wrapping_add(player.u16(ctx.b, position)));
        let half = ctx.data8(a + size).wrapping_add(ctx.data8(b + size)) as u16;
        if gap.wrapping_add(half) > half.wrapping_mul(2) {
            return false;
        }
    }
    true
}

/// `0xdd16e`: starts animation `animation`, then sorts the object.
pub(super) fn contact_animate<R: Bench>(ctx: &mut Cx<'_, R>, animation: u8) {
    ctx.anim_start(contact::ANIMATIONS, animation as u16);
    ctx.reinsert();
}

/// Animation drawn from a table indexed by the mode (`+0x2f`).
pub(super) fn contact_animate_mode<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, table: u32) {
    let mode = o.u8(ctx.b, field::MODE) as u32;
    let animation = ctx.prog8(table + mode);
    contact_animate(ctx, animation);
}

/// Creates the SET object of the pickup scene (type `+0x42`, word) and attaches it to the object. False if the type is
/// negative (no scene) or if the pool is full.
pub(super) fn contact_created_set<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    let word = o.u16(ctx.b, contact::SET_CREATED);
    if word as i16 <= -1 {
        return false;
    }
    let Some(new) = scene_set_of(ctx, word as u8) else {
        return false;
    };
    o.set_u16(ctx.b, contact::SET_CREATED, new.0 as u16);
    let owner = pointer(ctx, o, field::OWNER);
    let character = owner.u8(ctx.b, 2);
    new.set_u8(ctx.b, 0x37, character);
    true
}

/// Orients the object toward the side where the owner is (carry of `x_owner - x`).
pub(super) fn contact_facing<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let owner = pointer(ctx, o, field::OWNER);
    let (x_owner, x) = (owner.u16(ctx.b, Obj::X), o.u16(ctx.b, Obj::X));
    let flip = if x_owner < x { 0 } else { 0x20 };
    o.set_u8(ctx.b, field::FLIP, flip);
}

/// Has the SET object finished? (`+0x22` of the SET object non-zero.)
pub(super) fn contact_set_finished<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    let set = pointer(ctx, o, contact::SET_CREATED);
    set.u8(ctx.b, 0x22) != 0
}

/// Variant 0, end of the scene (`0xdd22e`): lifts the lock and goes to the flee substate.
pub(super) fn contact_0_end<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ctx.set_ram8(contact::BUSY, 0);
    o.set_u8(ctx.b, field::SUB_STATE, 6);
    contact_animate_mode(ctx, o, contact::V0_ANIMATIONS_END);
}

pub(super) fn shifted_x<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, step: u16) {
    let x = o.u16(ctx.b, Obj::X).wrapping_add(step);
    o.set_u16(ctx.b, Obj::X, x);
}

pub(super) fn contact_0<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    let target = crate::families::effects::target(ctx, contact::V0_STATES, state as u32);
    match state {
        state::INIT => {
            add_u8(ctx, o, Obj::STATE, 2);
            let mode = o.u8(ctx.b, field::MODE) as u32;
            let animation = ctx.prog8(contact::V0_ANIMATIONS + mode);
            o.set_u8(ctx.b, field::SUB_STATE, 0);
            contact_animate(ctx, animation);
        }
        state::ACTIVE => {
            let sub_state = o.u8(ctx.b, field::SUB_STATE);
            let target = crate::families::effects::target(ctx, contact::V0_SUB_STATES, sub_state as u32);
            match sub_state {
                0 => {
                    ctx.anim_step();
                    let pickup = o.u8(ctx.b, field::PICKUP);
                    if pickup == 0 {
                        return;
                    }
                    let busy = ctx.ram8(contact::BUSY) != 0;
                    if busy {
                        o.set_u8(ctx.b, field::PICKUP, 0);
                        return;
                    }
                    ctx.set_ram8(contact::BUSY, 1);
                    add_u8(ctx, o, field::SUB_STATE, 2);
                    shard(ctx, o);
                    contact_animate_mode(ctx, o, contact::V0_ANIMATIONS_KEY);
                }
                2 => {
                    ctx.anim_step();
                    let key = o.u8(ctx.b, 0x33);
                    if key == 1 {
                        return;
                    }
                    add_u8(ctx, o, field::SUB_STATE, 2);
                    if !contact_created_set(ctx, o) {
                        return contact_0_end(ctx, o);
                    }
                    contact_facing(ctx, o);
                    contact_animate_mode(ctx, o, contact::V0_ANIMATIONS_SET);
                }
                4 => {
                    if contact_set_finished(ctx, o) {
                        contact_0_end(ctx, o);
                    }
                }
                6 => {
                    let flip = o.u8(ctx.b, field::FLIP) != 0;
                    let step: u16 = if flip { 3 } else { 3u16.wrapping_neg() };
                    shifted_x(ctx, o, step);
                    ctx.anim_step();
                    ctx.cull_offscreen();
                }
                _ => crate::families::effects::outside_table(target),
            }
        }
        state::FREED | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
    }
}

/// Variant 1: swinging animation (`0xdd2a8`), sort and edge adjustment.
pub(super) fn contact_1_animate<R: Bench>(ctx: &mut Cx<'_, R>) {
    contact_animate(ctx, 0x18);
    ctx.keep_in_zone();
    ctx.push_terrain();
}

/// Variant 1: horizontal movement `X += VX << 8`.
pub(super) fn contact_1_advance<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let vx = o.u16(ctx.b, Obj::VX) as i16 as i32;
    let x = o.u32(ctx.b, Obj::X).wrapping_add((vx << 8) as u32);
    o.set_u32(ctx.b, Obj::X, x);
}

/// Variant 1: speed 0x200 in the direction of the orientation (`0xdd296`).
pub(super) fn contact_1_speed<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    o.set_u16(ctx.b, Obj::VX, 0x200);
    let flip = o.u8(ctx.b, field::FLIP) != 0;
    if !flip {
        let v = o.u16(ctx.b, Obj::VX).wrapping_neg();
        o.set_u16(ctx.b, Obj::VX, v);
    }
}

/// Variant 1, scene exit (`0xdd37c`): the lock is lifted, the bonus moves away.
pub(super) fn contact_1_exit<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    contact_1_speed(ctx, o);
    ctx.set_ram8(contact::BUSY, 0);
    o.set_u8(ctx.b, field::SUB_STATE, 6);
    contact_1_animate(ctx);
}

pub(super) fn contact_1<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    let target = crate::families::effects::target(ctx, contact::V1_STATES, state as u32);
    match state {
        state::INIT => {
            add_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, field::SUB_STATE, 0);
            o.set_u8(ctx.b, contact::DELAY, 0x3c);
            contact_1_speed(ctx, o);
            contact_1_animate(ctx);
        }
        state::ACTIVE => {
            let sub_state = o.u8(ctx.b, field::SUB_STATE);
            let target = crate::families::effects::target(ctx, contact::V1_SUB_STATES, sub_state as u32);
            match sub_state {
                0 => {
                    let key = contact_player(ctx, o);
                    if key {
                        // no next contact: the substates advance by themselves
                        ctx.set_ram8(0x5e, 1);
                        ctx.anim_start(contact::ANIMATIONS, 0x19);
                        return;
                    }
                    contact_1_advance(ctx, o);
                    let remainder = o.u8(ctx.b, contact::DELAY).wrapping_sub(1);
                    o.set_u8(ctx.b, contact::DELAY, remainder);
                    if !(remainder != 0) {
                        o.set_u8(ctx.b, contact::DELAY, 0x3c);
                        let flip = o.u8(ctx.b, field::FLIP) ^ 0x20;
                        o.set_u8(ctx.b, field::FLIP, flip);
                        let v = o.u16(ctx.b, Obj::VX).wrapping_neg();
                        o.set_u16(ctx.b, Obj::VX, v);
                    }
                    ctx.anim_step();
                    ctx.reinsert();
                    ctx.keep_in_zone();
                    ctx.push_terrain();
                }
                2 => {
                    add_u8(ctx, o, field::SUB_STATE, 2);
                    if !contact_created_set(ctx, o) {
                        return contact_1_exit(ctx, o);
                    }
                    contact_facing(ctx, o);
                }
                4 => {
                    let set = pointer(ctx, o, contact::SET_CREATED);
                    let in_progress = set.u8(ctx.b, 0x22) == 0;
                    if !in_progress {
                        contact_1_exit(ctx, o);
                    }
                }
                6 => {
                    contact_1_advance(ctx, o);
                    ctx.anim_step();
                    ctx.reinsert();
                    ctx.cull_offscreen();
                }
                _ => crate::families::effects::outside_table(target),
            }
        }
        state::FREED | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
    }
}

/// Variant 2, end of the scene (`0xdd44e`).
pub(super) fn contact_2_end<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ctx.set_ram8(contact::BUSY, 0);
    add_u8(ctx, o, field::SUB_STATE, 2);
}

pub(super) fn contact_2<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    let target = crate::families::effects::target(ctx, contact::V2_STATES, state as u32);
    match state {
        state::INIT => {
            add_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, field::SUB_STATE, 0);
            let mode = o.u8(ctx.b, field::MODE);
            contact_animate(ctx, mode.wrapping_add(0x14));
        }
        state::ACTIVE => {
            let sub_state = o.u8(ctx.b, field::SUB_STATE);
            let target = crate::families::effects::target(ctx, contact::V2_SUB_STATES, sub_state as u32);
            match sub_state {
                0 => {
                    ctx.anim_step();
                    let pickup = o.u8(ctx.b, field::PICKUP);
                    if pickup == 0 {
                        return;
                    }
                    let busy = ctx.ram8(contact::BUSY) != 0;
                    if busy {
                        o.set_u8(ctx.b, field::PICKUP, 0);
                        return;
                    }
                    ctx.set_ram8(contact::BUSY, 1);
                    add_u8(ctx, o, field::SUB_STATE, 2);
                    if !contact_created_set(ctx, o) {
                        contact_2_end(ctx, o);
                    }
                }
                2 => {
                    if contact_set_finished(ctx, o) {
                        contact_2_end(ctx, o);
                    }
                }
                4 => {
                    ctx.anim_step();
                }
                _ => crate::families::effects::outside_table(target),
            }
        }
        state::FREED | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::outside_table(target),
    }
}

pub(super) fn contact_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let target = crate::families::effects::target(ctx, contact::TABLE_VARIANTS, 2 * variant as u32);
    match variant {
        0 => contact_0(ctx, o),
        1 => contact_1(ctx, o),
        2 => contact_2(ctx, o),
        _ => crate::families::effects::outside_table(target),
    }
}
