use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::vars;
use crate::actors::families::shell::records::view;
use crate::actors::families::shell::tables::{tables, Read};

// ---------------------------------------------------------------------------------------------
// Type 0: treasures (0xdabca)

pub const ANIMATIONS: RomAddr = RomAddr(0x1bed1a);
pub const DURATION_TO_GROUND: u16 = 0x4b0;
pub const SCORE: u32 = 0x56;
pub const SOUND: u32 = 0x57;

/// `0xdad8e`: copies the variant's parameters.
pub(super) fn treasure_params<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let base = (o.u8(ctx.b, Obj::VARIANT) as u32) << 3;
    for (i, off) in [field::SHIMMER, 0x55, SCORE, SOUND, field::INDEX].into_iter().enumerate() {
        let v = tables().item_treasure_params.at8(ctx.b, base + i as u32);
        o.set_u8(ctx.b, off, v);
    }
}

pub(super) fn treasure_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if airborne(ctx, o) {
        state_next(ctx, o, sub::FALL);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        treasure_params(ctx, o);
    } else {
        state_next(ctx, o, sub::WAIT);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        o.set_u16(ctx.b, field::DURATION, DURATION_TO_GROUND);
        treasure_params(ctx, o);
        shimmer(ctx, o);
    }
    animate_and_sort(ctx, ANIMATIONS.0, variant);
}

pub(super) fn treasure_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    match sub_state {
        sub::FALL => {
            let in_flight = ctx.move_xyz().gt();
            if !in_flight {
                // placed: wait on the ground
                add_u8(ctx, o, field::SUB_STATE, 2);
                o.set_u16(ctx.b, field::DURATION, DURATION_TO_GROUND);
                o.set_u32(ctx.b, Obj::Z, 0);
                in_view(ctx, o);
                shimmer(ctx, o);
            }
            fall_end(ctx);
        }
        sub::WAIT => {
            // during a cutscene, an object left at the left of the screen disappears
            let cutscene = vars::INPUTS_LOCKED.get(ctx.b) != 0;
            if cutscene {
                let limit = crate::world::ram::CAMERA_X.get(ctx.b).wrapping_sub(0x40);
                let x = o.u16(ctx.b, Obj::X);
                if !((limit as i16) < x as i16) {
                    o.set_u8(ctx.b, Obj::STATE, state::OUTSIDE_SCREEN);
                    return;
                }
            }
            let pickup = o.u8(ctx.b, field::PICKUP);
            if !(pickup == 0) {
                add_u8(ctx, o, field::SUB_STATE, 2);
            }
            expire_and_blinks(ctx, o);
        }
        sub::PICKUP => {
            if !pickup_ready(ctx, o) {
                return;
            }
            add_u8(ctx, o, Obj::STATE, 2);
            let sound = o.u8(ctx.b, SOUND);
            effect_pickup(ctx, o, sound);
            let player_bits = o.u16(ctx.b, Treasure::PLAYER_BITS) & 0x300;
            let event = (o.u8(ctx.b, SCORE) as u16).wrapping_add(player_bits);
            crate::world::events::enqueue_points(ctx.b, event);
            mark_pickup(ctx, o);
        }
        _ => crate::actors::families::effects::bad_index(sub_state as u32),
    }
}

pub(super) fn treasure<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    match state {
        state::INIT => treasure_init(ctx, o),
        state::ACTIVE => treasure_active(ctx, o),
        state::FREED | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::actors::families::effects::bad_index(state as u32),
    }
}

view! {
    /// Fields of the treasure.
    Treasure {
        /// Bits of the players (0x300) (h).
        player_bits, set_player_bits, PLAYER_BITS: u16 @ 0x48;
    }
}
