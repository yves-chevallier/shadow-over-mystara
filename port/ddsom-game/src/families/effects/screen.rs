use crate::player::Player;
use super::*;

// ---------------------------------------------------------------------------------------------
// Type 21: sliding interface banner (0xaf360)

/// Type 21: interface banner (variant 0: at the bottom, 1: at the top) that slides in `Z` by one pixel per frame (as
/// long as `A5+0x6f` bit 0 is zero) up to the target height `+0x18`, then frees itself by decrementing the byte pointed
/// to by `+0x58` (banner counter of the creator).
pub(super) mod banner {
    pub const TARGET: u32 = 0x18;
    pub const CREATOR: u32 = 0x58;
}

pub(super) fn banner_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    o.set_u32(ctx.b, Obj::X, 0);
    o.set_u32(ctx.b, Obj::Y, 0);
    o.set_u16(ctx.b, Obj::Z, 0xc8);
    o.set_u16(ctx.b, banner::TARGET, 0x100);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if !(variant == 0) {
        o.set_u16(ctx.b, Obj::Z, 0x88);
        o.set_u16(ctx.b, banner::TARGET, 0x50);
    }
    ctx.anim_start(ANIMATIONS_INTERFACE.0, 0xec);
    ctx.reinsert();
}

/// End of the banner: decrements the creator's counter and frees the object.
pub(super) fn banner_end<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let counter = crate::mem::rd32(ctx.b, o.0 + banner::CREATOR);
    let v = crate::mem::rd8(ctx.b, counter).wrapping_sub(1);
    crate::mem::wr8(ctx.b, counter, v);
    ctx.free(&pools::EFFCT);
}

pub(super) fn banner_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let pause = vars::FRAME_BYTE.get(ctx.b) & 1 != 0;
    if !pause {
        let variant = o.u8(ctx.b, Obj::VARIANT);
        let step: u16 = if variant == 0 { 1 } else { 1u16.wrapping_neg() };
        shifted_u16(ctx, o, Obj::Z, step);
        let target = o.u16(ctx.b, banner::TARGET);
        let z = o.u16(ctx.b, Obj::Z);
        if target == z {
            return banner_end(ctx, o);
        }
    }
    ctx.reinsert();
}

pub(super) fn banner<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    match state {
        state::INIT => banner_init(ctx, o),
        state::ACTIVE => banner_active(ctx, o),
        state::FREED => banner_end(ctx, o),
        state::DESTROYED => ctx.free(&pools::EFFCT),
        _ => bad_index(state as u32),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 9: trail that crosses the screen (0xabbee)

/// Type 9: object born at a screen position drawn from a table (according to `+0x52`), advances in `X` (speed 0x200),
/// waits 0x120 frames (animation `variant + 0xc`) then plays its exit animation for 0x200 frames, advancing during the
/// last 0x50, before disappearing.
pub(super) mod trail {
    pub const ANIMATIONS: crate::data::RomAddr = crate::data::RomAddr(0x18ea30);
    pub const DELAY: u32 = 0x50;
    pub const POSITION: u32 = 0x52;
}

pub(super) fn trail_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    o.set_u16(ctx.b, trail::DELAY, 0x120);
    o.set_u32(ctx.b, Obj::VX, 0x0200_0000);
    let rank = (o.u8(ctx.b, trail::POSITION) as u32) << 2;
    let x = tables().trail_positions.at16(ctx.b, rank);
    o.set_u16(ctx.b, Obj::X, x);
    let y = tables().trail_positions.at16(ctx.b, rank + 2);
    o.set_u16(ctx.b, Obj::Y, y);
    o.set_u16(ctx.b, Obj::Z, 0);
    let (cam_x, cam_y) = (crate::ram::CAMERA_X.get(ctx.b), crate::ram::CAMERA_Y.get(ctx.b));
    shifted_u16(ctx, o, Obj::X, cam_x);
    shifted_u16(ctx, o, Obj::Y, cam_y);
    let animation = o.u8(ctx.b, Obj::VARIANT).wrapping_add(0xc);
    ctx.anim_start(trail::ANIMATIONS.0, animation as u16);
}

