//! End-of-scene cinematics of the players (P3, state `+4` = 0, `+5` = 0xc; table 0x23f4e).
//!
//! At the end of a scene, each player plays a small script specific to the scene: victory pose, walk
//! to a spot (one per player, tables indexed by `+0x7d`), turns, wait for a step of the scene
//! script (`SCENE_STEP`), exit from the screen. Each routine is an automaton whose step is `+6`
//! (offset table, `steps`); after the step, the player is reinserted into the actor list except at the
//! last step.
//!
//! The routines are reached through their original address (`HANDLERS`, consulted by `scripted`).

use super::actions::start_stand;
use super::walk::{clamp_to_screen, move_z};
use super::{animation, ram, Ctx, Player};
use crate::world::mem::{rd16, wr32, A5};
use crate::world::objects::lists::ACTORS;
use crate::world::objects::Obj;
use crate::actors::player::external as ext;
use crate::actors::player::tables::{pc16};

mod endings;
use endings::*;
mod entries;
pub use entries::*;
mod entries_late;
use entries_late::*;

type Handler = fn(&mut Ctx, Player);

/// Step of the scene script (byte), awaited by the players.
const SCENE_STEP: u32 = A5 - 0x55c8;
/// Object of the scene script that the players look at (16-bit pointer).
const SCENE_ACTOR: u32 = A5 - 0x55ba;
/// Value of `ram::FROZEN` when the end of the scene is over.
const END_DONE: u8 = 2;

/// Script fields: target spot (x, depth), timer.
const TARGET_X: u32 = 0x1e0;
const TARGET_Y: u32 = 0x1e2;
const TIMER: u32 = 0x1e4;
/// Rank of the player in the scene (index of the spot tables).
const RANK: u32 = 0x7d;

/// Rewritten routines: original address -> Rust routine.
pub const HANDLERS: &[(u32, Handler)] = &[
    (0x2411a, |g, p| steps(g, p, 0x24138, Some(24), END_2411A)),
    (0x2434e, |g, p| steps(g, p, 0x2436c, Some(4), END_2434E)),
    (0x243d6, |g, p| steps(g, p, 0x243f4, Some(8), END_243D6)),
    (0x2447e, |g, p| steps(g, p, 0x2449c, Some(18), END_2447E)),
    (0x245ec, |g, p| steps(g, p, 0x2460a, Some(12), END_245EC)),
    (0x247b2, |g, p| steps(g, p, 0x247d0, Some(4), END_247B2)),
    (0x2495c, |g, p| steps(g, p, 0x2497a, Some(6), END_2495C)),
    (0x249da, wait_end),
    (0x249ea, |g, p| steps(g, p, 0x249fe, None, END_249EA)),
    (0x24a56, |g, p| steps(g, p, 0x24a74, Some(8), END_24A56)),
    (0x24aba, |g, p| steps(g, p, 0x24ad8, Some(4), END_24ABA)),
    (0x24b18, |g, p| steps(g, p, 0x24b36, Some(6), END_24B18)),
    (0x24b88, end_24b88),
    (0x24be2, |g, p| steps(g, p, 0x24c00, Some(20), END_24BE2)),
    (0x24ddc, sink),
    (0x24de6, |g, p| steps(g, p, 0x24e04, Some(6), END_24DE6)),
];

/// Steps of a routine: original address of the step -> Rust routine.
type Steps = &'static [(u32, Handler)];

/// Dispatches according to `+6` through the offset table `table`; `steps` gives the rewritten steps. Reinserts the player into the actor list except at step `last`.
fn steps(g: &mut Ctx, p: Player, table: u32, last: Option<u8>, steps: Steps) {
    dispatch(g, p, table, steps);
    if last != Some(p.u8(g, Player::ACTION)) {
        ACTORS.reinsert(g.b, p.obj());
    }
}

/// Runs step `+6` of the routine with table `table`.
fn dispatch(g: &mut Ctx, p: Player, table: u32, steps: Steps) {
    let step = p.u8(g, Player::ACTION);
    let offset = pc16(g.b, table + step as u32);
    let at = table.wrapping_add(offset as i16 as i32 as u32);
    // like the original dispatch: D0 = step, D1 = offset (some steps pass them as they are to the
    // routines they call, so 0xb378a reads its setting in D1)
    g.jumped = super::Jumped { index: step, offset };
    match steps.iter().find(|s| s.0 == at) {
        Some(&(_, f)) => f(g, p),
        None if at == IDLE.0 => {}
        None if at == STRAY_HEX_DUMP.0 => stray_hex_dump(g, offset),
        // no native at the addresses of these step tables: the game without 68000 already stopped there
        // (`docs/code-mort.md`, "Sortie du Cpu, partie 6")
        None => unreachable!("cinematic step {at:06x} not rewritten"),
    }
}

