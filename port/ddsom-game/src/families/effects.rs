//! EFFCT family (class 0xE): visual effects (sparks, bursts, banners, animated interface elements). Dispatch `0x39cba`,
//! type table `0x39ce0` (92 types, 38 slots of 0x60 bytes).
//!
//! All the handlers share the same framework: the state byte `+4` chooses, through a table of relative words, the
//! initialization (0), the active state (2) or the release (4). The types ported here are those that the scenarios
//! exercise; the others stay in original code. See `docs/familles.md`.

use crate::objects::pools;
use crate::objects::Obj;

use super::{add_u8, handler, pointer};
use super::cx::{Bench, Cx};
use crate::objects::Current;
use crate::machine::Machine;

/// Values of the state byte `+4`.
mod state {
    pub const INIT: u8 = 0;
    pub const ACTIVE: u8 = 2;
    pub const FREED: u8 = 4;
    /// Set by the parent object that destroys its effects (or by the effect itself, `accessory_follow`): release,
    /// without the work of state 4 (`jmp 0x2218`).
    pub const DESTROYED: u8 = 6;
}

/// Shared fields.
mod field {
    pub const SUB_STATE: u32 = 0x05;
    pub const MODE: u32 = 0x2f;
    pub const FLIP: u32 = 0x2c;
    pub const OWNER: u32 = 0x50;
}

/// Players (4 records of 0x200 bytes).
const PLAYERS: u32 = 0xff85de;
const STEP_PLAYER: u32 = 0x200;
/// "Interface" animation set common to many effects.
const ANIMATIONS_INTERFACE: u32 = 0x1c03a6;

/// Target of the table jump `move.w (table,PC,Dn.w),D1 ; jmp (table,PC,D1.w)` for the index `index` (the address the
/// original would reach; only the indices outside the table still lead there, `outside_table`).
pub(crate) fn target<R: Bench>(ctx: &mut Cx<'_, R>, table: u32, index: u32) -> u32 {
    let word = ctx.prog16(table.wrapping_add(index)) as i16;
    table.wrapping_add(word as i32 as u32)
}

/// State or variant value outside the original's jump table, which would jump into data or into the middle of code:
/// never produced (docs/code-mort.md, batch B).
pub(crate) fn outside_table(target: u32) -> ! {
    unreachable!("index outside the jump table: the original would jump to {target:#x}")
}

/// Pool full where the original jumps to `address`: `tst.w (1,A5)`, reading a word at an odd address. The 68000 raises
/// an address error and the game stops on its exception screen; the engine stops too (docs/code-mort.md, batch B).
pub(crate) fn address_error(address: u32) -> ! {
    panic!("pool full: the original reads a word at an odd address {address:#x} (address error, exception screen)")
}

/// Copies `X`, `Z`, `Y` (high words) of `source` to those of `o`.
fn copy_position<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, source: Obj) {
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = source.u16(ctx.b, off);
        o.set_u16(ctx.b, off, v);
    }
}

/// Adds `n` to the high word of a coordinate.
fn shifted_u16<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, off: u32, n: u16) {
    let v = o.u16(ctx.b, off).wrapping_add(n);
    o.set_u16(ctx.b, off, v);
}

/// Three-state handler (init, active, release): reads the state, jumps through the word table `table`, runs `init` or
/// `active`; state 4 frees the object.
fn three_states<R: Bench>(
    ctx: &mut Cx<'_, R>,
    table: u32,
    init: fn(&mut Cx<'_, R>, Obj),
    active: fn(&mut Cx<'_, R>, Obj),
) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, table, state as u32);
    match state {
        state::INIT => init(ctx, o),
        state::ACTIVE => active(ctx, o),
        state::FREED => ctx.free(&pools::EFFCT),
        state::DESTROYED => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 5: halo attached to a player (0xab726)

/// Type 5: appears at the position of the owner player (`+0x50`), offset according to the variant (even, 0 to 10) and
/// the player's orientation, waits for a delay (byte `+0x18`, a function of byte `+0x7d` of the player) then lives
/// `0x28` frames before disappearing. Animation 0 of set `0x18e650`.
mod halo {
    pub const ENTRY: u32 = 0xab726;
    pub const TABLE: u32 = 0xab734;
    /// Offsets (dx, dz) per half-variant, words.
    pub const OFFSETS: u32 = 0xab79a;
    /// Appearance delays according to `+0x7d` of the player.
    pub const DELAYS: u32 = 0xab7b2;
    pub const ANIMATIONS: u32 = 0x18e650;
    pub const DURATION: u32 = 0x14;
    pub const DELAY: u32 = 0x18;
}

fn halo_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, halo::DURATION, 0x28);
    let player = pointer(ctx, o, field::OWNER);
    copy_position(ctx, o, player);
    let rank = ((o.u8(ctx.b, Obj::VARIANT) >> 1) as u32) << 2;
    let mut dx = ctx.prog16(halo::OFFSETS + rank);
    let dz = ctx.prog16(halo::OFFSETS + 2 + rank);
    let turned = player.u8(ctx.b, field::FLIP) != 0;
    if !turned {
        dx = dx.wrapping_neg();
    }
    shifted_u16(ctx, o, Obj::X, dx);
    shifted_u16(ctx, o, Obj::Z, dz);
    let phase = player.u8(ctx.b, 0x7d) as u32;
    let delay = ctx.prog8(halo::DELAYS + phase);
    o.set_u8(ctx.b, halo::DELAY, delay);
    ctx.anim_start(halo::ANIMATIONS, 0);
}

fn halo_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let delay = o.u8(ctx.b, halo::DELAY);
    if !(delay == 0) {
        o.set_u8(ctx.b, halo::DELAY, delay - 1);
        return;
    }
    let remainder = o.u8(ctx.b, halo::DURATION).wrapping_sub(1);
    o.set_u8(ctx.b, halo::DURATION, remainder);
    if !(remainder != 0) {
        add_u8(ctx, o, Obj::STATE, 2);
    }
    ctx.reinsert();
}

fn halo<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, halo::TABLE, halo_init, halo_active)
}

// ---------------------------------------------------------------------------------------------
// Type 21: sliding interface banner (0xaf360)

/// Type 21: interface banner (variant 0: at the bottom, 1: at the top) that slides in `Z` by one pixel per frame (as
/// long as `A5+0x6f` bit 0 is zero) up to the target height `+0x18`, then frees itself by decrementing the byte pointed
/// to by `+0x58` (banner counter of the creator).
mod banner {
    pub const ENTRY: u32 = 0xaf360;
    pub const TABLE: u32 = 0xaf36e;
    pub const TARGET: u32 = 0x18;
    pub const CREATOR: u32 = 0x58;
}

fn banner_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
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
    ctx.anim_start(ANIMATIONS_INTERFACE, 0xec);
    ctx.reinsert();
}

/// End of the banner: decrements the creator's counter and frees the object.
fn banner_end<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let counter = crate::mem::rd32(ctx.b, o.0 + banner::CREATOR);
    let v = crate::mem::rd8(ctx.b, counter).wrapping_sub(1);
    crate::mem::wr8(ctx.b, counter, v);
    ctx.free(&pools::EFFCT);
}

fn banner_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let pause = ctx.ram8(0x6f) & 1 != 0;
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

fn banner<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, banner::TABLE, state as u32);
    match state {
        state::INIT => banner_init(ctx, o),
        state::ACTIVE => banner_active(ctx, o),
        state::FREED => banner_end(ctx, o),
        state::DESTROYED => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 32: aura that follows its owner (0xb2c56)

/// Type 32: places itself 0x10 pixels in front (according to its orientation) and a sub-pixel in front of the owner
/// (`+0x50`), 0x18 pixels above it, and follows it at each frame (animation 0 of set `0x193fea`).
mod aura {
    pub const ENTRY: u32 = 0xb2c56;
    pub const TABLE: u32 = 0xb2c64;
    pub const ANIMATIONS: u32 = 0x193fea;
}

/// `0xb2ca4`: position of the owner, one sixteenth of a pixel in front, offset by 0x10 pixels.
fn aura_follow<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) -> Obj {
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

fn aura_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    let player = aura_follow(ctx, o);
    let z = player.u32(ctx.b, Obj::Z);
    o.set_u32(ctx.b, Obj::Z, z);
    shifted_u16(ctx, o, Obj::Z, 0x18);
    ctx.anim_start(aura::ANIMATIONS, 0);
    ctx.reinsert();
}

fn aura_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    aura_follow(ctx, o);
    ctx.reinsert();
}

fn aura<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, aura::TABLE, aura_init, aura_active)
}

// ---------------------------------------------------------------------------------------------
// Type 15: sparkle of an object (0xae428)

/// Type 15: sparkle placed on a pickup object (`+0x5c`, created by `0xae3ec` with the object's variant `+0x54`);
/// follows the object, 0x10 pixels higher, as long as it is active (state 2). As long as no player has passed nearby
/// (±0x20 in depth, ±0x60 in abscissa) it stays visible; then it blinks in periods of 0x30 frames, eight times, then
/// disappears.
mod shimmering {
    pub const ENTRY: u32 = 0xae428;
    pub const TABLE: u32 = 0xae436;
    pub const ANIMATIONS: u32 = 0x18fd48;
    pub const REPETITIONS: u32 = 0x57;
    pub const DELAY: u32 = 0x58;
    pub const TOGGLE: u32 = 0x5a;
    pub const TRIGGERS: u32 = 0x5b;
    pub const CARRIER: u32 = 0x5c;
}

fn shimmering_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
    o.set_u8(ctx.b, shimmering::REPETITIONS, 8);
    let rng = ctx.random();
    let draw = rng & 0x1f;
    o.set_u16(ctx.b, shimmering::DELAY, draw);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    ctx.anim_start(shimmering::ANIMATIONS, variant as u16);
    ctx.reinsert();
}

/// `0xae4e4`: has a player passed near the sparkle?
fn shimmer_player_near<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    for i in 0..4u32 {
        let player = Obj(PLAYERS + i * STEP_PLAYER);
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

fn shimmer_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
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

fn shimmering<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, shimmering::TABLE, shimmering_init, shimmer_active)
}

// ---------------------------------------------------------------------------------------------
// Type 9: trail that crosses the screen (0xabbee)

/// Type 9: object born at a screen position drawn from a table (according to `+0x52`), advances in `X` (speed 0x200),
/// waits 0x120 frames (animation `variant + 0xc`) then plays its exit animation for 0x200 frames, advancing during the
/// last 0x50, before disappearing.
mod trail {
    pub const ENTRY: u32 = 0xabbee;
    pub const TABLE: u32 = 0xabbfc;
    pub const POSITIONS: u32 = 0xabc54;
    pub const ANIMATIONS: u32 = 0x18ea30;
    pub const DELAY: u32 = 0x50;
    pub const POSITION: u32 = 0x52;
}

