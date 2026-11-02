use super::*;
use crate::data::RomAddr;
use crate::families::shell::records::view;
use crate::families::shell::tables::{tables, Read};

// ---------------------------------------------------------------------------------------------
// Type 5: animated bonuses (0xdc382)

pub const ANIMATIONS: RomAddr = RomAddr(0x1bf5c2);
pub const DURATION_TO_GROUND: u16 = 0x4b0;
/// Gap between the falling animation and the ground animation, and ground animation of a placed object.
pub const ANIMATION_GROUND: u8 = 6;
pub const ANIMATION_POSE: u8 = 0xc;

pub(super) fn bonus_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if airborne(ctx, o) {
        state_next(ctx, o, sub::FALL);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        let mode = o.u8(ctx.b, field::MODE);
        if !(mode == 0) {
            let base = (mode as u32) << 3;
            for (i, off) in [Obj::VX, Obj::VZ, Obj::AZ, Obj::VY].into_iter().enumerate() {
                let v = tables().item_bonus_speeds.at16(ctx.b, base + 2 * i as u32);
                o.set_u16(ctx.b, off, v);
            }
        }
        animate_and_sort(ctx, ANIMATIONS.0, variant);
    } else {
        state_next(ctx, o, sub::WAIT);
        o.set_u16(ctx.b, field::DURATION, DURATION_TO_GROUND);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        animate_and_sort(ctx, ANIMATIONS.0, variant.wrapping_add(ANIMATION_POSE));
    }
}

pub(super) fn bonus_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    match sub_state {
        sub::FALL => {
            ctx.anim_step();
            let in_flight = ctx.move_xyz().gt();
            if !in_flight {
                add_u8(ctx, o, field::SUB_STATE, 2);
                o.set_u16(ctx.b, field::DURATION, DURATION_TO_GROUND);
                o.set_u32(ctx.b, Obj::Z, 0);
                let variant = o.u8(ctx.b, Obj::VARIANT);
                ctx.anim_start(ANIMATIONS.0, variant.wrapping_add(ANIMATION_GROUND) as u16);
                in_view(ctx, o);
            }
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
            if !pickup_ready(ctx, o) {
                return;
            }
            add_u8(ctx, o, Obj::STATE, 2);
            let player_bits = o.u16(ctx.b, Bonus::PLAYER_BITS) & 0x300;
            let event = (o.u8(ctx.b, Obj::VARIANT) as u16).wrapping_add(player_bits);
            crate::events::enqueue_counter(ctx.b, event);
        }
        _ => crate::families::effects::bad_index(sub_state as u32),
    }
}

pub(super) fn bonus_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    match state {
        state::INIT => bonus_init(ctx, o),
        state::ACTIVE => bonus_active(ctx, o),
        state::FREED | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::bad_index(state as u32),
    }
}

view! {
    /// Fields of the animated bonus.
    Bonus {
        /// Bits of the players (0x300) (h).
        player_bits, set_player_bits, PLAYER_BITS: u16 @ 0x48;
    }
}