pub(super) fn trail_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    if !(sub_state != 0) {
        let remainder = o.u16(ctx.b, trail::DELAY).wrapping_sub(1);
        o.set_u16(ctx.b, trail::DELAY, remainder);
        if !(remainder != 0) {
            add_u8(ctx, o, field::SUB_STATE, 2);
            o.set_u16(ctx.b, trail::DELAY, 0x200);
            let variant = o.u8(ctx.b, Obj::VARIANT);
            ctx.anim_start(trail::ANIMATIONS.0, variant as u16);
            return;
        }
        if remainder > 0x50 {
            return;
        }
        ctx.anim_step();
        ctx.move_x();
        return ctx.reinsert();
    }
    let remainder = o.u16(ctx.b, trail::DELAY).wrapping_sub(1);
    o.set_u16(ctx.b, trail::DELAY, remainder);
    if !(remainder != 0) {
        add_u8(ctx, o, Obj::STATE, 2);
    }
    ctx.anim_step();
    ctx.reinsert();
}

pub(super) fn trail<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, trail_init, trail_active)
}

// ---------------------------------------------------------------------------------------------
// Type 26: cloud that crosses the screen (0xb11f4)

/// Type 26: cloud born at the right of the screen (random height), waits a random delay (`+0x5c`, 0 to 63 frames) then
/// crosses the screen to the left (speed according to the variant) up to 0x40 pixels beyond the edge.
pub(super) mod cloud {
    pub const ANIMATIONS: crate::data::RomAddr = crate::data::RomAddr(0x1bf8fe);
    pub const DELAY: u32 = 0x5c;
}

pub(super) fn cloud_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    let rng = ctx.random();
    let draw = rng & 0x3f;
    o.set_u16(ctx.b, cloud::DELAY, draw);
    let rng = ctx.random();
    let draw = rng & 0x3f;
    o.set_u16(ctx.b, Obj::Y, draw);
    shifted_u16(ctx, o, Obj::Y, 0x50);
    o.set_u16(ctx.b, Obj::Z, 0);
    let cam = crate::ram::CAMERA_ALT_X.get(ctx.b);
    o.set_u16(ctx.b, Obj::X, cam.wrapping_add(0x1c0));
    let variant = o.u8(ctx.b, Obj::VARIANT) as u32;
    let speed = tables().cloud_speeds.at16(ctx.b, 2 * variant);
    o.set_u16(ctx.b, Obj::VX, speed);
    let animation = tables().cloud_animations_number.at8(ctx.b, variant);
    ctx.anim_start(cloud::ANIMATIONS.0, animation as u16);
}

pub(super) fn cloud_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    if !(sub_state != 0) {
        let remainder = o.u16(ctx.b, cloud::DELAY).wrapping_sub(1);
        o.set_u16(ctx.b, cloud::DELAY, remainder);
        if !(remainder as i16 >= 0) {
            add_u8(ctx, o, field::SUB_STATE, 2);
        }
        return;
    }
    ctx.anim_step();
    // the original rereads D0 after the re-listing, which changes only its low word: the high word is that of the state
    // dispatch (`moveq #0,D0`), zero; `swap D0 ; asr.l #8,D0` therefore gives the speed × 256
    ctx.reinsert();
    let o = ctx.obj();
    let vx = o.u16(ctx.b, Obj::VX) as u32;
    let d0 = (vx << 16) as i32 >> 8;
    let x = o.u32(ctx.b, Obj::X).wrapping_sub(d0 as u32);
    o.set_u32(ctx.b, Obj::X, x);
    let limit = crate::ram::CAMERA_ALT_X.get(ctx.b).wrapping_sub(0x40);
    let xw = o.u16(ctx.b, Obj::X);
    if !((limit as i16) < xw as i16) {
        add_u8(ctx, o, Obj::STATE, 2);
    }
}

pub(super) fn cloud<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, cloud_init, cloud_active)
}

// ---------------------------------------------------------------------------------------------
// Type 17: player interface elements (0xae78c)

/// Type 17: game interface elements, five variants (0 to 4), placed relative to the camera. All free themselves when
/// the game phase `A5+0x2c` reaches 8 (end of game).
///
/// - variant 0: waits for `+0x1c` to be non-zero then fills 8 longs of the palette `0x91c0e0`;
/// - variant 1: cursor of player `+0x2f` (player pointer in `+0x1a`), animation according to the
///   character;
/// - variant 2: player window (gauge): copies into the palette, according to the state of the parent
///   object (`+0x1e`), the colors of the window; blinks when the gauge changes;
/// - variant 3: indicator placed at the bottom right, animation `0xe3 - +0x14`;
/// - variant 4: fixed element offset according to the player.
pub(super) mod interface {
    pub const PALETTE: u32 = 0x91c000;
    pub const PALETTE_0: u32 = 0x91c0e0;
    pub const CARRIER: u32 = 0x1e;
    pub const PLAYER: u32 = 0x1a;
    pub const CHARACTER: u32 = 0x42;
    // subroutines not rewritten
    pub const CHANGE_CHARACTER: crate::sound::Sound = crate::sound::CMD_134_234;
    pub const SOUND_GAUGE: crate::sound::Sound = crate::sound::CMD_13B_23B;
}