fn trail_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    o.set_u16(ctx.b, trail::DELAY, 0x120);
    o.set_u32(ctx.b, Obj::VX, 0x0200_0000);
    let rank = (o.u8(ctx.b, trail::POSITION) as u32) << 2;
    let x = ctx.prog16(trail::POSITIONS + rank);
    o.set_u16(ctx.b, Obj::X, x);
    let y = ctx.prog16(trail::POSITIONS + rank + 2);
    o.set_u16(ctx.b, Obj::Y, y);
    o.set_u16(ctx.b, Obj::Z, 0);
    let (cam_x, cam_y) = (crate::ram::CAMERA_X.get(ctx.b), crate::ram::CAMERA_Y.get(ctx.b));
    shifted_u16(ctx, o, Obj::X, cam_x);
    shifted_u16(ctx, o, Obj::Y, cam_y);
    let animation = o.u8(ctx.b, Obj::VARIANT).wrapping_add(0xc);
    ctx.anim_start(trail::ANIMATIONS, animation as u16);
}

fn trail_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    if !(sub_state != 0) {
        let remainder = o.u16(ctx.b, trail::DELAY).wrapping_sub(1);
        o.set_u16(ctx.b, trail::DELAY, remainder);
        if !(remainder != 0) {
            add_u8(ctx, o, field::SUB_STATE, 2);
            o.set_u16(ctx.b, trail::DELAY, 0x200);
            let variant = o.u8(ctx.b, Obj::VARIANT);
            ctx.anim_start(trail::ANIMATIONS, variant as u16);
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

fn trail<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, trail::TABLE, trail_init, trail_active)
}

// ---------------------------------------------------------------------------------------------
// Type 33: rubble that falls back (0xb2cca)

/// Type 33: burst thrown from a screen position (variant 0 to 3, speeds drawn from a table), drawn in the foreground.
/// It falls back; on touching the ground, the non-zero variants disappear, variant 0 (outside pause) creates two enemy
/// shots (ESHL 0x17, modes 0 and 1) before disappearing.
mod shard {
    pub const ENTRY: u32 = 0xb2cca;
    pub const TABLE: u32 = 0xb2cd8;
    pub const POSITIONS: u32 = 0xb2d72;
    pub const SPEEDS: u32 = 0xb2d52;
    pub const ANIMATIONS: u32 = 0x193ffe;
    pub const OFFSET: u32 = 0x5c;
    pub const CREATE_SHOT_A: u32 = 0xeab50;
}

fn shard_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    let rng = ctx.random();
    let draw = rng & 0x3f;
    o.set_u16(ctx.b, shard::OFFSET, draw);
    let flip = o.u8(ctx.b, field::FLIP) ^ 0x20;
    o.set_u8(ctx.b, field::FLIP, flip);
    let rank = (o.u8(ctx.b, Obj::VARIANT) as u32) << 2;
    let dx = ctx.prog16(shard::POSITIONS + rank);
    let dz = ctx.prog16(shard::POSITIONS + rank + 2);
    o.set_u16(ctx.b, Obj::Y, 0x40);
    let (x, z) = (crate::ram::CAMERA_ALT_X.get(ctx.b), crate::ram::CAMERA_ALT_Y.get(ctx.b));
    o.set_u16(ctx.b, Obj::X, x);
    o.set_u16(ctx.b, Obj::Z, z);
    shifted_u16(ctx, o, Obj::X, dx);
    shifted_u16(ctx, o, Obj::Z, dz);
    let rank = rank << 1;
    for (i, off) in [Obj::VX, Obj::VZ, Obj::AZ].into_iter().enumerate() {
        let v = ctx.prog16(shard::SPEEDS + rank + 2 * i as u32);
        o.set_u16(ctx.b, off, v);
    }
    let az = o.u16(ctx.b, Obj::AZ).wrapping_neg();
    o.set_u16(ctx.b, Obj::AZ, az);
    ctx.anim_start(shard::ANIMATIONS, 0);
    ctx.reinsert_foreground();
}

/// Landing (`0xb2d92`): disappearance, or two shots for variant 0.
fn shard_lands<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if !(variant != 0) {
        let pause = ctx.ram8(0x42) != 0;
        if !pause {
            crate::sound::play(ctx.b, shard::CREATE_SHOT_A, ctx.obj());
            let shot = ctx.alloc(&pools::ESHL);
            if shot.is_none() {
                return;
            }
            let shot = shot.unwrap();
            shot.set_u8(ctx.b, Obj::ACTIVE, 1);
            shot.set_u8(ctx.b, Obj::KIND, 0x17);
            shot.set_u8(ctx.b, field::MODE, 0);
            super::effects_creators::set_list(ctx.b, 3);
            let shot = ctx.alloc(&pools::ESHL);
            if shot.is_none() {
                return;
            }
            let shot = shot.unwrap();
            shot.set_u8(ctx.b, Obj::ACTIVE, 1);
            shot.set_u8(ctx.b, Obj::KIND, 0x17);
            shot.set_u8(ctx.b, field::MODE, 1);
        }
    }
    ctx.free(&pools::EFFCT);
}

fn shard_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ctx.anim_step();
    let in_flight = ctx.move_xz().pl(); // bpl
    if in_flight {
        return;
    }
    shard_lands(ctx, o)
}

fn shard<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, shard::TABLE, state as u32);
    match state {
        state::INIT => shard_init(ctx, o),
        state::ACTIVE => shard_active(ctx, o),
        state::FREED => shard_lands(ctx, o),
        state::DESTROYED => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 25: thrown dust (0xb111a)

/// Type 25: bursts thrown at birth (speed `+0x14` / `+0x1c`, scattered at random according to the mode `+0x2f`,
/// oriented according to `+0x2c`), then drifting horizontally while blinking until the end of their animation.
mod dust {
    pub const ENTRY: u32 = 0xb111a;
    pub const TABLE: u32 = 0xb1128;
    pub const ANIMATIONS: u32 = 0x1901ba;
    /// Direction of the drift (`+0x2d`) and on which axis the jump is made (`+0x43`).
    pub const DIRECTION: u32 = 0x2d;
    pub const ON_Z: u32 = 0x43;
}

fn dust_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
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
    ctx.anim_start(dust::ANIMATIONS, variant as u16);
    ctx.reinsert();
}

fn dust_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
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

fn dust<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, dust::TABLE, dust_init, dust_active)
}

// ---------------------------------------------------------------------------------------------
// Type 26: cloud that crosses the screen (0xb11f4)

/// Type 26: cloud born at the right of the screen (random height), waits a random delay (`+0x5c`, 0 to 63 frames) then
/// crosses the screen to the left (speed according to the variant) up to 0x40 pixels beyond the edge.
mod cloud {
    pub const ENTRY: u32 = 0xb11f4;
    pub const TABLE: u32 = 0xb1202;
    pub const SPEEDS: u32 = 0xb126a;
    pub const ANIMATIONS_NUMBER: u32 = 0xb1264;
    pub const ANIMATIONS: u32 = 0x1bf8fe;
    pub const DELAY: u32 = 0x5c;
}

fn cloud_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
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
    let speed = ctx.prog16(cloud::SPEEDS + 2 * variant);
    o.set_u16(ctx.b, Obj::VX, speed);
    let animation = ctx.prog8(cloud::ANIMATIONS_NUMBER + variant);
    ctx.anim_start(cloud::ANIMATIONS, animation as u16);
}

fn cloud_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
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

fn cloud<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, cloud::TABLE, cloud_init, cloud_active)
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
mod interface {
    pub const ENTRY: u32 = 0xae78c;
    pub const TABLE: u32 = 0xae79c;
    pub const STATES_0: u32 = 0xae7bc;
    pub const STATES_1: u32 = 0xae86a;
    pub const STATES_2: u32 = 0xae96c;
    pub const STATES_3: u32 = 0xaebda;
    pub const STATES_4: u32 = 0xaec3c;
    pub const PALETTE: u32 = 0x91c000;
    pub const PALETTE_0: u32 = 0x91c0e0;
    /// Source palettes (data) and palette address lists (per player).
    pub const SOURCES: u32 = 0x28ff00;
    pub const LISTS: u32 = 0xaeb62;
    /// Offsets of variant 1 per character.
    pub const OFFSETS_1: u32 = 0xae93c;
    /// Variant 2: animations and offsets per player.
    pub const ANIMATIONS_2: u32 = 0xae9e6;
    pub const SHAPES_2: u32 = 0xae9c2;
    pub const OFFSETS_2: u32 = 0xae9ce;
    pub const OFFSETS_4: u32 = 0xaec8e;
    pub const CARRIER: u32 = 0x1e;
    pub const PLAYER: u32 = 0x1a;
    pub const CHARACTER: u32 = 0x42;
    // subroutines not rewritten
    pub const CHANGE_CHARACTER: u32 = 0xea344;
    pub const SOUND_GAUGE: u32 = 0xea3b6;
}

/// Game phase: 8 or more, the game is over.
fn game_finished<R: Bench>(ctx: &mut Cx<'_, R>) -> bool {
    let phase = crate::ram::PHASE.get(ctx.b) as i8;
    phase >= 8
}

/// Places the object on the camera (`X` = camera x, `Z` = camera y, `Y` = 0).
fn place_on_camera<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let (cam_x, cam_y) = (crate::ram::CAMERA_X.get(ctx.b), crate::ram::CAMERA_Y.get(ctx.b));
    o.set_u16(ctx.b, Obj::X, cam_x);
    o.set_u16(ctx.b, Obj::Z, cam_y);
    o.set_u32(ctx.b, Obj::Y, 0);
}

fn interface_0<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    if game_finished(ctx) {
        return ctx.free(&pools::EFFCT);
    }
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, interface::STATES_0, state as u32);
    match state {
        state::INIT => {
            let ready = o.u8(ctx.b, 0x1c);
            if ready == 0 {
                return;
            }
            add_u8(ctx, o, Obj::STATE, 2);
            for i in 0..8 {
                crate::mem::wr32(ctx.b, interface::PALETTE_0 + 4 * i, 0xffff_ffff);
            }
        }
        state::ACTIVE => {}
        _ => outside_table(target),
    }
}

