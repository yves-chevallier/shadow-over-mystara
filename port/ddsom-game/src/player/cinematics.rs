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
use crate::mem::{rd16, rd32, wr32, A5};
use crate::objects::lists::ACTORS;
use crate::objects::Obj;
use crate::machine::Machine;
use crate::player::external as ext;

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
    let offset = g.prog16(table + step as u32);
    let at = table.wrapping_add(offset as i16 as i32 as u32);
    // like the original dispatch: D0 = step, D1 = offset (some steps pass them as they are to the
    // routines they call, so 0xb378a reads its setting in D1)
    g.jumped = super::Jumped { index: step, offset };
    match steps.iter().find(|s| s.0 == at) {
        Some(&(_, f)) => f(g, p),
        None if at == IDLE => {}
        None if at == STRAY_HEX_DUMP => stray_hex_dump(g, offset),
        // no native at the addresses of these step tables: the game without 68000 already stopped there
        // (`docs/code-mort.md`, "Sortie du Cpu, partie 6")
        None => unreachable!("cinematic step {at:06x} not rewritten"),
    }
}

/// Common final step (a simple RTS).
const IDLE: u32 = 0x2434c;

/// Step outside the table: routine 0x24aba has only three steps, but a player arriving in the cinematic with action
/// 0x10 (scenario `covb_region_2`) makes the jump read the word 0x6702 of its code, which leads into the middle of the
/// hexadecimal display routine of the TEST menu (0x2b1d0), after loading its two parameters.
const STRAY_HEX_DUMP: u32 = 0x2b1da;

/// Routine of the end cinematic whose table the stray jump leaves: the end dispatch (0x23f40) jumps to it with its
/// address in A1, which the hexadecimal display then takes as its destination.
const STRAY_ROUTINE: u32 = 0x24aba;
/// High word of D1 at the stray jump: the marker of the players' charge gauge (`game::G_PLAYER`, D1 of 0xb3e),
/// which the player loop and the dispatches (`move.w`) leave in place.
const STRAY_D1_HIGH: u32 = 0x50_0000;