/// Game phase: 8 or more, the game is over.
pub(super) fn game_finished<R: Bench>(ctx: &mut Cx<'_, R>) -> bool {
    let phase = crate::ram::PHASE.get(ctx.b) as i8;
    phase >= 8
}

/// Places the object on the camera (`X` = camera x, `Z` = camera y, `Y` = 0).
pub(super) fn place_on_camera<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let (cam_x, cam_y) = (crate::ram::CAMERA_X.get(ctx.b), crate::ram::CAMERA_Y.get(ctx.b));
    o.set_u16(ctx.b, Obj::X, cam_x);
    o.set_u16(ctx.b, Obj::Z, cam_y);
    o.set_u32(ctx.b, Obj::Y, 0);
}

pub(super) fn interface_0<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    if game_finished(ctx) {
        return ctx.free(&pools::EFFCT);
    }
    let state = o.u8(ctx.b, Obj::STATE);
    match state {
        state::INIT => {
            let ready = o.u8(ctx.b, Interface::VALUE);
            if ready == 0 {
                return;
            }
            add_u8(ctx, o, Obj::STATE, 2);
            for i in 0..8 {
                crate::mem::wr32(ctx.b, interface::PALETTE_0 + 4 * i, 0xffff_ffff);
            }
        }
        state::ACTIVE => {}
        _ => bad_index(state as u32),
    }
}

pub(super) fn interface_1<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    match state {
        state::INIT => {
            add_u8(ctx, o, Obj::STATE, 2);
            place_on_camera(ctx, o);
            let number = o.u8(ctx.b, field::MODE) as u32;
            for _ in 0..number {
            }
            let player = crate::ram::PLAYERS.nth(number);
            o.set_u16(ctx.b, interface::PLAYER, player.0 as u16);
            let character = player.u8(ctx.b, Player::CHARACTER) & 0x7e;
            o.set_u8(ctx.b, interface::CHARACTER, character);
            ctx.anim_start(ANIMATIONS_INTERFACE.0, 0x8e + number as u16);
        }
        state::ACTIVE => {
            let carrier = pointer(ctx, o, interface::CARRIER);
            let carrier_absent = carrier.u8(ctx.b, Carrier::PRESENT) == 0;
            let player = pointer(ctx, o, interface::PLAYER);
            if carrier_absent || {
                player.u8(ctx.b, Obj::ACTIVE) == 0
            } {
                if game_finished(ctx) {
                    ctx.free(&pools::EFFCT);
                }
                return;
            }
            place_on_camera(ctx, o);
            let character = player.u8(ctx.b, Player::CHARACTER) & 0x7e;
            let mut change = false;
            let known = o.u8(ctx.b, interface::CHARACTER);
            if !(character == known) {
                o.set_u8(ctx.b, interface::CHARACTER, character);
                change = true;
            }
            let rank = (character as u32) << 1;
            let dx = tables().interface_offsets_1.at16(ctx.b, rank);
            let dz = tables().interface_offsets_1.at16(ctx.b, 2 + rank);
            shifted_u16(ctx, o, Obj::X, dx);
            shifted_u16(ctx, o, Obj::Z, dz);
            if change {
                crate::sound::play(ctx.b, interface::CHANGE_CHARACTER, ctx.obj());
            }
            let raise = o.u8(ctx.b, Interface::RAISED) != 0;
            if raise {
                o.set_u16(ctx.b, Interface::SHAPE, 1);
                shifted_u16(ctx, o, Obj::Z, 0x10);
            }
            ctx.anim_step();
            ctx.reinsert();
        }
        state::FREED => ctx.free(&pools::EFFCT),
        state::DESTROYED => ctx.free(&pools::EFFCT),
        _ => bad_index(state as u32),
    }
}

/// `0xaeb08`: copies into the palette (OR of the priority bit 0xF000F000) the two 16-byte blocks of each slot of the
/// player's list; `offset` chooses the source palette (0 or 0x400/0x800).
pub(super) fn copy_palette<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, offset: u16) {
    let player = (o.u8(ctx.b, field::MODE) as u32) << 1 & 0xff;
    let rank = tables().interface_lists.at16(ctx.b, player) as i16 as i32 as u32;
    // the lists follow their offsets, read in data space
    let lists = &tables().interface_list_slots;
    let mut list = rank;
    loop {
        let slot = lists.at16(ctx.b, list);
        list = list.wrapping_add(2);
        if (slot as i16) < 0 {
            return; // bmi: end of list
        }
        let destination = interface::PALETTE.wrapping_add(slot as i16 as i32 as u32);
        let source = slot.wrapping_add(offset) as i16 as i32 as u32;
        for k in 0..8 {
            let v = crate::palette::library_excerpt!("interface_palettes", 0x28ff00, 0x290c00, 2).at32(ctx.b, source.wrapping_add(4 * k)) | 0xf000_f000;
            crate::mem::wr32(ctx.b, destination + 4 * k, v);
        }
    }
}

