use super::*;

// ---------------------------------------------------------------------------------------------
// Type 32: aura that follows its owner (0xb2c56)

/// Type 32: places itself 0x10 pixels in front (according to its orientation) and a sub-pixel in front of the owner
/// (`+0x50`), 0x18 pixels above it, and follows it at each frame (animation 0 of set `0x193fea`).
pub(super) mod aura {
    pub const ANIMATIONS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x193fea);
}

/// `0xb2ca4`: position of the owner, one sixteenth of a pixel in front, offset by 0x10 pixels.
pub(super) fn aura_follow<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> Obj {
    let player = pointer(ctx, o, field::OWNER);
    let x = player.u32(ctx.b, Obj::X);
    o.set_u32(ctx.b, Obj::X, x);
    let y = player.u32(ctx.b, Obj::Y).wrapping_sub(1);
    o.set_u32(ctx.b, Obj::Y, y);
    let turned = o.u8(ctx.b, field::FLIP) != 0;
    let offset: u16 = if !turned { 0xfff0 } else { 0x10 };
    shifted_u16(ctx, o, Obj::X, offset);
    player
}

pub(super) fn aura_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    let player = aura_follow(ctx, o);
    let z = player.u32(ctx.b, Obj::Z);
    o.set_u32(ctx.b, Obj::Z, z);
    shifted_u16(ctx, o, Obj::Z, 0x18);
    ctx.anim_start(aura::ANIMATIONS.0, 0);
    ctx.reinsert();
}

pub(super) fn aura_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    aura_follow(ctx, o);
    ctx.reinsert();
}

pub(super) fn aura<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, aura_init, aura_active)
}

// ---------------------------------------------------------------------------------------------
// Type 15: sparkle of an object (0xae428)

/// Type 15: sparkle placed on a pickup object (`+0x5c`, created by `0xae3ec` with the object's variant `+0x54`);
/// follows the object, 0x10 pixels higher, as long as it is active (state 2). As long as no player has passed nearby
/// (±0x20 in depth, ±0x60 in abscissa) it stays visible; then it blinks in periods of 0x30 frames, eight times, then
/// disappears.
pub(super) mod shimmering {
    pub const ANIMATIONS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x18fd48);
    pub const REPETITIONS: u32 = 0x57;
    pub const DELAY: u32 = 0x58;
    pub const TOGGLE: u32 = 0x5a;
    pub const TRIGGERS: u32 = 0x5b;
    pub const CARRIER: u32 = 0x5c;
}

pub(super) fn shimmering_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
    o.set_u8(ctx.b, shimmering::REPETITIONS, 8);
    let rng = ctx.random();
    let draw = rng & 0x1f;
    o.set_u16(ctx.b, shimmering::DELAY, draw);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    ctx.anim_start(shimmering::ANIMATIONS.0, variant as u16);
    ctx.reinsert();
}

/// `0xae4e4`: has a player passed near the sparkle?
pub(super) fn shimmer_player_near<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    for i in 0..4u32 {
        let player = crate::world::ram::PLAYERS.nth(i);
        let absent = player.u8(ctx.b, Obj::ACTIVE) == 0;
        if !absent {
            let dy = o.u16(ctx.b, Obj::Y).wrapping_sub(player.u16(ctx.b, Obj::Y)).wrapping_add(0x20);
            if !(dy > 0x40) {
                let dx = o.u16(ctx.b, Obj::X).wrapping_sub(player.u16(ctx.b, Obj::X)).wrapping_add(0x60);
                if !(dx > 0xc0) {
                    o.set_u8(ctx.b, shimmering::TRIGGERS, 1);
                    return;
                }
            }
        }
    }
}

pub(super) fn shimmer_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let carrier = pointer(ctx, o, shimmering::CARRIER);
    copy_position(ctx, o, carrier);
    shifted_u16(ctx, o, Obj::Z, 0x10);
    let state_carrier = carrier.u8(ctx.b, Obj::STATE);
    if !(state_carrier == 2) {
        add_u8(ctx, o, Obj::STATE, 2);
    }
    let triggers = o.u8(ctx.b, shimmering::TRIGGERS);
    if !(triggers != 0) {
        // not triggered yet: watching for the players (and cede control without sorting)
        return shimmer_player_near(ctx, o);
    }
    let remainder = o.u16(ctx.b, shimmering::DELAY).wrapping_sub(1);
    o.set_u16(ctx.b, shimmering::DELAY, remainder);
    if !(remainder != 0) {
        o.set_u16(ctx.b, shimmering::DELAY, 0x30);
        let toggle = o.u8(ctx.b, shimmering::TOGGLE) ^ 1;
        o.set_u8(ctx.b, shimmering::TOGGLE, toggle);
        let flags = o.u8(ctx.b, Obj::FLAGS);
        o.set_u8(ctx.b, Obj::FLAGS, flags & !2);
        if !(toggle != 0) {
            o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
            let remainder = o.u8(ctx.b, shimmering::REPETITIONS).wrapping_sub(1);
            o.set_u8(ctx.b, shimmering::REPETITIONS, remainder);
            if !(remainder != 0) {
                o.set_u32(ctx.b, Obj::STATE, 0x0400_0000);
            }
        }
    }
    ctx.reinsert();
}