fn interface_1<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, interface::STATES_1, state as u32);
    match state {
        state::INIT => {
            add_u8(ctx, o, Obj::STATE, 2);
            place_on_camera(ctx, o);
            let number = o.u8(ctx.b, field::MODE) as u32;
            for _ in 0..number {
            }
            let player = Obj(PLAYERS + number * STEP_PLAYER);
            o.set_u16(ctx.b, interface::PLAYER, player.0 as u16);
            let character = player.u8(ctx.b, 3) & 0x7e;
            o.set_u8(ctx.b, interface::CHARACTER, character);
            ctx.anim_start(ANIMATIONS_INTERFACE, 0x8e + number as u16);
        }
        state::ACTIVE => {
            let carrier = pointer(ctx, o, interface::CARRIER);
            let carrier_absent = carrier.u8(ctx.b, 0x1c) == 0;
            let player = pointer(ctx, o, interface::PLAYER);
            if carrier_absent || {
                let absent = player.u8(ctx.b, Obj::ACTIVE) == 0;
                absent
            } {
                if game_finished(ctx) {
                    ctx.free(&pools::EFFCT);
                }
                return;
            }
            place_on_camera(ctx, o);
            let character = player.u8(ctx.b, 3) & 0x7e;
            let mut change = false;
            let known = o.u8(ctx.b, interface::CHARACTER);
            if !(character == known) {
                o.set_u8(ctx.b, interface::CHARACTER, character);
                change = true;
            }
            let rank = (character as u32) << 1;
            let dx = ctx.prog16(interface::OFFSETS_1 + rank);
            let dz = ctx.prog16(interface::OFFSETS_1 + 2 + rank);
            shifted_u16(ctx, o, Obj::X, dx);
            shifted_u16(ctx, o, Obj::Z, dz);
            if change {
                crate::sound::play(ctx.b, interface::CHANGE_CHARACTER, ctx.obj());
            }
            let raise = o.u8(ctx.b, 0x16) != 0;
            if raise {
                o.set_u16(ctx.b, 0x12, 1);
                shifted_u16(ctx, o, Obj::Z, 0x10);
            }
            ctx.anim_step();
            ctx.reinsert();
        }
        state::FREED => ctx.free(&pools::EFFCT),
        state::DESTROYED => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

/// `0xaeb08`: copies into the palette (OR of the priority bit 0xF000F000) the two 16-byte blocks of each slot of the
/// player's list; `offset` chooses the source palette (0 or 0x400/0x800).
fn copy_palette<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, offset: u16) {
    let player = (o.u8(ctx.b, field::MODE) as u32) << 1 & 0xff;
    let rank = ctx.prog16(interface::LISTS + player) as i16 as i32 as u32;
    let mut list = interface::LISTS.wrapping_add(rank);
    loop {
        let slot = ctx.data16(list);
        list += 2;
        if (slot as i16) < 0 {
            return; // bmi: end of list
        }
        let destination = interface::PALETTE.wrapping_add(slot as i16 as i32 as u32);
        let source = interface::SOURCES.wrapping_add(slot.wrapping_add(offset) as i16 as i32 as u32);
        for k in 0..8 {
            let v = ctx.data32(source + 4 * k) | 0xf000_f000;
            crate::mem::wr32(ctx.b, destination + 4 * k, v);
        }
    }
}

fn interface_2<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, interface::STATES_2, state as u32);
    match state {
        state::INIT => {
            add_u8(ctx, o, Obj::STATE, 2);
            place_on_camera(ctx, o);
            o.set_u16(ctx.b, Obj::VX, 0);
            o.set_u16(ctx.b, Obj::VY, 0);
            o.set_u8(ctx.b, 0x2a, 0);
            o.set_u8(ctx.b, 0x2b, 7);
            let number = (o.u8(ctx.b, field::MODE) as u32) << 1;
            let shape = ctx.prog16(interface::SHAPES_2 + number);
            o.set_u16(ctx.b, 0x12, shape);
            let dx = ctx.prog16(interface::OFFSETS_2 + 2 * number);
            let dz = ctx.prog16(interface::OFFSETS_2 + 2 * number + 2);
            shifted_u16(ctx, o, Obj::X, dx);
            shifted_u16(ctx, o, Obj::Z, dz);
            interface_2_animation(ctx, o, 0);
        }
        state::ACTIVE => {
            if game_finished(ctx) {
                return ctx.free(&pools::EFFCT);
            }
            let carrier = pointer(ctx, o, interface::CARRIER);
            let absent = carrier.u8(ctx.b, 0x1c) == 0;
            if absent {
                return ctx.reinsert();
            }
            let sub_state = o.u8(ctx.b, field::SUB_STATE);
            if sub_state != 0 {
                return ctx.reinsert();
            }
            let number = o.u8(ctx.b, field::MODE) as u32;
            let word = carrier.u16(ctx.b, 8 + 2 * number);
            let mut d1: u8 = 0;
            if !(word == 0xffff) {
                d1 = 1;
            }
            let known = o.u8(ctx.b, 0x14);
            if !(d1 == known) {
                o.set_u8(ctx.b, 0x14, d1);
                // 0xae9f2
                let offset = if d1 == 0 { 0 } else { 0x400 };
                copy_palette(ctx, o, offset);
                // the routine chains onto the animation (0xaea0c), which sorts the object then comes back here
                interface_2_animation(ctx, o, d1 as u16);
            }
            let bits = 3u16 << ((number << 1) & 0x1f);
            let value = carrier.u16(ctx.b, 0x14) & bits;
            let old = o.u16(ctx.b, 0x1c);
            if !(value == old) {
                o.set_u16(ctx.b, 0x1c, value);
                let blinks = o.u8(ctx.b, 0x15) != 0;
                if !blinks {
                    o.set_u8(ctx.b, 0x15, 0xc);
                    crate::sound::play(ctx.b, interface::SOUND_GAUGE, ctx.obj());
                }
            }
            let remainder = o.u8(ctx.b, 0x15);
            if remainder == 0 {
                return ctx.reinsert();
            }
            let flags = o.u8(ctx.b, 0x2a);
            o.set_u8(ctx.b, 0x2a, flags & !1);
            let remainder = remainder.wrapping_sub(1);
            o.set_u8(ctx.b, 0x15, remainder);
            if !(remainder == 0) {
                if remainder & 2 == 0 {
                    return ctx.reinsert();
                }
                let flags = o.u8(ctx.b, 0x2a);
                o.set_u8(ctx.b, 0x2a, flags | 1);
                return ctx.reinsert();
            }
            let value = o.u16(ctx.b, 0x1c);
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
        _ => outside_table(target),
    }
}

/// End of the initialization of variant 2 (`0xaea0c`): window animation.
fn interface_2_animation<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, d1: u16) {
    let number = (o.u8(ctx.b, field::MODE) as u32) << 1;
    let animation = ctx.prog16(interface::ANIMATIONS_2 + number).wrapping_add(d1);
    ctx.anim_start(ANIMATIONS_INTERFACE, animation);
    ctx.reinsert();
}

fn interface_3<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    if game_finished(ctx) {
        return ctx.free(&pools::EFFCT);
    }
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, interface::STATES_3, state as u32);
    match state {
        state::INIT => {
            add_u8(ctx, o, Obj::STATE, 2);
            place_on_camera(ctx, o);
            shifted_u16(ctx, o, Obj::X, 0x10);
            shifted_u16(ctx, o, Obj::Z, 0xd8);
            interface_3_animation(ctx, o);
        }
        state::ACTIVE => {
            let (a, b) = (o.u8(ctx.b, 0x14), o.u8(ctx.b, 0x15));
            if !(a != b) {
                return ctx.reinsert();
            }
            interface_3_animation(ctx, o);
        }
        _ => outside_table(target),
    }
}

/// `0xaebfe`: animation `0xe3 - +0x14` of the interface set.
fn interface_3_animation<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let value = o.u8(ctx.b, 0x14);
    o.set_u8(ctx.b, 0x15, value);
    ctx.anim_start(ANIMATIONS_INTERFACE, 0xe3u8.wrapping_sub(value) as u16);
    ctx.reinsert();
}

fn interface_4<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, interface::STATES_4, state as u32);
    match state {
        state::INIT => {
            add_u8(ctx, o, Obj::STATE, 2);
            place_on_camera(ctx, o);
            let rank = (o.u8(ctx.b, field::MODE) as u32) << 3;
            let dx = ctx.prog16(interface::OFFSETS_4 + rank);
            shifted_u16(ctx, o, Obj::X, dx);
            let dz = ctx.prog16(interface::OFFSETS_4 + rank + 2);
            shifted_u16(ctx, o, Obj::Z, dz);
            let dy = ctx.prog16(interface::OFFSETS_4 + rank + 4);
            shifted_u16(ctx, o, 0x12, dy);
            let animation = ctx.prog16(interface::OFFSETS_4 + rank + 6);
            ctx.anim_start(ANIMATIONS_INTERFACE, animation);
            ctx.reinsert();
        }
        state::ACTIVE => {
            if game_finished(ctx) {
                return ctx.free(&pools::EFFCT);
            }
            ctx.reinsert();
        }
        state::FREED | state::DESTROYED => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

fn interface_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let target = target(ctx, interface::TABLE, 2 * variant as u32);
    match variant {
        0 => interface_0(ctx, o),
        1 => interface_1(ctx, o),
        2 => interface_2(ctx, o),
        3 => interface_3(ctx, o),
        4 => interface_4(ctx, o),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 29: impact spark on a target (0xb1440)

/// Type 29: spark stuck to a target (`+0x5c`, enemy or boss) at the impact point drawn from a table according to the
/// target's type; it plays an animation (drawn at random according to a flag `+0x42` read from a table per target type)
/// and disappears at the end of the animation. If the target no longer exists or does not fit, the spark goes directly
/// to state 6 (release).
mod impact {
    pub const ENTRY: u32 = 0xb1440;
    pub const TABLE: u32 = 0xb144e;
    pub const ANIMATIONS: u32 = 0x1908d6;
    pub const RANDOM_A: u32 = 0xb14a2;
    pub const RANDOM_B: u32 = 0xb14b2;
    /// Flags per enemy / boss type (two bytes: forbidden, flag).
    pub const ENEMIES: u32 = 0xb1544;
    pub const BOSS: u32 = 0xb1586;
    /// Impact offsets (dx, dz) per target type (4 bytes), for the enemies then for the rest.
    pub const OFFSETS_ENEMY: u32 = 0xb165a;
    pub const OFFSETS_OTHER: u32 = 0xb16de;
    pub const TARGET: u32 = 0x5c;
    pub const VARIANT_RANDOM: u32 = 0x42;
}

/// Sets state 6 (release on the next frame).
fn impact_abandon<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    o.set_u32(ctx.b, Obj::STATE, 0x0600_0000);
}

/// `0xb14dc`: reads the target's flag from the tables, or gives up.
fn impact_flag<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let pointer_target = o.u16(ctx.b, impact::TARGET);
    if pointer_target == 0 {
        return impact_abandon(ctx, o);
    }
    let target = pointer(ctx, o, impact::TARGET);
    let word = target.u16(ctx.b, 4);
    if word == 0x402 {
        return impact_abandon(ctx, o);
    }
    let class = target.u8(ctx.b, Obj::CLASS);
    let rank = (target.u8(ctx.b, Obj::KIND) as u32) * 2;
    if !(class != 2) {
        let forbidden = ctx.prog8(impact::ENEMIES + rank);
        if forbidden != 0 {
            return impact_abandon(ctx, o);
        }
        let flag = ctx.prog8(impact::ENEMIES + rank + 1);
        return o.set_u8(ctx.b, impact::VARIANT_RANDOM, flag);
    }
    if class != 4 {
        return impact_abandon(ctx, o);
    }
    let forbidden = ctx.prog8(impact::BOSS + rank);
    if forbidden != 0 {
        return impact_abandon(ctx, o);
    }
    let flag = ctx.prog8(impact::BOSS + rank + 1);
    o.set_u8(ctx.b, impact::VARIANT_RANDOM, flag);
}