pub(super) fn interface_2<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    match state {
        state::INIT => {
            add_u8(ctx, o, Obj::STATE, 2);
            place_on_camera(ctx, o);
            o.set_u16(ctx.b, Obj::VX, 0);
            o.set_u16(ctx.b, Obj::VY, 0);
            o.set_u8(ctx.b, Effect::OWN_PALETTE, 0);
            o.set_u8(ctx.b, Effect::PALETTE, 7);
            let number = (o.u8(ctx.b, field::MODE) as u32) << 1;
            let shape = tables().interface_shapes_2.at16(ctx.b, number);
            o.set_u16(ctx.b, Interface::SHAPE, shape);
            let dx = tables().interface_offsets_2.at16(ctx.b, 2 * number);
            let dz = tables().interface_offsets_2.at16(ctx.b, 2 * number + 2);
            shifted_u16(ctx, o, Obj::X, dx);
            shifted_u16(ctx, o, Obj::Z, dz);
            interface_2_animation(ctx, o, 0);
        }
        state::ACTIVE => {
            if game_finished(ctx) {
                return ctx.free(&pools::EFFCT);
            }
            let carrier = pointer(ctx, o, interface::CARRIER);
            let absent = carrier.u8(ctx.b, Carrier::PRESENT) == 0;
            if absent {
                return ctx.reinsert();
            }
            let sub_state = o.u8(ctx.b, field::SUB_STATE);
            if sub_state != 0 {
                return ctx.reinsert();
            }
            let number = o.u8(ctx.b, field::MODE) as u32;
            let word = carrier.u16(ctx.b, Carrier::SLOTS + 2 * number);
            let mut d1: u8 = 0;
            if !(word == 0xffff) {
                d1 = 1;
            }
            let known = o.u8(ctx.b, Interface::SHOWN);
            if !(d1 == known) {
                o.set_u8(ctx.b, Interface::SHOWN, d1);
                // 0xae9f2
                let offset = if d1 == 0 { 0 } else { 0x400 };
                copy_palette(ctx, o, offset);
                // the routine chains onto the animation (0xaea0c), which sorts the object then comes back here
                interface_2_animation(ctx, o, d1 as u16);
            }
            let bits = 3u16 << ((number << 1) & 0x1f);
            let value = carrier.u16(ctx.b, Carrier::GAUGE_BITS) & bits;
            let old = o.u16(ctx.b, Interface::VALUE);
            if !(value == old) {
                o.set_u16(ctx.b, Interface::VALUE, value);
                let blinks = o.u8(ctx.b, Interface::BLINK) != 0;
                if !blinks {
                    o.set_u8(ctx.b, Interface::BLINK, 0xc);
                    crate::sound::play(ctx.b, interface::SOUND_GAUGE, ctx.obj());
                }
            }
            let remainder = o.u8(ctx.b, Interface::BLINK);
            if remainder == 0 {
                return ctx.reinsert();
            }
            let flags = o.u8(ctx.b, Effect::OWN_PALETTE);
            o.set_u8(ctx.b, Effect::OWN_PALETTE, flags & !1);
            let remainder = remainder.wrapping_sub(1);
            o.set_u8(ctx.b, Interface::BLINK, remainder);
            if !(remainder == 0) {
                if remainder & 2 == 0 {
                    return ctx.reinsert();
                }
                let flags = o.u8(ctx.b, Effect::OWN_PALETTE);
                o.set_u8(ctx.b, Effect::OWN_PALETTE, flags | 1);
                return ctx.reinsert();
            }
            let value = o.u16(ctx.b, Interface::VALUE);
            let bit = ((number as u8) << 1) as u32;
            if value >> bit & 1 == 0 {
                return ctx.reinsert();
            }
            if value >> (bit + 1) & 1 == 0 {
                return ctx.reinsert();
            }
            add_u8(ctx, o, field::SUB_STATE, 2);
            copy_palette(ctx, o, 0x800);
            ctx.reinsert();
        }
        state::FREED => ctx.free(&pools::EFFCT),
        state::DESTROYED => ctx.free(&pools::EFFCT),
        _ => bad_index(state as u32),
    }
}