/// 0x2b1da: the hexadecimal display of 0x2b1d0 (eight consecutive longs starting at D1, five digits each, one
/// tile column per long), as reached by the out-of-table jump. Original glitch, written in the clear: the destination
/// A1 is the address of the routine 0x24aba, and the value D1 is the offset read in the table under the high word left
/// by the gauge.
fn stray_hex_dump(g: &mut Ctx, offset: u16) {
    let (mut a1, mut d1) = (STRAY_ROUTINE, STRAY_D1_HIGH | offset as u32);
    for _ in 0..8 {
        for _ in 0..5 {
            let digit = d1 & 0xf;
            d1 = d1.rotate_right(4);
            crate::mem::wr16(g.b, a1, digit as u16);
            crate::mem::wr16(g.b, a1.wrapping_add(2), 0x10);
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
    crate::mem::rd8(g.b, SCENE_STEP)
}

fn end_done(g: &mut Ctx) -> bool {
    ram::FROZEN.get(g.b) == END_DONE
}

fn rank(g: &mut Ctx, p: Player) -> u32 {
    p.u8(g, RANK) as u32
}

/// Victory pose (animation 35, or 36 if `+0x18a`) and sound.
fn victory_pose(g: &mut Ctx, p: Player) {
    let anim = if p.u8(g, 0x18a) != 0 { 36 } else { 35 };
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
    crate::objects::anim::start_by_character(g.b, p.obj(), &crate::objects::anim::SETS_BY_VARIANT, n as u16);
}

/// Stores the player's spot (table of (x, depth) pairs per rank) then turns toward it:
/// turn (next step, animation 5) or, already turned, walk (two steps further, animation 2).
fn face_place(g: &mut Ctx, p: Player, places: u32) {
    let at = places + 4 * rank(g, p);
    let (x, y) = (g.prog16(at), g.prog16(at + 2));
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

// ---- 0x2411a ----------------------------------------------------------------------------------

const END_2411A: Steps = &[
    (0x24152, |g, p| {
        let actor = rd16(g.b, SCENE_ACTOR) as i16 as i32 as u32;
        if rd32(g.b, actor + 4) == 0x0402_0800 {
            next(g, p);
        }
        anim_step(g, p);
    }),
    (0x2416a, |g, p| face_place(g, p, 0x241b8)),
    (0x241c8, walk_after_anim),
    (0x241de, walk_to_y),
    (0x24200, |g, p| {
        let target = p.u16(g, TARGET_X);
        if walk_x(g, p, target) {
            settle(g, p, 0);
        }
    }),
    (0x2423e, stand_after_anim),
    (0x24252, |g, p| {
        let actor = rd16(g.b, SCENE_ACTOR) as i16 as i32 as u32;
        if p.u16(g, Obj::X).wrapping_sub(rd16(g.b, actor + 8)) <= 120 {
            next(g, p);
            p.set_u16(g, Obj::VZ, 0x500);
            p.set_u16(g, Obj::AZ, (-56i16) as u16);
            anim_c(g, p, 0);
        } else {
            anim_step(g, p);
        }
    }),
    (0x24282, |g, p| {
        move_z(g, p);
        if (p.u16(g, Obj::Z) as i16) < 0 {
            next(g, p);
            p.set_u32(g, Obj::Z, 0);
            start_stand(g, p);
        } else {
            anim_step(g, p);
        }
    }),
    (0x242a2, |g, p| {
        let actor = rd16(g.b, SCENE_ACTOR) as i16 as i32 as u32;
        if crate::mem::rd8(g.b, actor + 4) != 6 {
            return;
        }
        next(g, p);
        p.set_u16(g, TIMER, 136);
        p.set_u8(g, Player::FACING, 0x20);
        let at = 0x242dc + 4 * rank(g, p);
        let (x, y) = (g.prog16(at), g.prog16(at + 2));
        p.set_u16(g, Obj::X, x);
        p.set_u16(g, Obj::Y, y);
        animation::start(g, p, 2);
    }),
    (0x242ec, |g, p| {
        let left = p.u16(g, TIMER).wrapping_sub(1);
        p.set_u16(g, TIMER, left);
        if left == 0 {
            next(g, p);
            p.set_u16(g, TIMER, 90);
            start_stand(g, p);
            return;
        }
        if left <= 80 {
            let x = p.u16(g, Obj::X).wrapping_add(2);
            p.set_u16(g, Obj::X, x);
        }
        anim_step(g, p);
    }),
    (0x24314, |g, p| {
        let left = p.u16(g, TIMER).wrapping_sub(1);
        p.set_u16(g, TIMER, left);
        if left == 0 {
            start_victory(g, p);
        } else {
            anim_step(g, p);
        }
    }),
    (0x2433a, wait_done),
];

// ---- 0x2434e (ten scenes) -----------------------------------------------------------------------

const END_2434E: Steps = &[
    (0x24372, |g, p| {
        start_victory(g, p);
        let scene = ram::SCENE.get(g.b);
        if scene == 46 {
            p.set_u8(g, 0x193, 1);
        }
        if matches!(scene, 46 | 58 | 66) {
            elf_effect(g, p);
        }
    }),
    (0x243c4, wait_done),
];

// ---- 0x243d6 ----------------------------------------------------------------------------------

const END_243D6: Steps = &[
    (0x243fe, |g, p| {
        next(g, p);
        p.set_u16(g, TIMER, 144);
        victory_pose(g, p);
    }),
    (0x2441e, |g, p| {
        let left = p.u16(g, TIMER).wrapping_sub(1);
        p.set_u16(g, TIMER, left);
        if left == 0 {
            next(g, p);
            p.set_u8(g, Player::FACING, 0x20);
            start_stand(g, p);
        } else {
            anim_step(g, p);
        }
    }),
    (0x2443a, |g, p| {
        if scene_step(g) == 20 {
            next(g, p);
            animation::start(g, p, 7);
        } else {
            anim_step(g, p);
        }
    }),
    (0x24454, |g, p| {
        walk_out(g, p);
        wait_done(g, p);
        crate::terrain::push(g.b, p.obj());
    }),
];

// ---- 0x2447e ----------------------------------------------------------------------------------

/// Final direction per rank (code bytes).
fn final_facing(g: &mut Ctx, p: Player, table: u32) -> u8 {
    let k = rank(g, p) & 3;
    g.prog8(table + k)
}

const END_2447E: Steps = &[
    (0x244b0, |g, p| face_place(g, p, 0x244fe)),
    (0x2450e, walk_after_anim),
    (0x24524, walk_to_y),
    (0x24546, |g, p| {
        let target = p.u16(g, TARGET_X);
        if walk_x(g, p, target) {
            let wanted = final_facing(g, p, 0x24590);
            settle(g, p, wanted);
        }
    }),
    (0x24594, stand_after_anim),
    (0x245a8, |g, p| {
        if ram::CUTSCENE.get(g.b) != 0 {
            next(g, p);
        }
        anim_step(g, p);
    }),
    (0x245b8, |g, p| {
        super::hurt::fall(g, p);
        clamp_to_screen(g.b, p);
    }),
    (0x245c4, |g, p| {
        if scene_step(g) == 16 {
            next(g, p);
            anim_c(g, p, 3);
        }
    }),
    (0x245da, wait_done),
];

// ---- 0x245ec ----------------------------------------------------------------------------------

/// Destination of the exit (x) and depth of the disappearance.
const EXIT_X: u16 = 220;
const EXIT_Y: u16 = 384;

const END_245EC: Steps = &[
    (0x2461c, start_victory),
    (0x24636, |g, p| {
        if scene_step(g) != 16 {
            anim_step(g, p);
            return;
        }
        next(g, p);
        let k = rank(g, p);
        let delay = g.prog8(0x24660 + k);
        p.set_u8(g, TIMER, delay);
        start_stand(g, p);
        crate::families::effects_creators::for_players::halo(&mut g.leftovers, g.b, p.obj());
    }),
    (0x24664, |g, p| {
        let left = p.u8(g, TIMER).wrapping_sub(1);
        p.set_u8(g, TIMER, left);
        if left == 0 {
            next(g, p);
        }
        anim_step(g, p);
    }),
    (0x24674, |g, p| {
        if scene_step(g) == 20 {
            turn_toward(g, p, 208);
        } else {
            anim_step(g, p);
        }
    }),
    (0x246b8, walk_after_anim),
    (0x246ce, |g, p| {
        if walk_x(g, p, EXIT_X) {
            next(g, p);
            animation::start(g, p, 37);
        }
    }),
    (0x246fe, |g, p| {
        if p.u16(g, Obj::Y) == EXIT_Y {
            next(g, p);
            p.set_u8(g, 0x1e7, 0);
            p.set_u8(g, 0x1e8, 1);
            p.set_u8(g, 0x1e6, 48);
            anim_step(g, p);
            return;
        }
        let y = p.u16(g, Obj::Y);
        let dy: u16 = if (y.wrapping_sub(EXIT_Y) as i16) < 0 { 1 } else { 0xffff };
        p.set_u16(g, Obj::Y, y.wrapping_add(dy));
        anim_step(g, p);
    }),
    (0x24734, fade_out),
];

/// Disappearance palettes (data): 3 sets of 12 characters × 32 bytes; memory destination
/// of the palettes, offset per character (word table).
use super::moves::{EXIT_PALETTES as FADE_PALETTES, EXIT_PALETTE_SLOTS as FADE_TARGETS};
const PALETTE_RAM: u32 = 0x91c000;

/// 0x24734: disappearance: after 48 frames the player is hidden; every 12 frames, their palette moves
/// to the next set (three sets).
fn fade_out(g: &mut Ctx, p: Player) {
    anim_step(g, p);
    let left = p.u8(g, 0x1e6).wrapping_sub(1);
    p.set_u8(g, 0x1e6, left);
    if left == 0 {
        next(g, p);
        let flags = p.u8(g, Player::FLAGS) | 2;
        p.set_u8(g, Player::FLAGS, flags);
    }
    p.set_u8(g, 0x3b, 0);
    let set = p.u8(g, 0x1e7);
    if set > 2 {
        return;
    }
    let wait = p.u8(g, 0x1e8).wrapping_sub(1);
    p.set_u8(g, 0x1e8, wait);
    if wait != 0 {
        return;
    }
    let character = p.u8(g, Player::CHARACTER) as u32;
    let from = FADE_PALETTES + 384 * set as u32 + 32 * character;
    let to = PALETTE_RAM.wrapping_add(rd16(g.b, FADE_TARGETS + 2 * character) as i16 as i32 as u32);
    for i in 0..8 {
        let v = rd32(g.b, from + 4 * i);
        wr32(g.b, to + 4 * i, v);
    }
    p.set_u8(g, 0x1e8, 12);
    p.set_u8(g, 0x1e7, set + 1);
}

// ---- 0x247b2 ----------------------------------------------------------------------------------

const END_247B2: Steps = &[
    (0x247e4, |g, p| {
        next(g, p);
        let anim = if p.u8(g, 0x18a) != 0 { 36 } else { 35 };
        animation::start(g, p, anim);
        reset_speed_class(g, p);
        g.sound(ext::sound::FANFARE, p);
    }),
    (0x2480c, |g, p| {
        if scene_step(g) == 12 {
            face_place(g, p, 0x24864);
        } else {
            anim_step(g, p);
        }
    }),
    (0x24874, walk_after_anim),
    (0x2488a, walk_to_y_animated),
    (0x248b0, |g, p| {
        let target = p.u16(g, TARGET_X);
        if walk_x(g, p, target) {
            let wanted = final_facing(g, p, 0x248fa);
            settle(g, p, wanted);
        }
    }),
    (0x248fe, stand_after_anim),
    (0x24912, |g, p| {
        anim_step(g, p);
    }),
    (0x24918, |g, p| {
        next(g, p);
        let phase = ((rank(g, p) & 3) << 3 & 15) as u16;
        p.set_u16(g, TIMER, phase);
        anim_c(g, p, 4);
    }),
    (0x24936, |g, p| {
        if end_done(g) {
            next(g, p);
        }
        let left = p.u16(g, TIMER).wrapping_sub(1) & 15;
        p.set_u16(g, TIMER, left);
        if left == 0 {
            // Original glitch: the setting is D1.b, the offset of the step left by the dispatch
            let setting = g.jumped.offset as u8;
            crate::families::effects_creators::for_players::blow(&mut g.leftovers, g.b, p.obj(), 4, setting);
        }
        anim_step(g, p);
    }),
];

// ---- 0x2495c, 0x249ea, 0x24a56, 0x24aba, 0x24b18 -------------------------------------------------

const END_2495C: Steps = &[
    (0x24982, start_victory),
    (0x2499c, |g, p| stand_at_scene_step(g, p, 16)),
    (0x249b4, |g, p| {
        if end_done(g) {
            next(g, p);
            elf_effect(g, p);
        }
        anim_step(g, p);
    }),
];

const END_249EA: Steps = &[
    (0x24a06, |g, p| {
        next(g, p);
        p.set_u16(g, TIMER, 16);
        p.set_u8(g, Player::FACING, 0x20);
        start_stand(g, p);
    }),
    (0x24a1c, |g, p| {
        let left = p.u16(g, TIMER).wrapping_sub(1);
        p.set_u16(g, TIMER, left);
        if left == 0 {
            next(g, p);
            animation::start(g, p, 7);
        } else {
            anim_step(g, p);
        }
    }),
    (0x24a34, |g, p| {
        let limit = ram::CAMERA_X.get(g.b).wrapping_add(480);
        if limit <= p.u16(g, Obj::X) {
            next(g, p);
        }
        let x = p.u16(g, Obj::X).wrapping_add(4);
        p.set_u16(g, Obj::X, x);
        anim_step(g, p);
        crate::terrain::push(g.b, p.obj());
    }),
];

const END_24A56: Steps = &[
    (0x24a7a, |g, p| {
        if scene_step(g) == 8 {
            next(g, p);
            p.set_u8(g, Player::FACING, 0x20);
            animation::start(g, p, 7);
        }
    }),
    (0x24a96, |g, p| {
        walk_out(g, p);
        wait_done(g, p);
    }),
];

const END_24ABA: Steps = &[
    (0x24ade, start_victory),
    (0x24af8, |g, p| {
        if end_done(g) {
            next(g, p);
            reset_speed_class(g, p);
        }
        anim_step(g, p);
    }),
];

const END_24B18: Steps = &[
    (0x24b3e, start_victory),
    (0x24b58, |g, p| {
        if ram::NO_VICTORY.get(g.b) != 0 && scene_step(g) == 16 {
            next(g, p);
            start_stand(g, p);
        } else {
            anim_step(g, p);
        }
    }),
    (0x24b76, wait_done),
];

// ---- 0x24be2 ----------------------------------------------------------------------------------

const END_24BE2: Steps = &[
    (0x24c16, start_victory),
    (0x24c30, |g, p| {
        if scene_step(g) == 16 {
            face_place(g, p, 0x24c88);
        } else {
            anim_step(g, p);
        }
    }),
    (0x24c98, walk_after_anim),
    (0x24cae, walk_to_y),
    (0x24cd0, |g, p| {
        let target = p.u16(g, TARGET_X);
        if !walk_x(g, p, target) {
            return;
        }
        next(g, p);
        if p.u8(g, Player::FACING) != 0 {
            go_to_gate(g, p);
        } else {
            p.flip(g);
            animation::start(g, p, 5);
        }
    }),
    (0x24d0e, |g, p| {
        if anim_step(g, p) {
            go_to_gate(g, p);
        }
    }),
    (0x24d2e, |g, p| {
        if scene_step(g) == 36 {
            next(g, p);
            let k = rank(g, p) & 3;
            let delay = g.prog16(0x24d50 + 2 * k);
            p.set_u16(g, TIMER, delay);
        }
        anim_step(g, p);
    }),
    (0x24d58, |g, p| {
        let wait = p.u16(g, TIMER);
        if wait != 0 {
            p.set_u16(g, TIMER, wait - 1);
            if wait - 1 == 0 {
                animation::start(g, p, 2);
            } else {
                anim_step(g, p);
            }
            return;
        }
        walk_to_y(g, p);
    }),
    (0x24d8e, |g, p| {
        let gap = p.u16(g, Obj::X).wrapping_sub(p.u16(g, TARGET_X));
        if gap.wrapping_add(3) <= 6 {
            next(g, p);
            p.set_u16(g, Obj::VZ, 0x100);
            p.set_u16(g, Obj::AZ, 48);
            start_stand(g, p);
        } else {
            let x = p.u16(g, Obj::X).wrapping_add(2);
            p.set_u16(g, Obj::X, x);
            anim_step(g, p);
        }
    }),
    (0x24dc0, |g, p| {
        move_z(g, p);
        if p.u16(g, Obj::Z) > 256 {
            next(g, p);
            p.set_u32(g, Obj::VZ, 0);
        }
        anim_step(g, p);
    }),
];

/// 0x24d16: still pose, then the exit door (x 192, depth 380) becomes the target spot.
fn go_to_gate(g: &mut Ctx, p: Player) {
    next(g, p);
    p.set_u16(g, TARGET_X, 192);
    p.set_u16(g, TARGET_Y, 380);
    start_stand(g, p);
}

// ---- 0x24de6 ----------------------------------------------------------------------------------

const END_24DE6: Steps = &[
    (0x24e0e, |g, p| {
        next(g, p);
        start_stand(g, p);
    }),
    (0x24e18, |g, p| {
        if scene_step(g) != 12 {
            anim_step(g, p);
            return;
        }
        next(g, p);
        let (x, y) = (ram::CAMERA_X.get(g.b), ram::CAMERA_Y.get(g.b));
        let at = 0x24e66 + 4 * (rank(g, p) & 3);
        let (dx, dy) = (g.prog16(at), g.prog16(at + 2));
        p.set_u16(g, Obj::X, x.wrapping_add(dx));
        p.set_u16(g, Obj::Y, y.wrapping_add(dy));
        victory_pose(g, p);
    }),
    (0x24e76, |g, p| stand_at_scene_step(g, p, 20)),
    (0x24e8e, wait_done),
];

// =================================================================================================
// Scene entries (`+5` = 0, table 0x23826 indexed by scene)
//
// The player appears (often off screen to the left), waits, walks to the right, strikes the pose
// and receives the controls when the game enters the play phase (`ram::PHASE` = 12). `ram::SUB_STEP`
// (`A5+0x2e`) is the step of the scene's entry script. The caller (`scripted::update`) reinserts the
// player into the actor list.
// =================================================================================================

/// Play phase: the players receive the controls.
const PLAYING_PHASE: u8 = 12;
/// Second timer of the entries.
const TIMER_B: u32 = 0x1e2;
/// Timer of the entries (word).
const WAIT: u32 = 0x1e0;
/// Weapon in hand (0: 0x40 offset of the animations).
const ARMED: u32 = 0x68;

/// Rewritten entries: original address -> Rust routine.
pub const INTRO_HANDLERS: &[(u32, Handler)] = &[
    (0x23ac8, |g, p| dispatch(g, p, 0x23ad6, INTRO_23AC8)),
    (0x23b32, intro_23b32),
    (0x24fe2, |g, p| dispatch(g, p, 0x24ff0, INTRO_24FE2)),
    (0x2522e, |g, p| dispatch(g, p, 0x2523c, INTRO_2522E)),
    (0x252ac, |g, p| dispatch(g, p, 0x252ba, INTRO_252AC)),
    (0x253a0, |g, p| dispatch(g, p, 0x253ae, INTRO_253A0)),
    (0x2541a, |g, p| dispatch(g, p, 0x25428, INTRO_2541A)),
    (0x25504, |g, p| dispatch(g, p, 0x25512, INTRO_25504)),
    (0x255ea, |g, p| dispatch(g, p, 0x255f8, INTRO_255EA)),
    (0x2564a, |g, p| dispatch(g, p, 0x25658, INTRO_2564A)),
    (0x256c2, |g, p| dispatch(g, p, 0x256d0, INTRO_256C2)),
    (0x257a4, |g, p| dispatch(g, p, 0x257b2, INTRO_257A4)),
    (0x25814, |g, p| dispatch(g, p, 0x25822, INTRO_25814)),
    (0x258de, |g, p| dispatch(g, p, 0x258ec, INTRO_258DE)),
    (0x25954, |g, p| dispatch(g, p, 0x25962, INTRO_25954)),
    (0x25a24, intro_25a24),
    (0x25aa2, |g, p| dispatch(g, p, 0x25ab0, INTRO_25AA2)),
    (0x25b3e, |g, p| dispatch(g, p, 0x25b4c, INTRO_25B3E)),
    (0x25ba0, |g, p| dispatch(g, p, 0x25bae, INTRO_25BA0)),
    (0x25c9e, |g, p| dispatch(g, p, 0x25cac, INTRO_25C9E)),
    (0x25d40, |g, p| dispatch(g, p, 0x25d4e, INTRO_25D40)),
    (0x25db4, |g, p| dispatch(g, p, 0x25dc2, INTRO_25DB4)),
    (0x25e28, |g, p| dispatch(g, p, 0x25e36, INTRO_25E28)),
    (0x25efc, intro_25efc),
    (0x25f7e, |g, p| dispatch(g, p, 0x25f8c, INTRO_25F7E)),
];

// ---- entry tools -------------------------------------------------------------------------

/// Appearance: the player becomes active; `visible` also sets `+1`; `armed`: weapon in hand.
fn appear(g: &mut Ctx, p: Player, visible: bool, armed: bool) {
    next(g, p);
    p.set_u8(g, Player::ACTIVE, 1);
    if visible {
        p.set_u8(g, Player::FLAGS, 1);
    }
    p.set_u8(g, ARMED, armed as u8);
}

/// Last step: in the play phase, the player receives the controls.
fn take_control(g: &mut Ctx, p: Player) {
    if ram::PHASE.get(g.b) == PLAYING_PHASE {
        p.set_state(g, 0x0200_0000);
    }
    anim_step(g, p);
}

/// Countdown of `WAIT`; true when it reaches zero.
fn countdown(g: &mut Ctx, p: Player, field: u32) -> bool {
    let left = p.u16(g, field).wrapping_sub(1);
    p.set_u16(g, field, left);
    left == 0
}

/// Wait without animation: true as long as `field` is not exhausted (it is counted down).
fn waiting(g: &mut Ctx, p: Player, field: u32) -> bool {
    let left = p.u16(g, field);
    if left != 0 {
        p.set_u16(g, field, left - 1);
    }
    left != 0
}

/// Advances by `dx` pixels.
fn step_x(g: &mut Ctx, p: Player, dx: u16) {
    let x = p.u16(g, Obj::X).wrapping_add(dx);
    p.set_u16(g, Obj::X, x);
}

/// Walks to the right (2 pixels per frame) for `field` frames, then `done`.
fn walk_right(g: &mut Ctx, p: Player, field: u32, done: fn(&mut Ctx, Player)) {
    step_x(g, p, 2);
    if countdown(g, p, field) {
        next(g, p);
        done(g, p);
    } else {
        anim_step(g, p);
    }
}

fn anim_0(g: &mut Ctx, p: Player) {
    animation::start(g, p, 0);
}

/// Word of the table `table` for the player's rank (index `2 × rank`, byte).
fn per_rank16(g: &mut Ctx, p: Player, table: u32) -> u16 {
    let index = (p.u8(g, RANK).wrapping_mul(2)) as u32;
    g.prog16(table + index)
}

/// End of the animation: still pose; in the meantime, the hero's cry at the gesture (fighter and elf).
fn pose_with_shout(g: &mut Ctx, p: Player) {
    if anim_step(g, p) {
        next(g, p);
        start_stand(g, p);
        return;
    }
    if p.u8(g, 0x33) == 0 {
        return;
    }
    p.set_u8(g, 0x33, 0);
    if matches!(p.class(g), 0 | 2) {
        super::spawn::swap(g, p, 0);
    }
}

/// End of the animation: next step and animation `n`; nothing else otherwise.
fn then_anim(g: &mut Ctx, p: Player, n: u16) {
    if anim_step(g, p) {
        next(g, p);
        animation::start(g, p, n);
    }
}

/// Countdown of `WAIT`, then the weapon in hand and pose 51.
fn draw_weapon_after_wait(g: &mut Ctx, p: Player) {
    if countdown(g, p, WAIT) {
        next(g, p);
        p.set_u8(g, ARMED, 1);
        animation::start(g, p, 51);
    } else {
        anim_step(g, p);
    }
}

/// The player stands in front of the object followed by the camera (`A5+0x46`), shifted by its number (`+2`);
/// `table`: x offsets (words).
pub(super) fn place_by_focus(g: &mut Ctx, p: Player, table: u32) -> u32 {
    let focus = ram::CAMERA_FOCUS.get(g.b) as i16 as i32 as u32;
    let (x, y) = (rd16(g.b, focus + 8), rd16(g.b, focus + 0x10));
    p.set_u16(g, Obj::X, x);
    p.set_u16(g, Obj::Y, y);
    p.set_u16(g, Obj::Z, 0);
    let index = 2 * p.u8(g, Player::INDEX) as u32;
    let dx = g.prog16(table + index);
    p.set_u16(g, Obj::X, x.wrapping_add(dx));
    focus
}

// ---- 0x23ac8, 0x23b32 ---------------------------------------------------------------------------

const INTRO_23AC8: Steps = &[
    (0x23adc, |g, p| {
        appear(g, p, true, true);
        p.set_u8(g, 0x15c, 0);
        p.set_u16(g, WAIT, 80);
        animation::start(g, p, 2);
    }),
    (0x23b02, |g, p| walk_right(g, p, WAIT, start_stand)),
    (0x23b1c, take_control),
];

/// 0x23b32: still appearance, then the controls in the play phase.
fn intro_23b32(g: &mut Ctx, p: Player) {
    if p.u8(g, Player::ACTION) == 0 {
        appear(g, p, true, false);
        p.set_u8(g, 0x15c, 0);
        p.set_u8(g, ARMED, 1);
        start_stand(g, p);
    } else if ram::PHASE.get(g.b) == PLAYING_PHASE {
        p.set_state(g, 0x0200_0000);
    }
}

// ---- 0x24fe2: walk-in, two turns, then a jump toward the bottom of the screen ---------

const INTRO_24FE2: Steps = &[
    (0x25006, |g, p| {
        next(g, p);
        p.set_u8(g, Player::ACTIVE, 1);
        p.set_u8(g, 0x15c, 0);
        p.set_u8(g, ARMED, 0);
        p.set_u16(g, WAIT, 136);
        p.set_u32(g, Obj::VX, 0x0200_0000);
        p.set_u32(g, Obj::VZ, 0);
        p.set_u32(g, Obj::VY, 0);
        animation::start(g, p, 2);
    }),
    (0x25034, |g, p| {
        if countdown(g, p, WAIT) {
            next(g, p);
            let wait = per_rank16(g, p, 0x25066);
            p.set_u16(g, WAIT, wait);
            crate::families::effects_creators::for_players::halo(&mut g.leftovers, g.b, p.obj());
            anim_0(g, p);
        } else {
            anim_step(g, p);
            super::walk::move_x(g, p);
        }
    }),
    (0x2506e, |g, p| {
        if !countdown(g, p, WAIT) {
            return;
        }
        next(g, p);
        p.set_u8(g, ARMED, 1);
        animation::start(g, p, 51);
        if p.u8(g, RANK) == 0 {
            g.sound(crate::sound::CMD_90, p);
            g.sound(crate::sound::SCENE_MUSIC, p);
        }
    }),
    (0x2509a, |g, p| {
        if anim_step(g, p) {
            next(g, p);
            let wait = per_rank16(g, p, 0x250be);
            p.set_u16(g, WAIT, wait);
            anim_0(g, p);
        }
    }),
    (0x250c6, turn_after_wait),
    (0x250e0, |g, p| {
        if anim_step(g, p) {
            next(g, p);
            p.set_u16(g, WAIT, 16);
            anim_0(g, p);
        }
    }),
    (0x250fc, turn_after_wait),
    (0x25116, |g, p| {
        if !anim_step(g, p) {
            return;
        }
        next(g, p);
        p.set_u16(g, WAIT, 320);
        let at = 0x25154 + 8 * p.u8(g, RANK) as u32;
        let (vx, vy, delay) = (g.prog16(at), g.prog16(at + 2), g.prog16(at + 4));
        p.set_u16(g, Obj::VX, vx);
        p.set_u16(g, Obj::VY, vy);
        p.set_u16(g, TIMER_B, delay);
        p.set_u16(g, Obj::AX, 0);
        p.set_u16(g, Obj::AY, 0);
        anim_c(g, p, 1);
    }),
    (0x25174, |g, p| {
        if !waiting(g, p, TIMER_B) {
            super::walk::move_xy(g, p);
        }
        anim_step(g, p);
        if rd16(g.b, A5 - 0x65c4) != 0 {
            step_x(g, p, 0xfffe);
        }
        if countdown(g, p, WAIT) {
            next(g, p);
            p.set_u16(g, WAIT, 6);
            let k = p.u8(g, RANK) as u32;
            let drift = g.prog32(0x251ce + 4 * k);
            p.set_u32(g, Obj::VY, drift);
            p.set_u16(g, Obj::VZ, 0x580);
            p.set_u16(g, Obj::AZ, (-56i16) as u16);
            p.set_u32(g, Obj::VX, 0);
            anim_c(g, p, 2);
        }
    }),
    (0x251de, |g, p| {
        super::walk::move_xz(g, p);
        step_x(g, p, 0xffff);
        if waiting(g, p, WAIT) {
            return;
        }
        let y = p.u32(g, Obj::Y).wrapping_add(p.u32(g, Obj::VY));
        p.set_u32(g, Obj::Y, y);
        if (p.u16(g, Obj::Z) as i16) < 0 {
            next(g, p);
            p.set_u32(g, Obj::Z, 0);
            anim_0(g, p);
        } else {
            anim_step(g, p);
        }
    }),
    (0x25218, take_control),
];

/// Countdown of `WAIT`, then turn (animation 5).
fn turn_after_wait(g: &mut Ctx, p: Player) {
    if countdown(g, p, WAIT) {
        next(g, p);
        p.flip(g);
        animation::start(g, p, 5);
    }
}

// ---- 0x2522e, 0x252ac: the player follows the camera object (mount, raft...) -----------------

const INTRO_2522E: Steps = &[
    (0x25240, |g, p| {
        appear(g, p, false, true);
        p.set_u8(g, 0x15c, 0);
        p.set_u16(g, WAIT, 80);
        anim_0(g, p);
        follow_focus(g, p);
    }),
    (0x25294, |g, p| {
        if ram::PHASE.get(g.b) == PLAYING_PHASE {
            p.set_state(g, 0x0200_0000);
        }
        follow_focus(g, p);
        anim_step(g, p);
    }),
];

/// 0x25260: in front of the camera object, 18 pixels closer.
fn follow_focus(g: &mut Ctx, p: Player) {
    place_by_focus(g, p, 0x2528c);
    let y = p.u16(g, Obj::Y).wrapping_sub(18);
    p.set_u16(g, Obj::Y, y);
}

const INTRO_252AC: Steps = &[
    (0x252c2, |g, p| {
        appear(g, p, false, true);
        p.set_u8(g, 0x15c, 0);
        p.set_u16(g, WAIT, 80);
        anim_0(g, p);
    }),
    (0x252e2, |g, p| {
        let focus = ram::CAMERA_FOCUS.get(g.b) as i16 as i32 as u32;
        let (x, y) = (rd16(g.b, focus + 8), rd16(g.b, focus + 0x10));
        p.set_u16(g, Obj::X, x);
        p.set_u16(g, Obj::Y, y.wrapping_sub(17));
        p.set_u16(g, Obj::Z, 0);
        let index = 2 * p.u8(g, Player::INDEX) as u32;
        let dx = g.prog16(0x25348 + index);
        p.set_u16(g, Obj::X, x.wrapping_add(dx));
        if crate::mem::rd8(g.b, focus + 5) == 0 {
            anim_step(g, p);
            return;
        }
        next(g, p);
        p.set_u16(g, Obj::VZ, 0x500);
        p.set_u16(g, Obj::AZ, (-56i16) as u16);
        p.set_u32(g, Obj::VX, 0);
        let drift = g.prog32(0x25350 + 2 * index);
        p.set_u32(g, Obj::VY, drift);
        anim_c(g, p, 0);
    }),
    (0x25360, |g, p| {
        super::walk::move_xz(g, p);
        let y = p.u32(g, Obj::Y).wrapping_add(p.u32(g, Obj::VY));
        p.set_u32(g, Obj::Y, y);
        if (p.u16(g, Obj::Z) as i16) < 0 {
            next(g, p);
            p.set_u32(g, Obj::Z, 0);
            anim_0(g, p);
        } else {
            anim_step(g, p);
        }
    }),
    (0x2538a, take_control),
];

// ---- 0x253a0 ------------------------------------------------------------------------------------

fn intro_253a0_start(g: &mut Ctx, p: Player) {
    appear(g, p, false, true);
    p.set_u16(g, WAIT, 160);
    animation::start(g, p, 7);
}

const INTRO_253A0: Steps = &[
    (0x253b4, |g, p| {
        if ram::SUB_STEP.get(g.b) == 14 {
            intro_253a0_start(g, p);
        }
    }),
    (0x253bc, intro_253a0_start),
    (0x253da, |g, p| {
        if countdown(g, p, WAIT) {
            next(g, p);
            p.set_u16(g, WAIT, 96);
            anim_0(g, p);
        } else if p.u16(g, WAIT) <= 40 {
            step_x(g, p, 4);
            anim_step(g, p);
        }
    }),
    (0x25406, |g, p| {
        if countdown(g, p, WAIT) {
            p.set_state(g, 0x0200_0000);
        }
        anim_step(g, p);
    }),
];

// ---- 0x2541a, 0x25504, 0x255ea, 0x2564a, 0x256c2, 0x257a4 -------------------------------------------

/// Wait of `WAIT` frames (without animation), then `WAIT` = 80.
fn wait_then_walk(g: &mut Ctx, p: Player) {
    if countdown(g, p, WAIT) {
        next(g, p);
        p.set_u16(g, WAIT, 80);
    }
}

const INTRO_2541A: Steps = &[
    (0x25434, |g, p| {
        appear(g, p, true, false);
        p.set_u8(g, 0x15c, 0);
        p.set_u16(g, WAIT, 224);
        animation::start(g, p, 2);
    }),
    (0x25458, wait_then_walk),
    (0x2546a, |g, p| {
        walk_right(g, p, WAIT, |g, p| {
            let index = p.u8(g, Player::INDEX) as u32;
            let wait = g.prog16(0x25492 + 2 * index);
            p.set_u16(g, WAIT, wait);
            start_stand(g, p);
        })
    }),
    (0x2549a, draw_weapon_after_wait),
    (0x254b8, pose_with_shout),
    (0x254ee, take_control),
];

const INTRO_25504: Steps = &[
    (0x25520, |g, p| {
        appear(g, p, false, false);
        p.set_u16(g, WAIT, 224);
        animation::start(g, p, 2);
    }),
    (0x2553a, wait_then_walk),
    (0x2554c, |g, p| walk_right(g, p, WAIT, anim_0)),
    (0x25568, |g, p| {
        if ram::SUB_STEP.get(g.b) == 8 {
            next(g, p);
            let wait = per_rank16(g, p, 0x2558e);
            p.set_u16(g, WAIT, wait);
            crate::families::effects_creators::for_players::halo(&mut g.leftovers, g.b, p.obj());
        }
        anim_step(g, p);
    }),
    (0x25596, |g, p| {
        if ram::SUB_STEP.get(g.b) >= 10 {
            draw_weapon_after_wait(g, p);
        } else {
            anim_step(g, p);
        }
    }),
    (0x255be, |g, p| then_anim(g, p, 0)),
    (0x255d4, take_control),
];

const INTRO_255EA: Steps = &[
    (0x255fe, |g, p| {
        appear(g, p, false, false);
        anim_0(g, p);
    }),
    (0x25612, |g, p| {
        if ram::SUB_STEP.get(g.b) == 6 {
            next(g, p);
            p.set_u8(g, ARMED, 1);
            animation::start(g, p, 51);
        } else {
            anim_step(g, p);
        }
    }),
    (0x25634, take_control),
];

const INTRO_2564A: Steps = &[
    (0x2565e, |g, p| {
        appear(g, p, false, true);
        super::hurt::hurt_animation(g, p, 16);
    }),
    (0x25674, |g, p| {
        if ram::PHASE.get(g.b) == PLAYING_PHASE {
            next(g, p);
            let wait = per_rank16(g, p, 0x25690);
            p.set_u16(g, WAIT, wait);
        }
    }),
    (0x25698, |g, p| {
        if waiting(g, p, WAIT) {
            return;
        }
        if anim_step(g, p) {
            p.set_state(g, 0x0200_0000);
            anim_0(g, p);
        }
    }),
];

const INTRO_256C2: Steps = &[
    (0x256dc, |g, p| {
        appear(g, p, false, false);
        p.set_u16(g, WAIT, 80);
        animation::start(g, p, 2);
    }),
    (0x256f6, |g, p| {
        if ram::SCRIPTED_B.get(g.b) != 0 || ram::SUB_STEP.get(g.b) == 6 {
            next(g, p);
        }
    }),
    (0x2570a, |g, p| {
        walk_right(g, p, WAIT, |g, p| {
            let wait = per_rank16(g, p, 0x25732);
            p.set_u16(g, WAIT, wait);
            start_stand(g, p);
        })
    }),
    (0x2573a, draw_weapon_after_wait),
    (0x25758, pose_with_shout),
    (0x2578e, take_control),
];

const INTRO_257A4: Steps = &[
    (0x257ba, |g, p| {
        appear(g, p, false, true);
        animation::start(g, p, 2);
    }),
    (0x257d0, |g, p| {
        if ram::SUB_STEP.get(g.b) == 6 {
            next(g, p);
            p.set_u16(g, WAIT, 80);
        }
    }),
    (0x257e4, |g, p| walk_right(g, p, WAIT, start_stand)),
    (0x257fe, take_control),
];

// ---- 0x25814, 0x258de, 0x25954, 0x25a24: jumps, slow walk, falls ------------------------------

const INTRO_25814: Steps = &[
    (0x25828, |g, p| {
        appear(g, p, false, true);
        let wait = per_rank16(g, p, 0x2584c);
        p.set_u16(g, WAIT, wait);
        animation::start(g, p, 2);
    }),
    (0x25854, |g, p| {
        walk_right(g, p, WAIT, |g, p| {
            let at = 0x2589a + 8 * p.u8(g, RANK) as u32;
            let (vx, vz, az, height) = (g.prog16(at), g.prog16(at + 2), g.prog16(at + 4), g.prog16(at + 6));
            p.set_u16(g, Obj::VX, vx);
            p.set_u16(g, Obj::AX, 0);
            p.set_u16(g, Obj::VZ, vz);
            p.set_u16(g, Obj::AZ, az);
            p.set_u16(g, Obj::Z, height);
            let y = p.u16(g, Obj::Y).wrapping_sub(height);
            p.set_u16(g, Obj::Y, y);
            anim_c(g, p, 2);
        })
    }),
    (0x258ba, |g, p| {
        super::walk::move_xz(g, p);
        if (p.u16(g, Obj::Z) as i16) <= 0 {
            p.set_state(g, 0x0200_0000);
            p.set_u32(g, Obj::Z, 0);
            start_stand(g, p);
        } else {
            anim_step(g, p);
        }
    }),
];

const INTRO_258DE: Steps = &[
    (0x258f0, |g, p| {
        appear(g, p, false, false);
        let slow = ram::SLOW_MODE.get(g.b) != 0;
        p.set_u16(g, WAIT, if slow { 128 } else { 80 });
        p.set_u16(g, Obj::VX, if slow { 1 } else { 2 });
        animation::start(g, p, if slow { 53 } else { 2 });
    }),
    (0x25924, |g, p| {
        let dx = p.u16(g, Obj::VX);
        step_x(g, p, dx);
        if !countdown(g, p, WAIT) {
            anim_step(g, p);
            return;
        }
        p.set_state(g, 0x0200_0000);
        if ram::SLOW_MODE.get(g.b) == 0 {
            start_stand(g, p);
        } else {
            animation::start(g, p, 53);
        }
    }),
];

/// Fall from the top of the screen: true on landing (animation 14); `+0x3b` is cleared as long as the
/// player is higher than `high`.
fn drop_in(g: &mut Ctx, p: Player, high: u16, clear_z: bool) {
    move_z(g, p);
    if (p.u16(g, Obj::Z) as i16) <= 0 {
        next(g, p);
        if clear_z {
            p.set_u32(g, Obj::Z, 0);
        }
        animation::start(g, p, 14);
        return;
    }
    anim_step(g, p);
    if p.u16(g, Obj::Z) > high {
        p.set_u8(g, 0x3b, 0);
    }
}

const INTRO_25954: Steps = &[
    (0x25968, |g, p| {
        appear(g, p, false, true);
        p.set_u16(g, Obj::VZ, (-256i16) as u16);
        p.set_u16(g, Obj::AZ, (-64i16) as u16);
        let at = 0x259b0 + 8 * p.u8(g, RANK) as u32;
        let (x, z, y, wait) = (g.prog16(at), g.prog16(at + 2), g.prog16(at + 4), g.prog16(at + 6));
        p.set_u16(g, Obj::X, x);
        p.set_u16(g, Obj::Z, z);
        p.set_u16(g, Obj::Y, y);
        p.set_u16(g, WAIT, wait);
        animation::start(g, p, 56);
        p.set_u8(g, 0x3b, 0);
    }),
    (0x259d0, |g, p| {
        if !waiting(g, p, WAIT) {
            drop_in(g, p, 80, false);
        }
    }),
    (0x25a08, |g, p| {
        if anim_step(g, p) {
            p.set_state(g, 0x0200_0000);
            p.set_u32(g, Obj::Z, 0);
            start_stand(g, p);
        }
    }),
];

/// 0x25a24: appearance at a fixed spot (table per rank: x, height, depth, direction), then action 0x2a
/// (jump in place) after 48 frames.
fn intro_25a24(g: &mut Ctx, p: Player) {
    if p.u8(g, Player::ACTION) != 0 {
        if countdown(g, p, WAIT) {
            p.set_state(g, 0x0200_2a00);
            p.set_u8(g, 0x3b, 1);
        }
        return;
    }
    next(g, p);
    p.set_u16(g, WAIT, 48);
    p.set_u8(g, Player::ACTIVE, 1);
    p.set_u8(g, ARMED, 1);
    let at = 0x25a6c + 8 * p.u8(g, RANK) as u32;
    let (x, z, y) = (g.prog16(at), g.prog16(at + 2), g.prog16(at + 4));
    let facing = g.prog8(at + 7);
    p.set_u16(g, Obj::X, x);
    p.set_u16(g, Obj::Z, z);
    p.set_u16(g, Obj::Y, y);
    p.set_u8(g, Player::FACING, facing);
    animation::start(g, p, 13);
    p.set_u8(g, 0x3b, 0);
}

// ---- 0x25aa2, 0x25b3e, 0x25ba0, 0x25c9e, 0x25d40, 0x25db4: walk-ins ----------------------

/// 0x25b20: at scene 41, one frame out of sixteen (according to the counter and the byte `A5+2`, like the original),
/// footstep effect in water (h).
fn splash(g: &mut Ctx, p: Player) {
    if ram::SCENE.get(g.b) == 41 && ram::FRAME_COUNTER.get(g.b).wrapping_add(ram::INPUTS_2.get(g.b)) & 15 == 0 {
        crate::families::effects_creators::for_players::wisp(&mut g.leftovers, g.b, p.obj());
    }
}

const INTRO_25AA2: Steps = &[
    (0x25ab8, |g, p| {
        appear(g, p, false, true);
        p.set_u16(g, WAIT, 224);
        animation::start(g, p, 2);
    }),
    (0x25ad4, wait_then_walk),
    (0x25ae6, |g, p| {
        step_x(g, p, 2);
        if countdown(g, p, WAIT) {
            next(g, p);
            anim_0(g, p);
        }
        splash(g, p);
        anim_step(g, p);
    }),
    (0x25b06, |g, p| {
        if ram::PHASE.get(g.b) == PLAYING_PHASE {
            p.set_state(g, 0x0200_0000);
        }
        splash(g, p);
        anim_step(g, p);
    }),
];

const INTRO_25B3E: Steps = &[
    (0x25b52, |g, p| {
        appear(g, p, false, true);
        p.set_u16(g, WAIT, 80);
        animation::start(g, p, 2);
    }),
    (0x25b6e, |g, p| walk_right(g, p, WAIT, anim_0)),
    (0x25b8a, take_control),
];

const INTRO_25BA0: Steps = &[
    (0x25bba, |g, p| {
        appear(g, p, false, false);
        let x = ram::CAMERA_X.get(g.b);
        let at = 0x25bf6 + 4 * p.u8(g, RANK) as u32;
        let (dx, dy) = (g.prog16(at), g.prog16(at + 2));
        p.set_u16(g, Obj::X, x.wrapping_add(dx));
        p.set_u16(g, Obj::Y, 280u16.wrapping_add(dy));
        p.set_u32(g, Obj::Z, 0);
        animation::start(g, p, 2);
    }),
    (0x25c06, |g, p| {
        if ram::SUB_STEP.get(g.b) == 6 {
            next(g, p);
            p.set_u16(g, TIMER_B, 80);
        }
    }),
    (0x25c1a, |g, p| {
        walk_right(g, p, TIMER_B, |g, p| {
            let wait = per_rank16(g, p, 0x25c4a);
            p.set_u16(g, WAIT, wait);
            crate::families::effects_creators::for_players::halo(&mut g.leftovers, g.b, p.obj());
            anim_0(g, p);
        })
    }),
    (0x25c52, draw_weapon_after_wait),
    (0x25c72, |g, p| then_anim(g, p, 0)),
    (0x25c88, take_control),
];

/// Walk after a wait: `WAIT` frames of waiting (without animation), then walk of `TIMER_B` frames.
fn delayed_walk(g: &mut Ctx, p: Player) {
    if waiting(g, p, WAIT) {
        return;
    }
    walk_right(g, p, TIMER_B, anim_0);
}

const INTRO_25C9E: Steps = &[
    (0x25cb6, |g, p| {
        appear(g, p, false, true);
        p.set_u16(g, WAIT, 80);
        animation::start(g, p, 2);
        if ram::NO_WALK_IN.get(g.b) == 0 {
            p.set_u8(g, Player::ACTION, 6);
            p.set_u16(g, TIMER_B, 480);
        }
    }),
    (0x25ce6, |g, p| walk_right(g, p, WAIT, anim_0)),
    (0x25d02, take_control),
    (0x25d18, |g, p| {
        if waiting(g, p, TIMER_B) {
            return;
        }
        walk_right(g, p, WAIT, anim_0);
    }),
];

const INTRO_25D40: Steps = &[
    (0x25d54, |g, p| intro_wait_walk(g, p, 72)),
    (0x25d76, delayed_walk),
    (0x25d9e, take_control),
];

const INTRO_25DB4: Steps = &[
    (0x25dc8, |g, p| intro_wait_walk(g, p, 64)),
    (0x25dea, delayed_walk),
    (0x25e12, take_control),
];

/// Appearance: wait of 224 frames, then walk of `walk` frames.
fn intro_wait_walk(g: &mut Ctx, p: Player, walk: u16) {
    appear(g, p, false, true);
    p.set_u16(g, WAIT, 224);
    p.set_u16(g, TIMER_B, walk);
    animation::start(g, p, 2);
}

// ---- 0x25e28, 0x25efc, 0x25f7e -----------------------------------------------------------------------

const INTRO_25E28: Steps = &[
    (0x25e3e, |g, p| {
        appear(g, p, false, true);
        p.set_u16(g, WAIT, 192);
        p.set_u16(g, Obj::VZ, (-256i16) as u16);
        p.set_u16(g, Obj::AZ, (-64i16) as u16);
        p.set_u16(g, Obj::Z, 256);
        let k = (p.u8(g, RANK) & 3) as u32;
        let delay = g.prog16(0x25e82 + 2 * k);
        p.set_u16(g, TIMER_B, delay);
        animation::start(g, p, 56);
        p.set_u8(g, 0x3b, 0);
    }),
    (0x25e8a, |g, p| {
        if waiting(g, p, WAIT) || waiting(g, p, TIMER_B) {
            return;
        }
        drop_in(g, p, 112, true);
    }),
    (0x25ed2, stand_after_anim),
    (0x25ee6, take_control),
];

/// 0x25efc: without `A5+0x5c`, wait for the signal `A5+0x51e`, then wait and walk.
fn intro_25efc(g: &mut Ctx, p: Player) {
    if ram::NO_VICTORY.get(g.b) == 0 {
        dispatch(g, p, 0x25f12, INTRO_25EFC);
    }
}

const INTRO_25EFC: Steps = &[
    (0x25f18, |g, p| {
        if ram::BOSS_SIGNAL.get(g.b) != 0 {
            appear(g, p, false, true);
            p.set_u16(g, WAIT, 192);
            p.set_u16(g, TIMER_B, 80);
        }
        animation::start(g, p, 2);
    }),
    (0x25f40, delayed_walk),
    (0x25f68, take_control),
];

const INTRO_25F7E: Steps = &[
    (0x25f9e, |g, p| {
        next(g, p);
        p.set_u8(g, ARMED, 0);
        if ram::NO_VICTORY.get(g.b) == 0 {
            p.set_u8(g, Player::ACTION, 16);
            p.set_u8(g, ARMED, 1);
        }
        p.set_u8(g, Player::ACTIVE, 1);
        p.set_u16(g, WAIT, 32);
        let (x, y) = (ram::CAMERA_X.get(g.b), ram::CAMERA_Y.get(g.b));
        let k = (p.u8(g, RANK) & 3) as u32;
        let (dx, dy) = (g.prog16(0x26004 + 4 * k), g.prog16(0x26006 + 4 * k));
        p.set_u16(g, Obj::X, x.wrapping_add(dx));
        p.set_u16(g, Obj::Y, y.wrapping_add(dy));
        p.set_u8(g, Player::FACING, if k & 1 != 0 { 0 } else { 0x20 });
        anim_0(g, p);
    }),
    (0x26014, |g, p| {
        if ram::SUB_STEP.get(g.b) == 8 && countdown(g, p, WAIT) {
            next(g, p);
            let k = (p.u8(g, RANK) & 3) as u32;
            let wait = g.prog16(0x26040 + 2 * k);
            p.set_u16(g, WAIT, wait);
            animation::start(g, p, 2);
        }
    }),
    (0x26048, |g, p| {
        let dx: u16 = if p.u8(g, Player::FACING) != 0 { 2 } else { 0xfffe };
        step_x(g, p, dx);
        if countdown(g, p, WAIT) {
            next(g, p);
            animation::start(g, p, 8);
        } else {
            anim_step(g, p);
        }
    }),
    (0x26070, |g, p| {
        if anim_step(g, p) {
            next(g, p);
        }
    }),
    (0x2607e, |g, p| {
        if ram::SUB_STEP.get(g.b) == 12 {
            next(g, p);
            p.set_u16(g, WAIT, 64);
        }
    }),
    (0x26092, |g, p| {
        if countdown(g, p, WAIT) {
            next(g, p);
            animation::start(g, p, 9);
        }
    }),
    (0x260a6, |g, p| {
        if anim_step(g, p) {
            next(g, p);
            p.set_u8(g, ARMED, 1);
            animation::start(g, p, 51);
        }
    }),
    (0x260c2, |g, p| then_anim(g, p, 0)),
    (0x260d8, take_control),
];