/// `0xb15c8` (non-zero variant): position of the target offset by `(+0x14, +0x16)`.
fn impact_position_shifted<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let pointer_target = o.u16(ctx.b, impact::TARGET);
    if pointer_target == 0 {
        return;
    }
    let target = pointer(ctx, o, impact::TARGET);
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = target.u32(ctx.b, off);
        o.set_u32(ctx.b, off, v);
    }
    let flip = target.u8(ctx.b, field::FLIP);
    o.set_u8(ctx.b, field::FLIP, flip);
    let (dx, dz) = (o.u16(ctx.b, Obj::VX), o.u16(ctx.b, 0x16));
    shifted_u16(ctx, o, Obj::X, dx);
    shifted_u16(ctx, o, Obj::Z, dz);
}

/// `0xb15fe` (variant 0): position of the target offset by the impact drawn from the table.
fn impact_position_table<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let pointer_target = o.u16(ctx.b, impact::TARGET);
    if pointer_target == 0 {
        return;
    }
    let target = pointer(ctx, o, impact::TARGET);
    for off in [Obj::X, Obj::Y, Obj::Z] {
        let v = target.u32(ctx.b, off);
        o.set_u32(ctx.b, off, v);
    }
    let flip = target.u8(ctx.b, field::FLIP);
    o.set_u8(ctx.b, field::FLIP, flip);
    let class = target.u8(ctx.b, Obj::CLASS);
    let table = if class == 2 { impact::OFFSETS_ENEMY } else { impact::OFFSETS_OTHER };
    let rank = ((target.u8(ctx.b, Obj::KIND) << 2) as u32) & 0xff;
    let mut dx = ctx.data16(table + rank);
    let dz = ctx.data16(table + rank + 2);
    let turned = target.u8(ctx.b, field::FLIP) != 0;
    if turned {
        dx = dx.wrapping_neg();
    }
    shifted_u16(ctx, o, Obj::X, dx);
    shifted_u16(ctx, o, Obj::Z, dz);
}

fn impact_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if !(variant == 0) {
        impact_position_shifted(ctx, o);
        o.set_u8(ctx.b, impact::VARIANT_RANDOM, 0);
    } else {
        impact_position_table(ctx, o);
    }
    impact_flag(ctx, o);
    let rng = ctx.random();
    let draw = rng & 0xf;
    let flag = o.u8(ctx.b, impact::VARIANT_RANDOM);
    let table = if flag == 0 { impact::RANDOM_A } else { impact::RANDOM_B };
    let animation = ctx.prog8(table + draw as u32);
    ctx.anim_start(impact::ANIMATIONS, animation as u16);
}

fn impact_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let end = ctx.anim_step();
    if end {
        add_u8(ctx, o, Obj::STATE, 2);
    }
    ctx.reinsert();
}

fn impact_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, impact::TABLE, state as u32);
    match state {
        state::INIT => impact_init(ctx, o),
        state::ACTIVE => impact_active(ctx, o),
        state::FREED | 6 => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 6: burst of debris (0xab7fa)

/// Type 6: debris thrown from an owner (`+0x50`). The first object (`+0x42` zero) creates as many siblings as indicated
/// by a table (according to the variant) then becomes a debris itself; each debris takes the owner's position, reads
/// its speed, orientation, offset and animation from a ROM sheet (variant, mode `+0x2f`), rises and falls back
/// blinking, then disappears.
mod debris {
    pub const ENTRY: u32 = 0xab7fa;
    pub const TABLE: u32 = 0xab808;
    /// Number of siblings per variant (program space); ROM sheets (data).
    pub const NUMBERS: u32 = 0xab95e;
    pub const SHEETS: u32 = 0xab8a2;
    pub const ANIMATIONS: u32 = 0x18e664;
    pub const CHILD: u32 = 0x42;
    pub const BLINK: u32 = 0x53;
}

/// `0xab7d8`: creates a debris (type 6, variant `variant`) attached to the current object.
fn created_debris<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, variant: u8) -> Option<Obj> {
    let new = ctx.alloc(&pools::EFFCT);
    // two successive `bcs`: in `0xab7d8`, then in the caller (the carry stayed set)
    let n = new?;
    n.set_u8(ctx.b, Obj::ACTIVE, 1);
    n.set_u8(ctx.b, Obj::KIND, 6);
    n.set_u8(ctx.b, Obj::VARIANT, variant);
    n.set_u8(ctx.b, debris::CHILD, 0);
    n.set_u16(ctx.b, field::OWNER, o.0 as u16);
    Some(n)
}

/// First debris: creates its siblings, then goes to the common initialization.
fn debris_parent<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    o.set_u8(ctx.b, debris::CHILD, 1);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let count = ctx.prog8(debris::NUMBERS + variant as u32);
    o.set_u8(ctx.b, field::MODE, count.wrapping_add(1));
    let owner = o.u16(ctx.b, field::OWNER);
    for remaining in (0..=count as u16).rev() {
        let Some(sibling) = created_debris(ctx, o, variant) else {
            break;
        };
        sibling.set_u16(ctx.b, field::OWNER, owner);
        sibling.set_u8(ctx.b, debris::CHILD, 1);
        sibling.set_u8(ctx.b, field::MODE, remaining as u8);
    }
    debris_init(ctx, o);
}

fn debris_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let child = o.u8(ctx.b, debris::CHILD);
    if child == 0 {
        return debris_parent(ctx, o);
    }
    add_u8(ctx, o, Obj::STATE, 2);
    let owner = pointer(ctx, o, field::OWNER);
    for off in [Obj::X, Obj::Z, Obj::Y] {
        let v = owner.u32(ctx.b, off);
        o.set_u32(ctx.b, off, v);
    }
    o.set_u8(ctx.b, debris::BLINK, 0);
    o.set_u16(ctx.b, 0x16, 0);
    let variant = o.u8(ctx.b, Obj::VARIANT) as u32;
    let offset = ctx.data16(debris::SHEETS + 2 * variant) as i16 as i32 as u32;
    let mode = (o.u8(ctx.b, field::MODE) as u32) << 4;
    let mut sheet = debris::SHEETS.wrapping_add(offset).wrapping_add(mode);
    let vx = ctx.data16(sheet);
    o.set_u16(ctx.b, Obj::VX, vx);
    let vz_az = ctx.data32(sheet + 2);
    o.set_u32(ctx.b, Obj::VZ, vz_az);
    let flip = ctx.data8(sheet + 6);
    o.set_u8(ctx.b, field::FLIP, flip);
    let animation = ctx.data8(sheet + 7);
    let (mut dx, dz, dy) = (ctx.data16(sheet + 8), ctx.data16(sheet + 10), ctx.data16(sheet + 12));
    sheet += 14;
    let _ = sheet;
    let turned = owner.u8(ctx.b, field::FLIP) != 0;
    if turned {
        let flip = o.u8(ctx.b, field::FLIP) ^ 0x20;
        o.set_u8(ctx.b, field::FLIP, flip);
        let vx = o.u16(ctx.b, Obj::VX).wrapping_neg();
        o.set_u16(ctx.b, Obj::VX, vx);
        dx = dx.wrapping_neg();
        o.set_u8(ctx.b, debris::BLINK, 1);
    }
    shifted_u16(ctx, o, Obj::X, dx);
    shifted_u16(ctx, o, Obj::Z, dz);
    shifted_u16(ctx, o, 0x12, dy);
    ctx.anim_start(debris::ANIMATIONS, animation as u16);
}

fn debris_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    ctx.move_xz();
    ctx.anim_step();
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags & !2);
    let blinks = o.u8(ctx.b, debris::BLINK) ^ 1;
    o.set_u8(ctx.b, debris::BLINK, blinks);
    if !(blinks == 0) {
        let flags = o.u8(ctx.b, Obj::FLAGS);
        o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
    }
    let z = o.u16(ctx.b, Obj::Z);
    if !(z as i16 > 0) {
        o.set_u32(ctx.b, Obj::Z, 0);
        o.set_u8(ctx.b, debris::BLINK, 6);
        add_u8(ctx, o, Obj::STATE, 2);
        let variant = o.u8(ctx.b, Obj::VARIANT);
        if !(variant == 0) {
            add_u8(ctx, o, debris::BLINK, 0xff);
        }
    }
    ctx.reinsert();
}

fn debris_end<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags & !2);
    let remainder = o.u8(ctx.b, debris::BLINK).wrapping_sub(1);
    o.set_u8(ctx.b, debris::BLINK, remainder);
    if remainder == 0 {
        return ctx.free(&pools::EFFCT);
    }
    if !(remainder & 1 == 0) {
        let flags = o.u8(ctx.b, Obj::FLAGS);
        o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
    }
    ctx.reinsert();
}

fn debris_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, debris::TABLE, state as u32);
    match state {
        state::INIT => debris_init(ctx, o),
        state::ACTIVE => debris_active(ctx, o),
        state::FREED => debris_end(ctx, o),
        6 => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 1: removed effect (0xa8758)

/// Type 1: unused table slot in this version of the game: the handler frees the object immediately (`bra` to `jmp
/// 0x2218`).
fn removed<R: Bench>(ctx: &mut Cx<'_, R>) {
    ctx.free(&pools::EFFCT);
}

// ---------------------------------------------------------------------------------------------
// Type 7: single-animation burst (0xaba4e)

/// Type 7: plays the animation (drawn according to the mode `+0x2f`); at the end of the animation it rises by two units
/// in depth then blinks for 0x1e frames before disappearing. Not exercised.
mod shard_single {
    pub const ENTRY: u32 = 0xaba4e;
    pub const TABLE: u32 = 0xaba5c;
    pub const ANIMATIONS_MODE: u32 = 0xaba84;
    pub const COUNT: u32 = 0x53;
}

fn shard_single_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    let mode = o.u8(ctx.b, field::MODE) as u32;
    let animation = ctx.prog8(shard_single::ANIMATIONS_MODE + mode);
    ctx.anim_start(0x1bf8fe, animation as u16);
    ctx.reinsert();
}

