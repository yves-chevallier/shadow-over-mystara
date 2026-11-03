use crate::player::Player;
use super::*;
use crate::data::RomAddr;
use crate::families::shell::vars;
use crate::families::shell::tables::{tables, Read};

// ---------------------------------------------------------------------------------------------
// Type 1: food and potions (0xdae26)

pub const ANIMATIONS: RomAddr = RomAddr(0x1bee16);
pub const DURATION_INITIAL: u16 = 0x960;
pub const DURATION_AFTER_FALL: u16 = 0x708;

pub(super) fn consumable_params<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let base = ((o.u8(ctx.b, Obj::VARIANT) as u32) * 2) & 0xff;
    let shimmering = tables().item_consumable_params.at8(ctx.b, base);
    o.set_u8(ctx.b, field::SHIMMER, shimmering);
    let index = tables().item_consumable_params.at8(ctx.b, base + 1);
    o.set_u8(ctx.b, field::INDEX, index);
}

pub(super) fn consumable_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if airborne(ctx, o) {
        state_next(ctx, o, sub::FALL);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        o.set_u16(ctx.b, field::DURATION, DURATION_INITIAL);
        consumable_params(ctx, o);
    } else {
        state_next(ctx, o, sub::WAIT);
        o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
        o.set_u16(ctx.b, field::DURATION, DURATION_INITIAL);
        consumable_params(ctx, o);
        shimmer(ctx, o);
    }
    animate_and_sort(ctx, ANIMATIONS.0, variant);
}

/// Effect of a food: heals the player; at full health, score event and sound.
pub(super) fn effect_food<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let player = pointer(ctx, o, field::OWNER);
    let (life_max, life) = (player.u16(ctx.b, Player::HP_MAX), player.u16(ctx.b, Player::HP));
    if !(life_max != life) {
        let event = 0x11 + ((player.u8(ctx.b, Obj::KIND) as u16) << 8);
        crate::events::enqueue_points(ctx.b, event);
        effect_pickup(ctx, o, 5);
        return;
    }
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let heal: u16 = if variant == 0 { 0x14 } else { 0x28 };
    let life = life.wrapping_add(heal);
    player.set_u16(ctx.b, Player::HP, life);
    if !(life <= life_max) {
        player.set_u16(ctx.b, Player::HP, life_max);
    }
    crate::sound::play(ctx.b, call::END_EFFECT, ctx.obj());
}

/// Potions: the player receives a countdown of 0x2d0 frames and flags +0x180..0x184.
pub(super) fn effect_potion<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, bytes: &[(u32, u8)]) {
    let player = pointer(ctx, o, field::OWNER);
    player.set_u16(ctx.b, Player::AURA_TIMER, 0x2d0);
    for &(off, v) in bytes {
        player.set_u8(ctx.b, off, v);
    }
    crate::sound::play(ctx.b, call::END_EFFECT, ctx.obj());
}

pub(super) fn consumable_effect<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT) as u32;
    // the effect routine of the variant (table of routines 0xdafb8: food 0xdafd0 twice, potions 0xdb044 and 0xdb026,
    // nothing 0xdb068, counter 0xdb06a)
    match variant {
        0 | 1 => effect_food(ctx, o),
        2 => effect_potion(ctx, o, &[(1, 1), (0x183, 1), (0x184, 0), (0x182, 0)]),
        3 => effect_potion(ctx, o, &[(0x184, 1), (0x183, 0), (0x182, 0)]),
        4 => {}
        5 => {
            let n = vars::KEYS.get(ctx.b).wrapping_add(1);
            vars::KEYS.set(ctx.b, n);
            if !(n <= 9) {
                vars::KEYS.set(ctx.b, 9);
            }
        }
        _ => crate::families::effects::bad_index(variant),
    }
    mark_pickup(ctx, o);
}

pub(super) fn consumable_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    match sub_state {
        sub::FALL => {
            let in_flight = ctx.move_xyz().gt();
            if !in_flight {
                add_u8(ctx, o, field::SUB_STATE, 2);
                o.set_u16(ctx.b, field::DURATION, DURATION_AFTER_FALL);
                o.set_u32(ctx.b, Obj::Z, 0);
                in_view(ctx, o);
                shimmer(ctx, o);
            }
            fall_end(ctx);
        }
        sub::WAIT => {
            let pickup = o.u8(ctx.b, field::PICKUP);
            if !(pickup == 0) {
                add_u8(ctx, o, field::SUB_STATE, 2);
            }
            // only variants 2 and following disappear
            let variant = o.u8(ctx.b, Obj::VARIANT);
            if (variant == 0) || (variant == 1) {
                return;
            }
            expire_and_blinks(ctx, o);
        }
        sub::PICKUP => {
            if !pickup_ready(ctx, o) {
                return;
            }
            add_u8(ctx, o, Obj::STATE, 2);
            consumable_effect(ctx, o);
        }
        _ => crate::families::effects::bad_index(sub_state as u32),
    }
}

pub(super) fn consumable_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    match state {
        state::INIT => consumable_init(ctx, o),
        state::ACTIVE => consumable_active(ctx, o),
        state::FREED | state::OUTSIDE_SCREEN => ctx.free(&pools::ITEM),
        _ => crate::families::effects::bad_index(state as u32),
    }
}