/// Common final step (a simple RTS).
const IDLE: crate::world::data::RomAddr = crate::world::data::RomAddr(0x2434c);

/// Step outside the table: routine 0x24aba has only three steps, but a player arriving in the cinematic with action
/// 0x10 (scenario `covb_region_2`) makes the jump read the word 0x6702 of its code, which leads into the middle of the
/// hexadecimal display routine of the TEST menu (0x2b1d0), after loading its two parameters.
const STRAY_HEX_DUMP: crate::world::data::RomAddr = crate::world::data::RomAddr(0x2b1da);

/// Routine of the end cinematic whose table the stray jump leaves: the end dispatch (0x23f40) jumps to it with its
/// address in A1, which the hexadecimal display then takes as its destination.
const STRAY_ROUTINE: crate::world::data::RomAddr = crate::world::data::RomAddr(0x24aba);
/// High word of D1 at the stray jump: the marker of the players' charge gauge (`game::G_PLAYER`, D1 of 0xb3e),
/// which the player loop and the dispatches (`move.w`) leave in place.
const STRAY_D1_HIGH: u32 = 0x50_0000;

/// 0x2b1da: the hexadecimal display of 0x2b1d0 (eight consecutive longs starting at D1, five digits each, one
/// tile column per long), as reached by the out-of-table jump. Original glitch, written in the clear: the destination
/// A1 is the address of the routine 0x24aba, and the value D1 is the offset read in the table under the high word left
/// by the gauge.
fn stray_hex_dump(g: &mut Ctx, offset: u16) {
    let (mut a1, mut d1) = (STRAY_ROUTINE.0, STRAY_D1_HIGH | offset as u32);
    for _ in 0..8 {
        for _ in 0..5 {
            let digit = d1 & 0xf;
            d1 = d1.rotate_right(4);
            crate::world::mem::wr16(g.b, a1, digit as u16);
            crate::world::mem::wr16(g.b, a1.wrapping_add(2), 0x10);
            a1 = a1.wrapping_sub(0x80);
        }
        d1 = d1.rotate_right(12).wrapping_add(0x10) & 0x3ffff;
        a1 = a1.wrapping_add(0x288);
    }
}

// ---- tools ------------------------------------------------------------------------------------

fn next(g: &mut Ctx, p: Player) {
    let step = p.u8(g, Player::ACTION).wrapping_add(2);
    p.set_u8(g, Player::ACTION, step);
}

fn anim_step(g: &mut Ctx, p: Player) -> bool {
    animation::step(g, p)
}

fn scene_step(g: &mut Ctx) -> u8 {
    crate::world::mem::rd8(g.b, SCENE_STEP)
}

fn end_done(g: &mut Ctx) -> bool {
    ram::FROZEN.get(g.b) == END_DONE
}

fn rank(g: &mut Ctx, p: Player) -> u32 {
    p.u8(g, RANK) as u32
}

/// Victory pose (animation 35, or 36 if `+0x18a`) and sound.
fn victory_pose(g: &mut Ctx, p: Player) {
    let anim = if p.u8(g, Player::HOLY) != 0 { 36 } else { 35 };
    animation::start(g, p, anim);
    g.sound(ext::sound::FANFARE, p);
}

/// Next step and victory pose.
fn start_victory(g: &mut Ctx, p: Player) {
    next(g, p);
    victory_pose(g, p);
}

/// Animation of set C (0x2cf0).
fn anim_c(g: &mut Ctx, p: Player, n: u32) {
    crate::world::objects::anim::start_by_character(g.b, p.obj(), &crate::world::objects::anim::SETS_BY_VARIANT, n as u16);
}

/// Stores the player's spot (table of (x, depth) pairs per rank) then turns toward it:
/// turn (next step, animation 5) or, already turned, walk (two steps further, animation 2).
fn face_place(g: &mut Ctx, p: Player, places: u32) {
    let at = places + 4 * rank(g, p);
    let (x, y) = (pc16(g.b, at), pc16(g.b, at + 2));
    p.set_u16(g, TARGET_X, x);
    p.set_u16(g, TARGET_Y, y);
    turn_toward(g, p, x);
}

/// Turns toward abscissa `x` (see `face_place`).
fn turn_toward(g: &mut Ctx, p: Player, x: u16) {
    let wanted = if p.u16(g, Obj::X) > x { 0 } else { 0x20 };
    next(g, p);
    if wanted == p.u8(g, Player::FACING) {
        next(g, p);
        animation::start(g, p, 2);
    } else {
        p.flip(g);
        animation::start(g, p, 5);
    }
}