fn shard_single_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let end = o.u8(ctx.b, 0x32);
    if !(end == 0) {
        add_u8(ctx, o, Obj::STATE, 2);
        o.set_u8(ctx.b, shard_single::COUNT, 0x1e);
        let y = o.u32(ctx.b, Obj::Y).wrapping_add(2);
        o.set_u32(ctx.b, Obj::Y, y);
    }
    ctx.anim_step();
}

fn shard_single_blinks<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let remainder = o.u8(ctx.b, shard_single::COUNT).wrapping_sub(1);
    o.set_u8(ctx.b, shard_single::COUNT, remainder);
    if remainder == 0 {
        return ctx.free(&pools::EFFCT);
    }
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags ^ 2);
}

fn shard_single_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, shard_single::TABLE, state as u32);
    match state {
        state::INIT => shard_single_init(ctx, o),
        state::ACTIVE => shard_single_active(ctx, o),
        state::FREED => shard_single_blinks(ctx, o),
        state::DESTROYED => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 19: screen sweep (0xaecce)

/// Type 19: object that enters from the left of the screen (x = -0x80, y = 0x150), waits 0x70 frames advancing two
/// pixels per frame, then slows down (acceleration -0x10) until it goes back. Not exercised.
mod sweep {
    pub const ENTRY: u32 = 0xaecce;
    pub const TABLE: u32 = 0xaecdc;
    pub const DELAY: u32 = 0x54;
}

fn sweep_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, sweep::DELAY, 0x70);
    o.set_u16(ctx.b, Obj::X, 0xff80);
    o.set_u16(ctx.b, Obj::Y, 0x150);
    o.set_u16(ctx.b, Obj::VX, 0x200);
    o.set_u16(ctx.b, Obj::AX, 0xfff0);
    ctx.anim_start(ANIMATIONS_INTERFACE, 0);
}

fn sweep_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
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

fn sweep_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, sweep::TABLE, sweep_init, sweep_active)
}

// ---------------------------------------------------------------------------------------------
// Type 27: rising pickup effect (0xb12e6)

/// Type 27: effect placed on the owner object (`+0x50`, created by `0xb12c0`: pickup) that rises by 0x8000 (half a
/// pixel) per frame for 0x1c frames. Not exercised.
mod smoke {
    pub const ENTRY: u32 = 0xb12e6;
    pub const TABLE: u32 = 0xb12f4;
    pub const ANIMATIONS: u32 = 0x19046c;
    pub const DURATION: u32 = 0x57;
}

fn smoke_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, smoke::DURATION, 0x1c);
    let owner = pointer(ctx, o, field::OWNER);
    let x = owner.u16(ctx.b, Obj::X);
    o.set_u16(ctx.b, Obj::X, x);
    let y = owner.u16(ctx.b, Obj::Y).wrapping_sub(1);
    o.set_u16(ctx.b, Obj::Y, y);
    o.set_u16(ctx.b, Obj::Z, 0x10);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    ctx.anim_start(smoke::ANIMATIONS, variant as u16);
    ctx.reinsert();
}

fn smoke_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let remainder = o.u8(ctx.b, smoke::DURATION).wrapping_sub(1);
    o.set_u8(ctx.b, smoke::DURATION, remainder);
    if !(remainder != 0) {
        add_u8(ctx, o, Obj::STATE, 2);
    }
    let z = o.u32(ctx.b, Obj::Z).wrapping_add(0x8000);
    o.set_u32(ctx.b, Obj::Z, z);
}

fn smoke_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, smoke::TABLE, smoke_init, smoke_active)
}

// ---------------------------------------------------------------------------------------------
// Type 28: spark placed on an object (0xb136e)

/// Type 28: spark stuck to an object (`+0x5c`), 0x20 above it; the animation depends on the variant and on `A5+0x70`.
/// At the end of the animation, it notifies its object (`+0x77` ← 1, except variant 6) then disappears. Not exercised.
mod spark {
    pub const ENTRY: u32 = 0xb136e;
    pub const TABLE: u32 = 0xb137c;
    pub const ANIMATIONS: u32 = 0x190534;
    pub const CARRIER: u32 = 0x5c;
}

fn spark_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    let carrier = pointer(ctx, o, spark::CARRIER);
    copy_position(ctx, o, carrier);
    let y = o.u16(ctx.b, Obj::Y).wrapping_sub(1);
    o.set_u16(ctx.b, Obj::Y, y);
    shifted_u16(ctx, o, Obj::Z, 0x20);
    let variant = o.u8(ctx.b, Obj::VARIANT) as u16;
    let word = ctx.ram16(0x70);
    let animation = variant.wrapping_add((word & 0xf) >> 1 << 3);
    ctx.anim_start(spark::ANIMATIONS, animation);
    ctx.reinsert();
}

fn spark_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
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
    carrier.set_u8(ctx.b, 0x77, 1);
}

fn spark_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, spark::TABLE, state as u32);
    match state {
        state::INIT => spark_init(ctx, o),
        state::ACTIVE => spark_active(ctx, o),
        state::FREED => add_u8(ctx, o, Obj::STATE, 2),
        6 => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 57: mark attached to a player (0xb6bca)

/// Type 57: mark placed 0x18 to the right and 0x2d above the owner (`+0x50`) as long as it stays in state `0x0204` /
/// `+6 = 0x2c`; otherwise it disappears. Not exercised.
mod mark {
    pub const ENTRY: u32 = 0xb6bca;
    pub const TABLE: u32 = 0xb6bd8;
    pub const ANIMATIONS: u32 = 0x196b62;
}

fn mark_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    let owner = pointer(ctx, o, field::OWNER);
    for off in [Obj::X, Obj::Z] {
        let v = owner.u16(ctx.b, off);
        o.set_u16(ctx.b, off, v);
    }
    let y = owner.u32(ctx.b, Obj::Y).wrapping_sub(1);
    o.set_u32(ctx.b, Obj::Y, y);
    shifted_u16(ctx, o, Obj::X, 0x18);
    shifted_u16(ctx, o, Obj::Z, 0x2d);
    ctx.anim_start(mark::ANIMATIONS, 0);
    ctx.reinsert();
}

fn mark_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let owner = pointer(ctx, o, field::OWNER);
    let state = owner.u16(ctx.b, 4);
    if state != 0x204 {
        return ctx.free(&pools::EFFCT);
    }
    let sub = owner.u8(ctx.b, 6);
    if sub != 0x2c {
        return ctx.free(&pools::EFFCT);
    }
    ctx.anim_step();
}

fn mark_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, mark::TABLE, state as u32);
    match state {
        state::INIT => mark_init(ctx, o),
        state::ACTIVE => mark_active(ctx, o),
        state::FREED => ctx.free(&pools::EFFCT),
        state::DESTROYED => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 62: burst of fragments (0xb77ac)

/// Type 62: takes the variant and mode of the object it follows (`+0x5c`), places itself 0x48 to the left of it and
/// waits for it to be hit (`+0x33` non-zero); it then explodes into nine EFFCT 0x40 objects (and a tenth described by
/// the mode) then disappears. Not exercised.
mod sheaf {
    pub const ENTRY: u32 = 0xb77ac;
    pub const TABLE: u32 = 0xb77ba;
    pub const ANIMATIONS: u32 = 0x197a5c;
    pub const CARRIER: u32 = 0x5c;
}

fn sheaf_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    let carrier = pointer(ctx, o, sheaf::CARRIER);
    let variant = carrier.u8(ctx.b, Obj::VARIANT);
    o.set_u8(ctx.b, Obj::VARIANT, variant);
    let mode = carrier.u8(ctx.b, field::MODE);
    o.set_u8(ctx.b, field::MODE, mode);
    for off in [Obj::X, Obj::Y] {
        let v = carrier.u16(ctx.b, off);
        o.set_u16(ctx.b, off, v);
    }
    let x = o.u16(ctx.b, Obj::X).wrapping_sub(0x48);
    o.set_u16(ctx.b, Obj::X, x);
    o.set_u16(ctx.b, Obj::Z, 0x40);
    ctx.anim_start(sheaf::ANIMATIONS, 0);
    o.set_u8(ctx.b, 0x3b, 0);
    ctx.reinsert();
}

/// `0xb7810`: the explosion.
fn sheaf_explodes<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let mut last = None;
    for _ in 0..9 {
        let new = ctx.alloc(&pools::EFFCT);
        if new.is_none() {
            return ctx.free(&pools::EFFCT);
        }
        let n = new.unwrap();
        n.set_u8(ctx.b, Obj::ACTIVE, 1);
        n.set_u8(ctx.b, Obj::KIND, 0x40);
        for off in [Obj::X, Obj::Y] {
            let v = o.u16(ctx.b, off);
            n.set_u16(ctx.b, off, v);
        }
        last = Some(n);
    }
    let mode = o.u8(ctx.b, field::MODE);
    if !(mode == 0) {
        // the last object created is reused to carry the mode
        let n = last.unwrap();
        n.set_u8(ctx.b, Obj::ACTIVE, 1);
        n.set_u8(ctx.b, Obj::KIND, 0x40);
        n.set_u8(ctx.b, Obj::VARIANT, 1);
        let variant = o.u8(ctx.b, Obj::VARIANT);
        n.set_u8(ctx.b, field::MODE, variant);
        for off in [Obj::X, Obj::Y] {
            let v = o.u16(ctx.b, off);
            n.set_u16(ctx.b, off, v);
        }
    }
    ctx.free(&pools::EFFCT);
}

fn sheaf_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, sheaf::TABLE, state as u32);
    match state {
        state::INIT => sheaf_init(ctx, o),
        state::ACTIVE => {
            let carrier = pointer(ctx, o, sheaf::CARRIER);
            let key = carrier.u8(ctx.b, 0x33);
            if key == 0 {
                return;
            }
            sheaf_explodes(ctx, o);
        }
        state::FREED => sheaf_explodes(ctx, o),
        state::DESTROYED => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 56: debris fragments (0xb6c6e)

/// Type 56: burst thrown into the air from its position (random scatter of ±128 in X and ±32 in depth, speeds drawn
/// from two tables, animation 0x164 to 0x167); disappears when it falls back below the ground. Not exercised.
mod shards_debris {
    pub const ENTRY: u32 = 0xb6c6e;
    pub const TABLE: u32 = 0xb6c7c;
    pub const SPEEDS_X: u32 = 0xb6d18;
    pub const SPEEDS_Y: u32 = 0xb6d28;
}

/// Masked random draw.
fn draw<R: Bench>(ctx: &mut Cx<'_, R>, mask: u16) -> u16 {
    let rng = ctx.random();
    rng & mask
}

fn shards_debris_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    o.set_u16(ctx.b, 0x42, 0x40);
    o.set_u8(ctx.b, 0x2a, 1);
    o.set_u8(ctx.b, 0x2b, 0x1d);
    let flip = draw(ctx, 0x20);
    o.set_u8(ctx.b, field::FLIP, flip as u8);
    let dx = draw(ctx, 0xff).wrapping_sub(0x80);
    shifted_u16(ctx, o, Obj::X, dx);
    let dy = draw(ctx, 0x3f).wrapping_sub(0x20);
    shifted_u16(ctx, o, Obj::Y, dy);
    o.set_u16(ctx.b, Obj::VZ, 0x400);
    o.set_u16(ctx.b, Obj::AZ, 0xffc0);
    let r = draw(ctx, 0x1c) as u32;
    let vx = ctx.prog16(shards_debris::SPEEDS_X + r);
    o.set_u16(ctx.b, Obj::VX, vx);
    let r = draw(ctx, 0x1c) as u32;
    let vy = ctx.prog16(shards_debris::SPEEDS_Y + r);
    o.set_u16(ctx.b, Obj::VY, vy);
    let animation = draw(ctx, 3).wrapping_add(0x164);
    ctx.anim_start(ANIMATIONS_INTERFACE, animation);
    ctx.reinsert();
}

