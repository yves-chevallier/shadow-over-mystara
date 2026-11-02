use super::*;
use crate::families::shell::tables::{tables, Read};

// ---------------------------------------------------------------------------------------------
// Type 0: treasures (0xdabca)

/// Type 0, "treasure": coins and gems picked up for points. Variants 0 to 5; an eight-byte table per variant gives
/// sparkle (+0x54), a specific byte (+0x55), score event (+0x56), sound (+0x57) and pickup index (+0x5d).
///
/// States: 0 init (in the air: fall; on the ground: wait 0x4b0 frames + sparkle), 2 active (fall, wait with blinking,
/// picked up: sound + score), 4/6 release.
pub const ENTRY: u32 = 0xdabca;
pub const ANIMATIONS: u32 = 0x1bed1a;
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
    animate_and_sort(ctx, ANIMATIONS, variant);
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
            let cutscene = ctx.ram8(0x44) != 0;
            if cutscene {
                let limit = crate::ram::CAMERA_X.get(ctx.b).wrapping_sub(0x40);
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
            let player_bits = o.u16(ctx.b, 0x48) & 0x300;
            let event = (o.u8(ctx.b, SCORE) as u16).wrapping_add(player_bits);
            crate::events::enqueue_points(ctx.b, event);
            mark_pickup(ctx, o);
        }
        _ => crate::families::effects::bad_index(sub_state as u32),
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
        _ => crate::families::effects::bad_index(state as u32),
    }
}