pub(super) fn shimmering<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, shimmering_init, shimmer_active)
}

// ---------------------------------------------------------------------------------------------
// Type 25: thrown dust (0xb111a)

/// Type 25: bursts thrown at birth (speed `+0x14` / `+0x1c`, scattered at random according to the mode `+0x2f`,
/// oriented according to `+0x2c`), then drifting horizontally while blinking until the end of their animation.
pub(super) mod dust {
    pub const ANIMATIONS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1901ba);
    /// Direction of the drift (`+0x2d`) and on which axis the jump is made (`+0x43`).
    pub const DIRECTION: u32 = 0x2d;
    pub const ON_Z: u32 = 0x43;
}

pub(super) fn dust_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    let mut dx = o.u16(ctx.b, Obj::VX);
    let mut dy = o.u16(ctx.b, Obj::VY);
    let turned = o.u8(ctx.b, field::FLIP) != 0;
    if turned {
        dx = dx.wrapping_neg();
    }
    let mode = o.u8(ctx.b, field::MODE);
    if !(mode == 0) {
        let d3 = (mode as u16) << 3;
        dx = dx.wrapping_sub(d3);
        dy = dy.wrapping_sub(d3);
        let mask = (d3 << 1).wrapping_sub(1);
        let rng = ctx.random();
        dx = dx.wrapping_add(rng & mask);
        let rng = ctx.random();
        dy = dy.wrapping_add(rng & mask);
    }
    shifted_u16(ctx, o, Obj::X, dx);
    let on_z = o.u8(ctx.b, dust::ON_Z);
    if !(on_z == 0) {
        shifted_u16(ctx, o, Obj::Z, dy);
    } else {
        shifted_u16(ctx, o, Obj::Y, dy);
    }
    o.set_u16(ctx.b, Obj::VX, 0);
    let flip = o.u8(ctx.b, dust::DIRECTION);
    if !(flip == 0) {
        let mut v: u16 = 0x180;
        let turned = o.u8(ctx.b, field::FLIP) != 0;
        if turned {
            v = v.wrapping_neg();
        }
        let flip = o.u8(ctx.b, dust::DIRECTION);
        if !(flip == 1) {
            v = v.wrapping_neg();
        }
        o.set_u16(ctx.b, Obj::VX, v);
    }
    let variant = o.u8(ctx.b, Obj::VARIANT);
    ctx.anim_start(dust::ANIMATIONS.0, variant as u16);
    ctx.reinsert();
}

pub(super) fn dust_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let vx = o.u16(ctx.b, Obj::VX) as i16 as i32;
    let x = o.u32(ctx.b, Obj::X).wrapping_add((vx << 8) as u32);
    o.set_u32(ctx.b, Obj::X, x);
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags ^ 2);
    let end = ctx.anim_step();
    if end {
        return ctx.free(&pools::EFFCT);
    }
    ctx.reinsert();
}

pub(super) fn dust<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, dust_init, dust_active)
}

// ---------------------------------------------------------------------------------------------
// Type 27: rising pickup effect (0xb12e6)

/// Type 27: effect placed on the owner object (`+0x50`, created by `0xb12c0`: pickup) that rises by 0x8000 (half a
/// pixel) per frame for 0x1c frames. Not exercised.
pub(super) mod smoke {
    pub const ANIMATIONS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x19046c);
    pub const DURATION: u32 = 0x57;
}

pub(super) fn smoke_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, smoke::DURATION, 0x1c);
    let owner = pointer(ctx, o, field::OWNER);
    let x = owner.u16(ctx.b, Obj::X);
    o.set_u16(ctx.b, Obj::X, x);
    let y = owner.u16(ctx.b, Obj::Y).wrapping_sub(1);
    o.set_u16(ctx.b, Obj::Y, y);
    o.set_u16(ctx.b, Obj::Z, 0x10);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    ctx.anim_start(smoke::ANIMATIONS.0, variant as u16);
    ctx.reinsert();
}