fn shards_debris_active<R: Bench>(ctx: &mut Cx<'_, R>, _o: Obj) {
    let positive = ctx.move_xyz().pl(); // bpl
    if positive {
        return;
    }
    ctx.free(&pools::EFFCT);
}

fn shards_debris_end<R: Bench>(ctx: &mut Cx<'_, R>, _o: Obj) {
    ctx.free(&pools::EFFCT);
}

fn shards_debris_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, shards_debris::TABLE, state as u32);
    match state {
        state::INIT => shards_debris_init(ctx, o),
        state::ACTIVE => shards_debris_active(ctx, o),
        state::FREED => shards_debris_end(ctx, o),
        state::DESTROYED => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 51: projectile thrown from an object (0xb64c8)

/// Type 51: born on the object `+0x5c` (if that object disappears, it goes to state 6), offset by 0x84 pixels forward
/// and 0x16 in depth, then leaves in an arc (speeds 0x140 and 0x3c0, gravity 0x38) until it touches the ground. Not
/// exercised.
mod projectile {
    pub const ENTRY: u32 = 0xb64c8;
    pub const TABLE: u32 = 0xb64de;
    pub const SUB_STATES: u32 = 0xb6572;
    pub const ANIMATIONS: u32 = 0x1733ac;
    pub const TARGET: u32 = 0x5c;
}

fn projectile_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
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
    ctx.anim_start(projectile::ANIMATIONS, 0x19);
    ctx.reinsert();
}

fn projectile_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    let target = target(ctx, projectile::SUB_STATES, sub_state as u32);
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
        _ => outside_table(target),
    }
}

fn projectile_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let pointer_target = o.u16(ctx.b, projectile::TARGET);
    if pointer_target == 0 {
        o.set_u32(ctx.b, Obj::STATE, 0x0600_0000);
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, projectile::TABLE, state as u32);
    match state {
        state::INIT => projectile_init(ctx, o),
        state::ACTIVE => projectile_active(ctx, o),
        state::FREED | 6 => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 52: sparkle of a character (0xb65e0)

/// Type 52: variant of the sparkle (type 15) that only watches the players of class 5 still alive (non-zero hit points
/// `+0x62`); once triggered it blinks eight times, hidden as long as its object (`+0x5c`) is. Not exercised.
mod shimmer_class {
    pub const ENTRY: u32 = 0xb65e0;
    pub const TABLE: u32 = 0xb65ee;
    pub const ANIMATIONS: u32 = 0x1bc77c;
    pub const REPETITIONS: u32 = 0x57;
    pub const DELAY: u32 = 0x58;
    pub const TOGGLE: u32 = 0x5a;
    pub const TRIGGERS: u32 = 0x5b;
    pub const CARRIER: u32 = 0x5c;
}

fn shimmer_class_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
    o.set_u8(ctx.b, shimmer_class::REPETITIONS, 8);
    let t = draw(ctx, 0x1f);
    o.set_u16(ctx.b, shimmer_class::DELAY, t);
    ctx.anim_start(shimmer_class::ANIMATIONS, 0);
    ctx.reinsert();
}

/// `0xb668a`: has a living class 5 player passed near the object?
fn shimmer_class_player<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    for i in 0..4u32 {
        let player = Obj(PLAYERS + i * STEP_PLAYER);
        let absent = player.u8(ctx.b, Obj::ACTIVE) == 0;
        if !absent {
            let dead = player.u16(ctx.b, 0x62) == 0;
            if !dead {
                let class = player.u8(ctx.b, 3) >> 1;
                if !(class != 5) {
                    let dy = o.u16(ctx.b, Obj::Y).wrapping_sub(player.u16(ctx.b, Obj::Y)).wrapping_add(0x20);
                    if !(dy > 0x40) {
                        let dx = o.u16(ctx.b, Obj::X).wrapping_sub(player.u16(ctx.b, Obj::X)).wrapping_add(0x60);
                        if !(dx > 0xc0) {
                            o.set_u8(ctx.b, shimmer_class::TRIGGERS, 1);
                            return;
                        }
                    }
                }
            }
        }
    }
}

fn shimmer_class_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let carrier = pointer(ctx, o, shimmer_class::CARRIER);
    let state_carrier = carrier.u16(ctx.b, 4);
    if !(state_carrier == 0x200) {
        add_u8(ctx, o, Obj::STATE, 2);
    }
    let triggers = o.u8(ctx.b, shimmer_class::TRIGGERS);
    if !(triggers != 0) {
        return shimmer_class_player(ctx, o);
    }
    let cache = carrier.u8(ctx.b, Obj::HIDDEN);
    if !(cache == 0) {
        let flags = o.u8(ctx.b, Obj::FLAGS);
        o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
        return;
    }
    let remainder = o.u16(ctx.b, shimmer_class::DELAY).wrapping_sub(1);
    o.set_u16(ctx.b, shimmer_class::DELAY, remainder);
    if remainder != 0 {
        return;
    }
    o.set_u16(ctx.b, shimmer_class::DELAY, 0x30);
    let toggle = o.u8(ctx.b, shimmer_class::TOGGLE) ^ 1;
    o.set_u8(ctx.b, shimmer_class::TOGGLE, toggle);
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags & !2);
    if toggle != 0 {
        return;
    }
    let flags = o.u8(ctx.b, Obj::FLAGS);
    o.set_u8(ctx.b, Obj::FLAGS, flags | 2);
    let remainder = o.u8(ctx.b, shimmer_class::REPETITIONS).wrapping_sub(1);
    o.set_u8(ctx.b, shimmer_class::REPETITIONS, remainder);
    if remainder != 0 {
        return;
    }
    o.set_u32(ctx.b, Obj::STATE, 0x0400_0000);
}

fn shimmer_class_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, shimmer_class::TABLE, shimmer_class_init, shimmer_class_active)
}

// ---------------------------------------------------------------------------------------------
// Type 24: character accessory (0xb0ff2)

/// Type 24: accessory stuck to its owner (`+0x50`) at an offset (dx, dy, dz) read according to the variant; active only
/// when the scene is not frozen (`A5+0x3d`). If the owner has disappeared it goes to state 6. Not exercised.
mod accessory {
    pub const ENTRY: u32 = 0xb0ff2;
    pub const TABLE: u32 = 0xb1000;
    pub const OFFSETS: u32 = 0xb10c0;
    pub const ANIMATIONS: u32 = 0x16e752;
    pub const END_ANIMATION: u32 = 0x43;
}

/// `0xb1052`: sticks the object back to the owner.
fn accessory_follow<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let pointer_owner = o.u16(ctx.b, field::OWNER);
    if !(pointer_owner != 0) {
        o.set_u32(ctx.b, Obj::STATE, 0x0600_0000);
        return;
    }
    let owner = pointer(ctx, o, field::OWNER);
    for off in [Obj::X, Obj::Y, Obj::Z] {
        let v = owner.u32(ctx.b, off);
        o.set_u32(ctx.b, off, v);
    }
    let flip = owner.u8(ctx.b, field::FLIP);
    o.set_u8(ctx.b, field::FLIP, flip);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let rank = (variant << 3) as u32; // one-byte offset, like the original
    let mut dx = ctx.prog16(accessory::OFFSETS + rank);
    let dy = ctx.prog16(accessory::OFFSETS + rank + 2);
    let ddepth = ctx.prog16(accessory::OFFSETS + rank + 4) as i16 as i32;
    let dz = ctx.prog16(accessory::OFFSETS + rank + 6);
    if !(flip == 0) {
        dx = dx.wrapping_neg();
    }
    shifted_u16(ctx, o, Obj::X, dx);
    shifted_u16(ctx, o, Obj::Y, dy);
    let y = o.u32(ctx.b, Obj::Y).wrapping_add(ddepth as u32);
    o.set_u32(ctx.b, Obj::Y, y);
    shifted_u16(ctx, o, Obj::Z, dz);
}

fn accessory_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, accessory::TABLE, state as u32);
    match state {
        state::INIT => {
            let frozen = ctx.ram8(0x3d) != 0;
            if frozen {
                return;
            }
            add_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, accessory::END_ANIMATION, 0);
            accessory_follow(ctx, o);
            ctx.anim_start(accessory::ANIMATIONS, 3);
        }
        state::ACTIVE => {
            let frozen = ctx.ram8(0x3d) != 0;
            if frozen {
                return;
            }
            let end = o.u8(ctx.b, accessory::END_ANIMATION);
            if !(end == 0) {
                add_u8(ctx, o, Obj::STATE, 2);
            }
            accessory_follow(ctx, o);
            ctx.anim_step();
            ctx.reinsert();
        }
        state::FREED => ctx.free(&pools::EFFCT),
        state::DESTROYED => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 45: proximity trigger (0xb5340)

/// Type 45: object placed in the scenery that waits for the main player (`A5+0x46`) to pass over it: it then gives an
/// event to each present player (`0x19b08`, offset per player) then disappears; it also disappears if it stays behind
/// the camera. Not exercised.
mod trigger {
    pub const ENTRY: u32 = 0xb5340;
    pub const TABLE: u32 = 0xb534e;
    pub const ANIMATIONS: u32 = 0x1bf5c2;
    pub const PRINCIPAL: i32 = 0x46;
    pub const EFFECT: u32 = 0xea50c;
    pub const OFFSETS: u32 = 0xb53ec;
}

