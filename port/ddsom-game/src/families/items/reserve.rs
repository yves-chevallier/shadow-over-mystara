use super::*;
use crate::data::RomAddr;
use crate::families::shell::records::Base;
use crate::families::shell::tables::{tables, Read};

// ---------------------------------------------------------------------------------------------
// Type 7: player reserve (0xdce6e)

pub const ANIMATIONS: RomAddr = RomAddr(0x1c001e);
pub const DURATION_TO_GROUND: u16 = 0x4b0;
pub const COUNTERS: u32 = 0x150;

pub(super) fn reserve_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let mode = o.u8(ctx.b, field::MODE);
    if mode != 0 {
        let delay = o.u8(ctx.b, inventory::DELAY);
        if !(delay == 0) {
            o.set_u8(ctx.b, inventory::DELAY, delay - 1);
            return;
        }
        add_u8(ctx, o, Obj::STATE, 2);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        return animate_and_sort(ctx, ANIMATIONS.0, variant.wrapping_add(0xa));
    }
    if airborne(ctx, o) {
        state_next(ctx, o, sub::FALL);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
    } else {
        state_next(ctx, o, sub::WAIT);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        o.set_u16(ctx.b, field::DURATION, DURATION_TO_GROUND);
    }
    animate_and_sort(ctx, ANIMATIONS.0, variant);
}

/// Wait substate: the player who touches the object (`PICKUP`) makes it go to the pickup.
pub(super) fn reserve_wait<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let pickup = o.u8(ctx.b, field::PICKUP);
    if !(pickup == 0) {
        add_u8(ctx, o, field::SUB_STATE, 2);
    }
    expire_and_blinks(ctx, o);
}

/// `0xdd0da`: adds the object to the owner's reserve. True if the pickup is refused.
pub(super) fn reserve_add<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> bool {
    let player = pointer(ctx, o, field::OWNER);
    let class = (player.u8(ctx.b, Obj::VARIANT) >> 1) as u32;
    let accepted = tables().item_reserve_classes.at8(ctx.b, class);
    if !(accepted != 0) {
        o.set_u8(ctx.b, field::SUB_STATE, 6);
        o.set_u8(ctx.b, field::SUB_SUB_STATE, 0);
        o.set_u8(ctx.b, inventory::DELAY, 0xc);
        return true;
    }
    let variant = o.u8(ctx.b, Obj::VARIANT) as u32;
    let counter = player.0 + COUNTERS + variant;
    let n = crate::mem::rd8(ctx.b, counter).wrapping_add(1);
    crate::mem::wr8(ctx.b, counter, n);
    if !(n <= 9) {
        crate::mem::wr8(ctx.b, counter, 9);
    }
    false
}

pub(super) fn reserve_pickup<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let player = pointer(ctx, o, field::OWNER);
    let pickup = o.u8(ctx.b, field::PICKUP);
    if !(pickup == 2) {
        let ready = player.u8(ctx.b, Base::ANIM_EVENT) != 0;
        if !ready {
            return;
        }
    }
    let character = player.u8(ctx.b, Obj::VARIANT) as u32;
    let allowed = tables().item_reserve_characters.at8(ctx.b, character);
    if !(allowed != 0) {
        add_u8(ctx, o, field::SUB_STATE, 0xfe);
        o.set_u8(ctx.b, field::PICKUP, 0);
        return;
    }
    let refuse = reserve_add(ctx, o);
    if !refuse {
        add_u8(ctx, o, Obj::STATE, 2);
    }
}

pub(super) fn reserve_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    match sub_state {
        sub::FALL => {
            let pose = o.u8(ctx.b, field::SUB_SUB_STATE);
            if !(pose != 0) {
                let in_flight = ctx.move_xyz().gt();
                if !in_flight {
                    add_u8(ctx, o, field::SUB_SUB_STATE, 2);
                    o.set_u16(ctx.b, field::DURATION, DURATION_TO_GROUND);
                    o.set_u32(ctx.b, Obj::Z, 0);
                    let mode = o.u8(ctx.b, field::MODE);
                    if !(mode != 0) {
                        in_view(ctx, o);
                    }
                }
                return fall_end(ctx);
            }
            let mode = o.u8(ctx.b, field::MODE);
            if !(mode == 0) {
                add_u8(ctx, o, field::SUB_STATE, 2);
                return reserve_wait(ctx, o);
            }
            let end = ctx.anim_step();
            if end {
                add_u8(ctx, o, field::SUB_STATE, 2);
            }
        }
        sub::WAIT => reserve_wait(ctx, o),
        sub::PICKUP => reserve_pickup(ctx, o),
        6 => inventory_reject(ctx, o),
        _ => crate::families::effects::bad_index(sub_state as u32),
    }
}

pub(super) fn reserve_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    match state {
        state::INIT => reserve_init(ctx, o),
        state::ACTIVE => reserve_active(ctx, o),
        state::FREED | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::bad_index(state as u32),
    }
}
