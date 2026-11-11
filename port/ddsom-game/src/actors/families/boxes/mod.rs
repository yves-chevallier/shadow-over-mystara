//! BOX family (class 0xA): destructible crates, barrels and chests. Dispatch `0x39e50`, type table
//! `0x39e76` (4 types). 21 slots of 0x80 bytes. See `docs/familles.md`.

use crate::actors::families::shell::records::{SetView, view};
use crate::world::objects::pools;
use crate::world::objects::Obj;
use super::shell::tables::{tables, Read};

use super::{add_u8, handler, pointer};
use super::cx::{Bench, Cx};
use crate::world::objects::Current;

/// Values of the state byte `+4`.
mod state {
    pub const INIT: u8 = 0;
    pub const ACTIVE: u8 = 2;
    pub const FREED: u8 = 4;
    pub const OUTSIDE_SCREEN: u8 = 6;
}

/// Common BOX fields.
mod field {
    pub const SUB_STATE: u32 = 0x05;
    pub const MODE: u32 = 0x2f;
    pub const GAME_SHADOWS: u32 = 0x2e;
    /// Hit received, set by the players' code.
    pub const HIT_B: u32 = 0x6e;
}

mod rom {
    pub const ANIMATIONS: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1bebc2);
}

const SHADOWS: u8 = 0x35;

// ---------------------------------------------------------------------------------------------
// Type 0: crate, barrel or chest (0xd99fa)

/// Type 0: destructible crate, barrel or chest (variants 0 to 3). A ROM sheet gives its
/// characteristics; when struck, it plays its breaking animation, drops up to three ITEM objects then
/// blinks and disappears. A chest (kind >= 3) shakes when struck; it opens (substate 2)
/// when a player opens it, consuming a key if it is locked. A player can also lift the
/// crate (substate 4: it follows their hands), throw it (6: flight, bounce off the screen edge,
/// breaks on landing) or drop it (8: fall).
pub mod chest;

// ---------------------------------------------------------------------------------------------
// Type 2: inert scenery (0xda7ac)

/// Type 2: scenery element with no behavior (animation 5 of the BOX set); the object initializes itself,
/// then just sorts itself every frame until its release. Not exercised by the scenarios.
mod scenery {
    pub const ANIMATION: u16 = 5;
}

fn box_2<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    match state {
        state::INIT => {
            add_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, Obj::ACTIVE, 2);
            o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
            ctx.anim_start(rom::ANIMATIONS.0, scenery::ANIMATION);
            ctx.reinsert();
        }
        state::ACTIVE => ctx.reinsert(),
        state::FREED | state::OUTSIDE_SCREEN => ctx.free(&pools::BOX),
        _ => crate::actors::families::effects::bad_index(state as u32),
    }
}

// ---------------------------------------------------------------------------------------------
// Type 1: barrier to open (0xda578)

/// Type 1: interactive scenery element struck by the players (`+0x6e`, set by their hit code):
/// variant 0, grid that creates a SET object (`0x8b`, from which a door comes out) then waits for it to end; variant
/// 1, two-phase mechanism whose animation changes the state of the striking object (`+0x6c`);
/// variant 3 like 0 at initialization. The object disappears when the camera has passed it by 0x200.
/// Not exercised.
mod barrier {
    pub const ANIMATIONS_0: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1bed06);
    pub const ANIMATIONS_1: crate::world::data::RomAddr = crate::world::data::RomAddr(0x1712d4);
    pub const SOUND: crate::hw::sound::Sound = crate::hw::sound::CMD_37C_47C;
    pub const HITTER: u32 = 0x6c;
    pub const SET_CREATED: u32 = 0x7c;
}

/// `0xda6f4`: creates the barrier's SET object and attaches it.
fn barrier_created_set<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let Some(new) = super::creators::scene_set(ctx.b, 0x8b) else { return };
    let hitter = pointer(ctx, o, barrier::HITTER);
    let mode = (o.u8(ctx.b, field::MODE) as u32) << 3;
    let class = ((hitter.u8(ctx.b, Obj::VARIANT) & 0xf) >> 1) as u32;
    let position = tables().barrier_positions.at8(ctx.b, (mode + class) & 0xffff);
    new.set_u8(ctx.b, SetView::PARAM_A, position);
    let character = hitter.u8(ctx.b, Obj::KIND);
    new.set_u8(ctx.b, SetView::PARAM_B, character);
    o.set_u16(ctx.b, barrier::SET_CREATED, new.0 as u16);
}