/// `0xb53b0`: trigger.
fn trigger_effect<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    crate::sound::play(ctx.b, trigger::EFFECT, ctx.obj());
    let level = ctx.ram8(0x35) as u32;
    let mut offset = ctx.prog8(trigger::OFFSETS + level) as u16;
    for i in 0..4u32 {
        let player = Obj(PLAYERS + i * STEP_PLAYER);
        let absent = player.u8(ctx.b, Obj::ACTIVE) == 0;
        if !absent {
            let variant = o.u8(ctx.b, Obj::VARIANT) as u32;
            crate::events::enqueue_counter(ctx.b, (variant as u16).wrapping_add(offset as u16));
        }
        offset = offset.wrapping_add(0x100);
    }
    ctx.free(&pools::EFFCT);
}

fn trigger_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let limit = crate::ram::CAMERA_X.get(ctx.b).wrapping_sub(0x60);
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

fn trigger_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    ctx.anim_start(trigger::ANIMATIONS, variant as u16);
    ctx.reinsert();
}

fn trigger_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, trigger::TABLE, state as u32);
    match state {
        state::INIT => trigger_init(ctx, o),
        state::ACTIVE => trigger_active(ctx, o),
        state::FREED => trigger_effect(ctx, o),
        state::DESTROYED => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 4: action bubble of a character (0xaaac0)

/// Type 4: bubble (animated icon) linked to a player (`+0x50`) or to the object it handles (`+0x54`). The animation is
/// chosen from the carrier's current action (`+0x3d` of the player, `+0x10d/0x10e`, character `+3`) in small ROM
/// tables; eight variants:
///
/// - 0: player action (`+0x3d` from 0 to 32; beyond that, the original's table jumps into data);
/// - 1, 2, 5, 6: action of the carried object (`+0x54`) or fixed (`0x10`, `0x25` to `0x27`);
/// - 3: secondary bubble that settles on the carried object (`0xaadba`);
/// - 4, 7: fixed animations (`0x23`, `4`).
///
/// State 2 animates the bubble (variant 3: makes it blink and sticks it back to its carrier); state 4 frees; a table
/// code equal to `0x29` spawns a bubble of variant 3.
mod bubble {
    pub const ENTRY: u32 = 0xaaac0;
    pub const STATES: u32 = 0xaaace;
    pub const VARIANTS: u32 = 0xaaaee;
    pub const ANIMATIONS: u32 = 0x18d53e;
    // jump tables according to the action (`+0x3d`)
    pub const ACTIONS_0: u32 = 0xaab0e;
    pub const ACTIONS_1: u32 = 0xaab40;
    pub const ACTIONS_2: u32 = 0xaab5e;
    pub const ACTIONS_5: u32 = 0xaab84;
    pub const ACTIONS_6: u32 = 0xaab9c;
    // animation tables (data)
    pub const TABLE_77: u32 = 0xab4de;
    pub const TABLE_HIT: u32 = 0xab4f6;
    pub const TABLE_CHARACTER: u32 = 0xab4f0;
    pub const TABLE_OBJECT_A: u32 = 0xab4e4;
    pub const TABLE_OBJECT_B: u32 = 0xab4ea;
    pub const TABLE_10D_A: u32 = 0xab676;
    pub const TABLE_10D_B: u32 = 0xab6a6;
    pub const TABLE_10D_C: u32 = 0xab6d6;
    /// Offsets of variant 3 (according to the carrier's class, `+0x43` and `+0x86` of the player).
    pub const OFFSETS_3: u32 = 0xaae2a;
    pub const OWNER: u32 = 0x50;
    pub const DOOR: u32 = 0x54;
    pub const END_ANIMATION: u32 = 0x43;
    pub const HIT: u32 = 0x33;
    pub const COUNTDOWN_TIMER: u32 = 0x42;
    pub const PALETTE_GAUGE: u32 = 0x91c41c;
}

/// `0xaac00`: displays animation `number` of the bubble set, then sorts the object.
fn bubble_shown<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, number: u8) {
    if !(number != 0x2b) {
        o.set_u16(ctx.b, 0x2a, 0x115);
    }
    ctx.anim_start(bubble::ANIMATIONS, number as u16);
    ctx.reinsert();
}

/// `0xab26a`: a `0x29` code spawns a bubble of variant 3 (or, if the carrier is a BOX, plays animation 4).
fn bubble_created_next<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let door = pointer(ctx, o, bubble::DOOR);
    let class = door.u8(ctx.b, Obj::CLASS);
    if !(class != 0xa) {
        return bubble_shown(ctx, o, 4);
    }
    o.set_u32(ctx.b, Obj::STATE, 0);
    o.set_u8(ctx.b, Obj::VARIANT, 3);
    o.set_u8(ctx.b, bubble::END_ANIMATION, 0);
    let new = ctx.alloc(&pools::EFFCT);
    if new.is_none() {
        return;
    }
    let n = new.unwrap();
    n.set_u8(ctx.b, Obj::ACTIVE, 1);
    n.set_u8(ctx.b, Obj::KIND, 4);
    let p = o.u16(ctx.b, bubble::DOOR);
    n.set_u16(ctx.b, bubble::DOOR, p);
    n.set_u8(ctx.b, Obj::VARIANT, 3);
    n.set_u8(ctx.b, bubble::END_ANIMATION, 1);
}

/// `0xaabf4`: reads the animation code in the table `table` (data) at index `index`; code `0x29` creates the
/// continuation, the others are displayed.
fn bubble_reads<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, table: u32, index: u16) {
    let code = ctx.data8(table + index as u32);
    if code == 0x29 {
        return bubble_created_next(ctx, o);
    }
    bubble_shown(ctx, o, code);
}

/// The original jumps into another routine's epilogue (`movem.l (SP)+,{A1-A4} ; rts`, 0xaa9fe): the next four return
/// addresses are popped into A1-A4, and the return goes to the fifth. These are the callers' returns: the bubble goes
/// straight back to the top of the game loop (`0x1c468`), over the EFFCT dispatch, the world update, the end of the
/// phase and the end of the frame. Here they are the task's four most recent pending calls; the handler's return goes
/// to the fifth, and the overtaken Rust functions (dispatch loop...) give up (`tasks::table::returned`). Nobody reads
/// A1-A4 (those addresses): the task resumes in Rust, without registers.
/// Original glitch: the rest of the frame (next objects, sprites, collisions, end of frame) is skipped.
fn foreign_epilogue<R>(ctx: &mut Cx<'_, R>) {
    crate::tasks::table::pop_calls(ctx.b, 4);
}

/// `0xaace8`: common continuation. For character 1 the original jumps into the epilogue of another routine (`movem.l
/// (SP)+,{A1-A4} ; rts`): the effect is reproduced on the stack.
fn bubble_next<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let owner = pointer(ctx, o, bubble::OWNER);
    let character = (owner.u8(ctx.b, 3) >> 1).wrapping_sub(1);
    if character == 0 {
        return foreign_epilogue(ctx);
    }
    create_impact(ctx, o);
}

/// `0xb13fa`: creates the impact spark (type 29, variant 0) on the carried object (`+0x54`). For character 1, the
/// caller jumps into the epilogue `0xaa9fe` of another routine (see `foreign_epilogue`).
fn create_impact<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    if let Some(n) = ctx.alloc(&pools::EFFCT) {
        n.set_u8(ctx.b, Obj::ACTIVE, 1);
        n.set_u8(ctx.b, Obj::KIND, 0x1d);
        n.set_u8(ctx.b, Obj::VARIANT, 0);
        let door = o.u16(ctx.b, bubble::DOOR);
        n.set_u16(ctx.b, 0x5c, door);
    }
}

/// `0xaacfe`: about one time out of sixteen, draws at random whether to continue (`0x5555`: even bits).
fn bubble_random<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let rng = ctx.random();
    let draw = rng & 0xf;
    if 0x5555u16 >> draw & 1 != 0 {
        bubble_next(ctx, o);
    }
}

/// Common cases of variant 0: `0xaabc4` (byte `+0x10d` or `+0x10e` of the player in D0).
fn bubble_0_common<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, player: Obj, d0: u8) {
    let class = player.u8(ctx.b, 3) >> 1;
    let mark = player.u8(ctx.b, 0x77);
    if !(mark == 0) {
        player.set_u8(ctx.b, 0x77, 0);
        bubble_reads(ctx, o, bubble::TABLE_77, class as u16);
        return bubble_random(ctx, o);
    }
    let index = ((d0 as u16) << 3).wrapping_add(class as u16);
    bubble_reads(ctx, o, bubble::TABLE_HIT, index);
    bubble_random(ctx, o);
}

/// `0xaabac`: animation according to `+0x10d` (or `+0x10e` for character 4).
fn bubble_0_action_10d<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, player: Obj) {
    let mut d0 = player.u8(ctx.b, 0x10d);
    let class = player.u8(ctx.b, 3) >> 1;
    if !(class != 4) {
        d0 = player.u8(ctx.b, 0x10e);
    }
    bubble_0_common(ctx, o, player, d0);
}

/// Tables of variant 0: the player's action `0x3d` leads to one of the ten routines.
fn bubble_0<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let player = pointer(ctx, o, bubble::OWNER);
    let action = player.u8(ctx.b, 0x3d);
    let target = target(ctx, bubble::ACTIONS_0, action as u32);
    let byte = |ctx: &mut Cx<'_, R>, off: u32| player.u8(ctx.b, off) as u16;
    let class = player.u8(ctx.b, 3) >> 1;
    match action {
        0 => bubble_0_action_10d(ctx, o, player),
        2 => {
            let d0 = player.u8(ctx.b, 0x10e);
            bubble_0_common(ctx, o, player, d0);
        }
        4 => {
            let index = byte(ctx, 0x10d);
            bubble_reads(ctx, o, bubble::TABLE_10D_A, index);
        }
        6 => {
            let index = byte(ctx, 0x10e);
            bubble_reads(ctx, o, bubble::TABLE_10D_A, index);
        }
        8 => {
            let index = byte(ctx, 0x10d);
            bubble_reads(ctx, o, bubble::TABLE_10D_C, index);
        }
        10 => {
            let index = byte(ctx, 0x10e);
            bubble_reads(ctx, o, bubble::TABLE_10D_C, index);
        }
        12 => {
            let mut index = byte(ctx, 0x10d);
            if !(class != 4) {
                index = byte(ctx, 0x10e);
            }
            bubble_reads(ctx, o, bubble::TABLE_10D_B, index);
        }
        14 => bubble_reads(ctx, o, bubble::TABLE_CHARACTER, class as u16),
        16 => {
            let door = pointer(ctx, o, bubble::DOOR);
            let is_box = door.u8(ctx.b, Obj::CLASS) == 0xa;
            if is_box {
                return bubble_0_action_10d(ctx, o, player);
            }
            let gauge_inactive = ctx.ram16(0x4be) != 0;
            if !gauge_inactive {
                crate::mem::wr16(ctx.b, bubble::PALETTE_GAUGE, 0xf0ff);
            }
            bubble_reads(ctx, o, bubble::TABLE_OBJECT_A, class as u16);
            bubble_next(ctx, o);
        }
        // 0xaacba (actions 18 and 20 share the same code)
        18 | 20 => {
            let door = pointer(ctx, o, bubble::DOOR);
            let is_box = door.u8(ctx.b, Obj::CLASS) == 0xa;
            if is_box {
                return bubble_0_action_10d(ctx, o, player);
            }
            let gauge_inactive = ctx.ram16(0x4be) != 0;
            if !gauge_inactive {
                crate::mem::wr16(ctx.b, bubble::PALETTE_GAUGE, 0xf0ff);
            }
            bubble_reads(ctx, o, bubble::TABLE_OBJECT_B, class as u16);
            bubble_next(ctx, o);
        }
        // 0xaac82
        22 => bubble_shown(ctx, o, 0x23),
        // 0xaad12: the player's gauge (`+0x77`) has just been filled
        24 | 26 | 28 | 30 | 32 => {
            let gauge = player.u8(ctx.b, 0x77) != 0;
            if gauge {
                player.set_u8(ctx.b, 0x77, 0);
                o.set_u8(ctx.b, field::MODE, 1);
                crate::mem::wr16(ctx.b, bubble::PALETTE_GAUGE, 0xff00);
                return bubble_shown(ctx, o, 0x24);
            }
            bubble_shown(ctx, o, 0x10);
        }
        _ => outside_table(target),
    }
}