pub(super) fn smoke_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let remainder = o.u8(ctx.b, smoke::DURATION).wrapping_sub(1);
    o.set_u8(ctx.b, smoke::DURATION, remainder);
    if !(remainder != 0) {
        add_u8(ctx, o, Obj::STATE, 2);
    }
    let z = o.u32(ctx.b, Obj::Z).wrapping_add(0x8000);
    o.set_u32(ctx.b, Obj::Z, z);
}

pub(super) fn smoke_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, smoke_init, smoke_active)
}

// ---------------------------------------------------------------------------------------------
// Type 28: spark placed on an object (0xb136e)

/// Type 28: spark stuck to an object (`+0x5c`), 0x20 above it; the animation depends on the variant and on `A5+0x70`.
/// At the end of the animation, it notifies its object (`+0x77` ← 1, except variant 6) then disappears. Not exercised.
pub(super) mod spark {
    pub const ANIMATIONS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x190534);
    pub const CARRIER: u32 = 0x5c;
}

pub(super) fn spark_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    let carrier = pointer(ctx, o, spark::CARRIER);
    copy_position(ctx, o, carrier);
    let y = o.u16(ctx.b, Obj::Y).wrapping_sub(1);
    o.set_u16(ctx.b, Obj::Y, y);
    shifted_u16(ctx, o, Obj::Z, 0x20);
    let variant = o.u8(ctx.b, Obj::VARIANT) as u16;
    let word = vars::DAMAGE_SETTING.get(ctx.b);
    let animation = variant.wrapping_add((word & 0xf) >> 1 << 3);
    ctx.anim_start(spark::ANIMATIONS.0, animation);
    ctx.reinsert();
}

pub(super) fn spark_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let end = ctx.anim_step();
    if !end {
        return;
    }
    add_u8(ctx, o, Obj::STATE, 2);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if variant == 6 {
        return;
    }
    let carrier = pointer(ctx, o, spark::CARRIER);
    carrier.set_u8(ctx.b, Carrier::MARK, 1);
}

pub(super) fn spark_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    match state {
        state::INIT => spark_init(ctx, o),
        state::ACTIVE => spark_active(ctx, o),
        state::FREED => add_u8(ctx, o, Obj::STATE, 2),
        6 => ctx.free(&pools::EFFCT),
        _ => bad_index(state as u32),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 51: projectile thrown from an object (0xb64c8)

/// Type 51: born on the object `+0x5c` (if that object disappears, it goes to state 6), offset by 0x84 pixels forward
/// and 0x16 in depth, then leaves in an arc (speeds 0x140 and 0x3c0, gravity 0x38) until it touches the ground. Not
/// exercised.
pub(super) mod projectile {
    pub const ANIMATIONS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1733ac);
    pub const TARGET: u32 = 0x5c;
}

pub(super) fn projectile_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    let pointer_target = o.u16(ctx.b, projectile::TARGET);
    if pointer_target == 0 {
        return;
    }
    let target = pointer(ctx, o, projectile::TARGET);
    for off in [Obj::X, Obj::Y, Obj::Z] {
        let v = target.u32(ctx.b, off);
        o.set_u32(ctx.b, off, v);
    }
    let flip = target.u8(ctx.b, field::FLIP);
    o.set_u8(ctx.b, field::FLIP, flip);
    shifted_u16(ctx, o, Obj::Y, 1);
    let (mut dx, dy) = (0x84u16, 0xffeau16);
    o.set_u16(ctx.b, Obj::VX, 0x140);
    o.set_u16(ctx.b, Obj::VZ, 0x3c0);
    o.set_u16(ctx.b, Obj::AZ, 0xffc8);
    if !(flip == 0) {
        dx = dx.wrapping_neg();
        let v = o.u16(ctx.b, Obj::VX).wrapping_neg();
        o.set_u16(ctx.b, Obj::VX, v);
    }
    shifted_u16(ctx, o, Obj::X, dx);
    shifted_u16(ctx, o, Obj::Y, dy);
    let orientation = o.u8(ctx.b, field::FLIP) ^ 0x20;
    o.set_u8(ctx.b, field::FLIP, orientation);
    ctx.anim_start(projectile::ANIMATIONS.0, 0x19);
    ctx.reinsert();
}

pub(super) fn projectile_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    match sub_state {
        0 => {
            let in_flight = ctx.move_xz().gt();
            if !in_flight {
                add_u8(ctx, o, field::SUB_STATE, 2);
                o.set_u16(ctx.b, Obj::Z, 0);
            }
            ctx.anim_step();
        }
        2 => {}
        _ => bad_index(sub_state as u32),
    }
}