fn barrier_init<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let variant = o.u8(ctx.b, Obj::VARIANT);
    match variant {
        0 | 3 => {
            add_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
            o.set_u32(ctx.b, Barrier::ORIGIN, 0);
            ctx.anim_start(barrier::ANIMATIONS_0.0, 0);
        }
        1 => {
            add_u8(ctx, o, Obj::STATE, 2);
            o.set_u8(ctx.b, field::GAME_SHADOWS, SHADOWS);
            super::eshl_creators::single_3c(&mut crate::actors::families::effects_creators::leftovers::Pure, ctx.b, o);
            ctx.anim_start(barrier::ANIMATIONS_1.0, 4);
        }
        _ => crate::actors::families::effects::bad_index(2 * variant as u32),
    }
}

/// Variant 0, active: substate 0 waits for the hit, substate 2 waits for the end of the SET; then disappears
/// when it leaves the screen.
fn barrier_active_0<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    match sub_state {
        0 => {
            let hit = o.u8(ctx.b, field::HIT_B);
            if !(hit == 0) {
                add_u8(ctx, o, field::SUB_STATE, 2);
                barrier_created_set(ctx, o);
            }
            ctx.reinsert();
        }
        2 => {
            let set = pointer(ctx, o, barrier::SET_CREATED);
            let finished = set.u8(ctx.b, SetView::FINISHED) != 0;
            if finished {
                o.set_u8(ctx.b, field::SUB_STATE, 0);
                o.set_u8(ctx.b, field::HIT_B, 0);
            }
        }
        _ => crate::actors::families::effects::bad_index(sub_state as u32),
    }
    let (cam_x, _) = ctx.camera();
    let gap = cam_x.wrapping_sub(o.u16(ctx.b, Obj::X));
    if !((gap as i16) < 0x200) {
        add_u8(ctx, o, Obj::STATE, 2);
    }
}

fn barrier_active_1<R: Bench>(ctx: &mut Cx<'_, R>, o: Obj) {
    let sub_state = o.u8(ctx.b, field::SUB_STATE);
    match sub_state {
        0 | 4 => {
            let hit = o.u8(ctx.b, field::HIT_B);
            if hit == 0 {
                return ctx.reinsert();
            }
            add_u8(ctx, o, field::SUB_STATE, 2);
            let hitter = pointer(ctx, o, barrier::HITTER);
            hitter.set_u32(ctx.b, Obj::STATE, 0x0200_3000);
            let animation = if sub_state == 0 { 5 } else { 6 };
            ctx.anim_start(barrier::ANIMATIONS_1.0, animation);
        }
        2 | 6 => {
            let end = ctx.anim_step();
            if !end {
                return;
            }
            if sub_state == 2 {
                add_u8(ctx, o, field::SUB_STATE, 2);
            } else {
                add_u8(ctx, o, field::SUB_STATE, 0xfa);
            }
            o.set_u8(ctx.b, field::HIT_B, 0);
            crate::hw::sound::play(ctx.b, barrier::SOUND, ctx.obj());
        }
        _ => crate::actors::families::effects::bad_index(sub_state as u32),
    }
}

fn barrier_handler<R: Bench>(ctx: &mut Cx<'_, R>) {
    let o = ctx.obj();
    let cache = o.u8(ctx.b, Obj::HIDDEN) != 0;
    if cache {
        return;
    }
    let state = o.u8(ctx.b, Obj::STATE);
    match state {
        state::INIT => barrier_init(ctx, o),
        state::ACTIVE => {
            let variant = o.u8(ctx.b, Obj::VARIANT);
            match variant {
                0 => barrier_active_0(ctx, o),
                1 => barrier_active_1(ctx, o),
                _ => crate::actors::families::effects::bad_index(2 * variant as u32),
            }
        }
        state::FREED | state::OUTSIDE_SCREEN => ctx.free(&pools::BOX),
        _ => crate::actors::families::effects::bad_index(state as u32),
    }
}

handler!(BOX_1, 0xda578, "box_barrier", barrier_handler);
handler!(BOX_2, 0xda7ac, "box_scenery", box_2);

// ---------------------------------------------------------------------------------------------
// Type 3: pushed block (0xda7f2)

/// Type 3: block that a player pushes (action 0x2e, registered by the player in `+0x6c`). Variant 0: slides
/// in X between two bounds, accompanied by a SET (type 7, variant 0xc); out of the bounds, it stops (substate
/// 2) and notifies its SET. Variant 1: slides in height (`+0x10`) and realigns itself with the terrain at each step.
/// The pushing player is placed back against the block (offset per character and direction).
pub mod pushes;

view! {
    /// Fields of the barrier.
    Barrier {
        /// Original position (h).
        origin, set_origin, ORIGIN: u32 @ 0x78;
    }
}