/// Variants 1, 2, 5 and 6: the action `0x3d` of the carried object (`+0x54`; player for variant 1) chooses a fixed
/// animation.
fn bubble_by_action<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj, offset_pointer: u32, doubler: bool, table: u32, count: u32) {
    let source = pointer(ctx, o, offset_pointer);
    let action = source.u8(ctx.b, 0x3d) as u32;
    let index = if doubler { action * 2 } else { action };
    let target = target(ctx, table, index);
    if index >= count || (!doubler && index & 1 != 0) {
        // an action outside the table would lead the original into the code that follows it
        outside_table(target);
    }
    let code = match (table, index / if doubler { 2 } else { 1 }) {
        (bubble::ACTIONS_1, _) => 0x10,
        (_, 0) => 0x25,
        (_, 1) => 0x26,
        _ => 0x27,
    };
    bubble_shown(ctx, o, code);
}

/// Variant 3 (`0xaadba`): sticks the bubble back to its carrier.
fn bubble_3_reattach<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let door = pointer(ctx, o, bubble::DOOR);
    let mut d0: u8 = 0;
    let class = door.u8(ctx.b, Obj::CLASS);
    if !(class == 2) {
        d0 = 0x20;
    }
    d0 = d0.wrapping_add(door.u8(ctx.b, Obj::KIND));
    let flip = door.u8(ctx.b, field::FLIP) ^ 0x20;
    o.set_u8(ctx.b, field::FLIP, flip);
    for off in [Obj::X, Obj::Z] {
        let v = door.u16(ctx.b, off);
        o.set_u16(ctx.b, off, v);
    }
    let y = door.u32(ctx.b, Obj::Y).wrapping_sub(1);
    o.set_u32(ctx.b, Obj::Y, y);
    let next = (o.u8(ctx.b, bubble::END_ANIMATION) & 1) as u16;
    let base = bubble::OFFSETS_3 + (((d0 as u16) << 4).wrapping_add(next << 3)) as u32;
    let index = ((door.u8(ctx.b, 0x86) as u32) << 2) & 0xffff;
    let mut dx = ctx.data16(base + index);
    let dz = ctx.data16(base + index + 2);
    shifted_u16(ctx, o, Obj::Z, dz);
    if !(flip != 0) {
        dx = dx.wrapping_neg();
    }
    shifted_u16(ctx, o, Obj::X, dx);
}

fn bubble_3_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let end = o.u8(ctx.b, bubble::END_ANIMATION);
    let number = if end == 0 { 0x29 } else { 0x2a };
    ctx.anim_start(bubble::ANIMATIONS, number);
    o.set_u8(ctx.b, bubble::COUNTDOWN_TIMER, 0x1e);
    bubble_3_reattach(ctx, o);
}

fn bubble_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    add_u8(ctx, o, Obj::STATE, 2);
    o.set_u8(ctx.b, field::MODE, 0);
    let variant = o.u8(ctx.b, Obj::VARIANT);
    let target = target(ctx, bubble::VARIANTS, 2 * variant as u32);
    match variant {
        0 => bubble_0(ctx, o),
        1 => bubble_by_action(ctx, o, bubble::OWNER, false, bubble::ACTIONS_1, 6),
        2 => bubble_by_action(ctx, o, bubble::DOOR, true, bubble::ACTIONS_2, 20),
        3 => bubble_3_init(ctx, o),
        4 => bubble_shown(ctx, o, 0x23),
        5 => bubble_by_action(ctx, o, bubble::DOOR, true, bubble::ACTIONS_5, 6),
        6 => bubble_by_action(ctx, o, bubble::DOOR, true, bubble::ACTIONS_6, 6),
        7 => bubble_shown(ctx, o, 4),
        _ => outside_table(target),
    }
}

fn bubble_active<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT);
    if variant == 3 {
        // variant 3: blinks, sticks back, and goes to the next state after `COUNTDOWN_TIMER` frames
        let flags = o.u8(ctx.b, Obj::FLAGS);
        o.set_u8(ctx.b, Obj::FLAGS, flags ^ 2);
        ctx.anim_step();
        bubble_3_reattach(ctx, o);
        let remainder = o.u8(ctx.b, bubble::COUNTDOWN_TIMER).wrapping_sub(1);
        o.set_u8(ctx.b, bubble::COUNTDOWN_TIMER, remainder);
        if !(remainder != 0) {
            add_u8(ctx, o, Obj::STATE, 2);
        }
        return ctx.reinsert();
    }
    let end = ctx.anim_step();
    if end {
        add_u8(ctx, o, Obj::STATE, 2);
        return;
    }
    // the background scroll gauge follows the press of the button linked to the hit (`+0x33`)
    let mut value = 0u16;
    let hit = o.u8(ctx.b, bubble::HIT);
    if !(hit == 0) {
        let gauge = ctx.ram16(0x4be) != 0;
        if !gauge {
            let blocked = ctx.ram8(0x4f) != 0;
            if !blocked {
                value = 0x100;
            }
        }
    }
    let address = crate::mem::A5.wrapping_add(0x4ba);
    crate::mem::wr16(ctx.b, address, value);
    ctx.reinsert();
}

/// Release (`0xab24c`): gives back the gauge palette if the bubble modified it.
fn bubble_end<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let modified = o.u8(ctx.b, field::MODE);
    if !(modified == 0) {
        let gauge = ctx.ram16(0x4be) != 0;
        if !gauge {
            crate::mem::wr16(ctx.b, bubble::PALETTE_GAUGE, 0xffff);
        }
    }
    ctx.free(&pools::EFFCT);
}

fn bubble_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let state = o.u8(ctx.b, Obj::STATE);
    let target = target(ctx, bubble::STATES, state as u32);
    match state {
        state::INIT => bubble_init(ctx, o),
        state::ACTIVE => bubble_active(ctx, o),
        state::FREED => bubble_end(ctx, o),
        6 => ctx.free(&pools::EFFCT),
        _ => outside_table(target),
    }
}

handler!(EFFCT_4, bubble::ENTRY, "effct_bubble", bubble_handler);
handler!(EFFCT_24, accessory::ENTRY, "effct_accessory", accessory_handler);
handler!(EFFCT_45, trigger::ENTRY, "effct_trigger", trigger_handler);
handler!(EFFCT_51, projectile::ENTRY, "effct_projectile", projectile_handler);
handler!(EFFCT_52, shimmer_class::ENTRY, "effct_shimmer_class", shimmer_class_handler);
handler!(EFFCT_56, shards_debris::ENTRY, "effct_shards_debris", shards_debris_handler);
// ---------------------------------------------------------------------------------------------
// Type 8: trace on an object (0xababa)

/// Type 8: trace stuck to an object (`+0x5c`), a sub-pixel in front or behind according to the variant; its animation
/// depends on the object's type (and its class: BOX); it disappears at the end of the animation. Not exercised.
mod trace {
    pub const ENTRY: u32 = 0xababa;
    pub const TABLE: u32 = 0xabac8;
    pub const ANIMATIONS_TYPE: u32 = 0xabb26;
    pub const ANIMATIONS: u32 = 0x18e872;
    pub const CARRIER: u32 = 0x5c;
}

fn trace_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
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
    let animation = ctx.prog8(trace::ANIMATIONS_TYPE + d0 as u32).wrapping_add(variant);
    ctx.anim_start(trace::ANIMATIONS, animation as u16);
    ctx.reinsert();
}

fn trace_active<R: Bench>(ctx: &mut Cx<'_, R>, _o: Obj) {
    let end = ctx.anim_step();
    if end {
        ctx.free(&pools::EFFCT);
    }
}

fn trace_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    three_states(ctx, trace::TABLE, trace_init, trace_active)
}

handler!(EFFCT_8, trace::ENTRY, "effct_trace", trace_handler);
handler!(EFFCT_28, spark::ENTRY, "effct_spark", spark_handler);
handler!(EFFCT_57, mark::ENTRY, "effct_mark", mark_handler);
handler!(EFFCT_62, sheaf::ENTRY, "effct_sheaf", sheaf_handler);
handler!(EFFCT_7, shard_single::ENTRY, "effct_shard_single", shard_single_handler);
handler!(EFFCT_19, sweep::ENTRY, "effct_sweep", sweep_handler);
handler!(EFFCT_27, smoke::ENTRY, "effct_smoke", smoke_handler);
handler!(EFFCT_1, 0xa8758, "effct_removed", removed);
handler!(EFFCT_6, debris::ENTRY, "effct_debris", debris_handler);
handler!(EFFCT_29, impact::ENTRY, "effct_impact", impact_handler);
handler!(EFFCT_17, interface::ENTRY, "effct_interface", interface_handler);
handler!(EFFCT_5, halo::ENTRY, "effct_halo", halo);
handler!(EFFCT_9, trail::ENTRY, "effct_trail", trail);
handler!(EFFCT_15, shimmering::ENTRY, "effct_shimmer", shimmering);
handler!(EFFCT_21, banner::ENTRY, "effct_banner", banner);
handler!(EFFCT_25, dust::ENTRY, "effct_dust", dust);
handler!(EFFCT_26, cloud::ENTRY, "effct_cloud", cloud);
handler!(EFFCT_32, aura::ENTRY, "effct_aura", aura);
handler!(EFFCT_33, shard::ENTRY, "effct_shard", shard);