pub(super) fn projectile_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let pointer_target = o.u16(ctx.b, projectile::TARGET);
    if pointer_target == 0 {
        o.set_u32(ctx.b, Obj::STATE, 0x0600_0000);
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    match state {
        state::INIT => projectile_init(ctx, o),
        state::ACTIVE => projectile_active(ctx, o),
        state::FREED | 6 => ctx.free(&pools::EFFCT),
        _ => bad_index(state as u32),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 45: proximity trigger (0xb5340)

/// Type 45: object placed in the scenery that waits for the main player (`A5+0x46`) to pass over it: it then gives an
/// event to each present player (`0x19b08`, offset per player) then disappears; it also disappears if it stays behind
/// the camera. Not exercised.
pub(super) mod trigger {
    pub const ANIMATIONS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1bf5c2);
    pub const PRINCIPAL: i32 = 0x46;
    pub const EFFECT: crate::hw::sound::Sound = crate::hw::sound::CMD_14A_24A;
}

/// `0xb53b0`: trigger.
pub(super) fn trigger_effect<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    crate::hw::sound::play(ctx.b, trigger::EFFECT, ctx.obj());
    let level = crate::world::ram::PLAYERS_ALIVE.get(ctx.b) as u32;
    let mut offset = tables().trigger_offsets.at8(ctx.b, level) as u16;
    for i in 0..4u32 {
        let player = crate::world::ram::PLAYERS.nth(i);
        let absent = player.u8(ctx.b, Obj::ACTIVE) == 0;
        if !absent {
            let variant = o.u8(ctx.b, Obj::VARIANT) as u32;
            crate::world::events::enqueue_counter(ctx.b, (variant as u16).wrapping_add(offset));
        }
        offset = offset.wrapping_add(0x100);
    }
    ctx.free(&pools::EFFCT);
}

pub(super) fn trigger_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let limit = crate::world::ram::CAMERA_X.get(ctx.b).wrapping_sub(0x60);
    let x = o.u16(ctx.b, Obj::X);
    if limit as i16 > x as i16 {
        return ctx.free(&pools::EFFCT);
    }
    let principal = Obj(ctx.ram16(trigger::PRINCIPAL) as i16 as i32 as u32 & 0xff_ffff);
    let dx = principal.u16(ctx.b, Obj::X).wrapping_sub(x).wrapping_add(0x44);
    if dx > 0x74 {
        return;
    }
    let dy = principal.u16(ctx.b, Obj::Y).wrapping_sub(o.u16(ctx.b, Obj::Y)).wrapping_sub(9);
    if !(dy <= 0x1c) {
        return;
    }
    trigger_effect(ctx, o);
}

pub(super) fn trigger_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    ctx.anim_start(trigger::ANIMATIONS.0, variant as u16);
    ctx.reinsert();
}

pub(super) fn trigger_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    match state {
        state::INIT => trigger_init(ctx, o),
        state::ACTIVE => trigger_active(ctx, o),
        state::FREED => trigger_effect(ctx, o),
        state::DESTROYED => ctx.free(&pools::EFFCT),
        _ => bad_index(state as u32),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 8: trace on an object (0xababa)

/// Type 8: trace stuck to an object (`+0x5c`), a sub-pixel in front or behind according to the variant; its animation
/// depends on the object's type (and its class: BOX); it disappears at the end of the animation. Not exercised.
pub(super) mod trace {
    pub const ANIMATIONS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x18e872);
    pub const CARRIER: u32 = 0x5c;
}

pub(super) fn trace_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    let carrier = pointer(ctx, o, trace::CARRIER);
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = carrier.u32(ctx.b, off);
        o.set_u32(ctx.b, off, v);
    }
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let offset: u32 = if variant == 0 { 0xffff_ffff } else { 1 };
    let y = o.u32(ctx.b, Obj::Y).wrapping_add(offset);
    o.set_u32(ctx.b, Obj::Y, y);
    let mut d0: u8 = 0;
    let class = carrier.u8(ctx.b, Obj::CLASS);
    if !(class != 4) {
        d0 = 0x20;
    }
    d0 = d0.wrapping_add(carrier.u8(ctx.b, Obj::KIND));
    let animation = tables().trace_animations_type.at8(ctx.b, d0 as u32).wrapping_add(variant);
    ctx.anim_start(trace::ANIMATIONS.0, animation as u16);
    ctx.reinsert();
}

pub(super) fn trace_active<R: Bench>(ctx: &mut Cx<'_, R>, _o: Obj) {
    let end = ctx.anim_step();
    if end {
        ctx.free(&pools::EFFCT);
    }
}

pub(super) fn trace_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, trace_init, trace_active)
}