/// End of the initialization of variant 2 (`0xaea0c`): window animation.
pub(super) fn interface_2_animation<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, d1: u16) {
    let number = (o.u8(ctx.b, field::MODE) as u32) << 1;
    let animation = tables().interface_animations_2.at16(ctx.b, number).wrapping_add(d1);
    ctx.anim_start(ANIMATIONS_INTERFACE.0, animation);
    ctx.reinsert();
}

pub(super) fn interface_3<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    if game_finished(ctx) {
        return ctx.free(&pools::EFFCT);
    }
    let state = o.u8(ctx.b, Obj::STATE);
    match state {
        state::INIT => {
            add_u8(ctx, o, Obj::STATE, 2);
            place_on_camera(ctx, o);
            shifted_u16(ctx, o, Obj::X, 0x10);
            shifted_u16(ctx, o, Obj::Z, 0xd8);
            interface_3_animation(ctx, o);
        }
        state::ACTIVE => {
            let (a, b) = (o.u8(ctx.b, Interface::SHOWN), o.u8(ctx.b, Interface::BLINK));
            if !(a != b) {
                return ctx.reinsert();
            }
            interface_3_animation(ctx, o);
        }
        _ => bad_index(state as u32),
    }
}

/// `0xaebfe`: animation `0xe3 - +0x14` of the interface set.
pub(super) fn interface_3_animation<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let value = o.u8(ctx.b, Interface::SHOWN);
    o.set_u8(ctx.b, Interface::BLINK, value);
    ctx.anim_start(ANIMATIONS_INTERFACE.0, 0xe3u8.wrapping_sub(value) as u16);
    ctx.reinsert();
}

pub(super) fn interface_4<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    match state {
        state::INIT => {
            add_u8(ctx, o, Obj::STATE, 2);
            place_on_camera(ctx, o);
            let rank = (o.u8(ctx.b, field::MODE) as u32) << 3;
            let dx = tables().interface_offsets_4.at16(ctx.b, rank);
            shifted_u16(ctx, o, Obj::X, dx);
            let dz = tables().interface_offsets_4.at16(ctx.b, rank + 2);
            shifted_u16(ctx, o, Obj::Z, dz);
            let dy = tables().interface_offsets_4.at16(ctx.b, rank + 4);
            shifted_u16(ctx, o, 0x12, dy);
            let animation = tables().interface_offsets_4.at16(ctx.b, rank + 6);
            ctx.anim_start(ANIMATIONS_INTERFACE.0, animation);
            ctx.reinsert();
        }
        state::ACTIVE => {
            if game_finished(ctx) {
                return ctx.free(&pools::EFFCT);
            }
            ctx.reinsert();
        }
        state::FREED | state::DESTROYED => ctx.free(&pools::EFFCT),
        _ => bad_index(state as u32),
    }
}

pub(super) fn interface_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let variant = o.u8(ctx.b, Obj::VARIANT);
    match variant {
        0 => interface_0(ctx, o),
        1 => interface_1(ctx, o),
        2 => interface_2(ctx, o),
        3 => interface_3(ctx, o),
        4 => interface_4(ctx, o),
        _ => bad_index(2 * variant as u32),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 19: screen sweep (0xaecce)

/// Type 19: object that enters from the left of the screen (x = -0x80, y = 0x150), waits 0x70 frames advancing two
/// pixels per frame, then slows down (acceleration -0x10) until it goes back. Not exercised.
pub(super) mod sweep {
    pub const DELAY: u32 = 0x54;
}

pub(super) fn sweep_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, sweep::DELAY, 0x70);
    o.set_u16(ctx.b, Obj::X, 0xff80);
    o.set_u16(ctx.b, Obj::Y, 0x150);
    o.set_u16(ctx.b, Obj::VX, 0x200);
    o.set_u16(ctx.b, Obj::AX, 0xfff0);
    ctx.anim_start(ANIMATIONS_INTERFACE.0, 0);
}

pub(super) fn sweep_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    if !(sub_state != 0) {
        let delay = o.u8(ctx.b, sweep::DELAY);
        if delay != 0 {
            o.set_u8(ctx.b, sweep::DELAY, delay - 1);
            shifted_u16(ctx, o, Obj::X, 2);
        } else {
            ctx.move_x();
            let vx = o.u16(ctx.b, Obj::VX);
            if !(vx as i16 >= 0) {
                add_u8(ctx, o, field::SUB_STATE, 1);
            }
        }
    }
    ctx.anim_step();
    ctx.reinsert();
}

pub(super) fn sweep_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, sweep_init, sweep_active)
}

