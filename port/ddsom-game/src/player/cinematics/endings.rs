//! Step tables of the end-of-scene cinematics (routines 0x2411a to 0x24de6).

use crate::objects::Obj;
use crate::player::Player;
use super::*;
use crate::player::tables::{pc8, pc16, word, long};

/// Countdown of the palette fade of the exit (player scratch).
const FADE_TIMER: u32 = 0x1e6;
/// Palette set reached by the fade (player scratch).
const FADE_SET: u32 = 0x1e7;
/// Wait between two fade steps (player scratch).
const FADE_WAIT: u32 = 0x1e8;

// ---- 0x2411a ----------------------------------------------------------------------------------

pub(super) const END_2411A: Steps = &[
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
        let (x, y) = (pc16(g.b, at), pc16(g.b, at + 2));
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

pub(super) const END_2434E: Steps = &[
    (0x24372, |g, p| {
        start_victory(g, p);
        let scene = ram::SCENE.get(g.b);
        if scene == 46 {
            p.set_u8(g, Player::MAGIC_USED, 1);
        }
        if matches!(scene, 46 | 58 | 66) {
            elf_effect(g, p);
        }
    }),
    (0x243c4, wait_done),
];

// ---- 0x243d6 ----------------------------------------------------------------------------------

pub(super) const END_243D6: Steps = &[
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
pub(super) fn final_facing(g: &mut Ctx, p: Player, table: u32) -> u8 {
    let k = rank(g, p) & 3;
    pc8(g.b, table + k)
}

pub(super) const END_2447E: Steps = &[
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
        super::super::hurt::fall(g, p);
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
pub(super) const EXIT_X: u16 = 220;
pub(super) const EXIT_Y: u16 = 384;

pub(super) const END_245EC: Steps = &[
    (0x2461c, start_victory),
    (0x24636, |g, p| {
        if scene_step(g) != 16 {
            anim_step(g, p);
            return;
        }
        next(g, p);
        let k = rank(g, p);
        let delay = pc8(g.b, 0x24660 + k);
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
            p.set_u8(g, FADE_SET, 0);
            p.set_u8(g, FADE_WAIT, 1);
            p.set_u8(g, FADE_TIMER, 48);
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
use super::super::moves::{EXIT_PALETTES as FADE_PALETTES, EXIT_PALETTE_SLOTS as FADE_TARGETS};
pub(super) const PALETTE_RAM: u32 = 0x91c000;

/// 0x24734: disappearance: after 48 frames the player is hidden; every 12 frames, their palette moves
/// to the next set (three sets).
pub(super) fn fade_out(g: &mut Ctx, p: Player) {
    anim_step(g, p);
    let left = p.u8(g, FADE_TIMER).wrapping_sub(1);
    p.set_u8(g, FADE_TIMER, left);
    if left == 0 {
        next(g, p);
        let flags = p.u8(g, Player::FLAGS) | 2;
        p.set_u8(g, Player::FLAGS, flags);
    }
    p.set_u8(g, Player::GROUND_MARK, 0);
    let set = p.u8(g, FADE_SET);
    if set > 2 {
        return;
    }
    let wait = p.u8(g, FADE_WAIT).wrapping_sub(1);
    p.set_u8(g, FADE_WAIT, wait);
    if wait != 0 {
        return;
    }
    let character = p.u8(g, Player::CHARACTER) as u32;
    let from = FADE_PALETTES + 384 * set as u32 + 32 * character;
    let to = PALETTE_RAM.wrapping_add(word(g.b, FADE_TARGETS + 2 * character) as i16 as i32 as u32);
    for i in 0..8 {
        let v = long(g.b, from + 4 * i);
        wr32(g.b, to + 4 * i, v);
    }
    p.set_u8(g, FADE_WAIT, 12);
    p.set_u8(g, FADE_SET, set + 1);
}

// ---- 0x247b2 ----------------------------------------------------------------------------------

pub(super) const END_247B2: Steps = &[
    (0x247e4, |g, p| {
        next(g, p);
        let anim = if p.u8(g, Player::HOLY) != 0 { 36 } else { 35 };
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

pub(super) const END_2495C: Steps = &[
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

pub(super) const END_249EA: Steps = &[
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

pub(super) const END_24A56: Steps = &[
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

pub(super) const END_24ABA: Steps = &[
    (0x24ade, start_victory),
    (0x24af8, |g, p| {
        if end_done(g) {
            next(g, p);
            reset_speed_class(g, p);
        }
        anim_step(g, p);
    }),
];

pub(super) const END_24B18: Steps = &[
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

pub(super) const END_24BE2: Steps = &[
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
            let delay = pc16(g.b, 0x24d50 + 2 * k);
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
pub(super) fn go_to_gate(g: &mut Ctx, p: Player) {
    next(g, p);
    p.set_u16(g, TARGET_X, 192);
    p.set_u16(g, TARGET_Y, 380);
    start_stand(g, p);
}

// ---- 0x24de6 ----------------------------------------------------------------------------------

pub(super) const END_24DE6: Steps = &[
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
        let (dx, dy) = (pc16(g.b, at), pc16(g.b, at + 2));
        p.set_u16(g, Obj::X, x.wrapping_add(dx));
        p.set_u16(g, Obj::Y, y.wrapping_add(dy));
        victory_pose(g, p);
    }),
    (0x24e76, |g, p| stand_at_scene_step(g, p, 20)),
    (0x24e8e, wait_done),
];