/// End of a turn: the walk begins (animation 2).
fn turned(g: &mut Ctx, p: Player) {
    if anim_step(g, p) {
        next(g, p);
        animation::start(g, p, 2);
    }
}

/// A one-pixel step in depth toward the spot; true on arrival (nothing else is done).
fn walk_y(g: &mut Ctx, p: Player) -> bool {
    let (y, target) = (p.u16(g, Obj::Y), p.u16(g, TARGET_Y));
    if y == target {
        return true;
    }
    let dy: u16 = if (y.wrapping_sub(target) as i16) < 0 { 1 } else { 0xffff };
    p.set_u16(g, Obj::Y, y.wrapping_add(dy));
    anim_step(g, p);
    false
}

/// Walk in depth; on arrival, next step.
fn walk_to_y(g: &mut Ctx, p: Player) {
    if walk_y(g, p) {
        next(g, p);
    }
}

/// Same, the animation also advancing on arrival.
fn walk_to_y_animated(g: &mut Ctx, p: Player) {
    if walk_y(g, p) {
        next(g, p);
        anim_step(g, p);
    }
}

/// Two pixels in x toward `target`; true on arrival (within 3 pixels), without moving.
fn walk_x(g: &mut Ctx, p: Player, target: u16) -> bool {
    let gap = p.u16(g, Obj::X).wrapping_sub(target);
    if gap.wrapping_add(3) <= 6 {
        return true;
    }
    let dx: u16 = if (gap as i16) < 0 { 2 } else { 0xfffe };
    let x = p.u16(g, Obj::X).wrapping_add(dx);
    p.set_u16(g, Obj::X, x);
    anim_step(g, p);
    false
}

/// Arrived at the spot in x: turns toward the direction `wanted` (turn, animation 5) or, already turned,
/// still pose two steps further.
fn settle(g: &mut Ctx, p: Player, wanted: u8) {
    next(g, p);
    if wanted == p.u8(g, Player::FACING) {
        next(g, p);
        start_stand(g, p);
    } else {
        p.flip(g);
        animation::start(g, p, 5);
    }
}

/// End of the animation: next step and still pose.
fn stand_after_anim(g: &mut Ctx, p: Player) {
    if anim_step(g, p) {
        next(g, p);
        start_stand(g, p);
    }
}

/// End of the animation: next step and walk (animation 2).
fn walk_after_anim(g: &mut Ctx, p: Player) {
    turned(g, p)
}

/// Waits for step `value` of the scene script, then still pose.
fn stand_at_scene_step(g: &mut Ctx, p: Player, value: u8) {
    if scene_step(g) == value {
        next(g, p);
        start_stand(g, p);
    } else {
        anim_step(g, p);
    }
}

/// Waits for the end of the end of scene (`ram::FROZEN` = 2).
fn wait_done(g: &mut Ctx, p: Player) {
    if end_done(g) {
        next(g, p);
    }
    anim_step(g, p);
}

/// If the speed class is 2, it becomes 0xff.
fn reset_speed_class(g: &mut Ctx, p: Player) {
    if p.u8(g, Player::SPEED_CLASS) == 2 {
        p.set_u8(g, Player::SPEED_CLASS, 0xff);
    }
}

fn elf_effect(g: &mut Ctx, p: Player) {
    if p.class(g) == 2 {
        super::init::raise_spells(g, p);
    }
}

/// Walks to the right (4 pixels per frame) as long as the player is less than 480 pixels from the left
/// edge of the screen.
fn walk_out(g: &mut Ctx, p: Player) {
    let limit = ram::CAMERA_X.get(g.b).wrapping_add(480);
    if limit > p.u16(g, Obj::X) {
        let x = p.u16(g, Obj::X).wrapping_add(4);
        p.set_u16(g, Obj::X, x);
    }
}

// ---- routines without a table ---------------------------------------------------------------------------

/// 0x249da: nothing but the reinsertion, until the end.
fn wait_end(g: &mut Ctx, p: Player) {
    if !end_done(g) {
        ACTORS.reinsert(g.b, p.obj());
    }
}

/// 0x24ddc: the player sinks by 2 pixels per frame.
fn sink(g: &mut Ctx, p: Player) {
    let z = p.u16(g, Obj::Z).wrapping_sub(2);
    p.set_u16(g, Obj::Z, z);
    anim_step(g, p);
}

/// 0x24b88: without `A5+0x66`, victory pose then wait; otherwise still pose (0x24102).
fn end_24b88(g: &mut Ctx, p: Player) {
    if ram::NO_WALK_IN.get(g.b) != 0 {
        super::scripted::idle(g, p);
        return;
    }
    steps(g, p, 0x24bb0, Some(4), &[(0x24bb6, start_victory), (0x24bd0, wait_done)]);
}

