//! Climbing (action 0x1a) and the exit from the scene (action 0x20).

use crate::world::objects::Obj;
use crate::actors::player::Player;
use super::*;
use crate::actors::player::tables::{player_data, word, long, Aligned, Read};

// ---- climb (action 0x1a, 0x35e56) (h) ---------------------------------------------------------

/// Climbing fields.
pub(super) mod climb_field {
    /// Step phase (0: to prepare).
    pub const PHASE: u32 = 0x1e0;
    /// Frames before the next oscillation step (modulo 4).
    pub const SWAY_TIMER: u32 = 0x1e1;
    /// Orientation at the start of the lateral movement.
    pub const SIDE: u32 = 0x1e2;
    /// Rank in the oscillation table (`+0x4c`), modulo 32.
    pub const SWAY: u32 = 0x2d;
    pub const SPEED_TABLE: u32 = 0x4c;
    /// Object in hand (`+0x140`) and number of uses (`+0x15d`) of the elf's special move.
    pub const HELD_ITEM: u32 = 0x140;
}


/// Action 0x1a (0x35e56): climb (h). The player rises in Z through an oscillation table, moves
/// according to the directions (up: climbs, down: descends to the ground, sides: walk or run), and can
/// attack (button 1) or, as an elf with a wand, cast a spell (button 4).
pub fn climb(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::STEP) {
        0 => climb_start(g, p),
        2 => climb_hold(g, p),
        4 => climb_up(g, p),
        6 => climb_down(g, p),
        n @ (8 | 0xa | 0xc | 0xe) => climb_side(g, p, n),
        0x10 => climb_attack(g, p),
        0x12 => climb_spell(g, p),
        v => unreachable!("0x35e56 : value {v:#x} outside the original jump table"),
    }
    if matches!(p.u8(g, Player::STEP), 0 | 0x10 | 0x12) {
        return;
    }
    let pressed = p.u8(g, Player::KEYS_NEW);
    if pressed & 0x10 != 0 {
        p.set_u8(g, Player::STEP, 0x10);
        let character = p.u8(g, Player::CHARACTER) as u32;
        let anim = player_data().climb_attacks.at8(g.b, character);
        animation::start_plain(g, p, anim as u16);
    } else if pressed & 0x80 != 0
        && p.class(g) == 2
        && p.u8(g, Player::SPECIAL_COUNT) == 2
        && (0x24..=0x26).contains(&p.u8(g, climb_field::HELD_ITEM))
    {
        p.set_u8(g, Player::STEP, 0x12);
        p.set_u8(g, Player::TIMER_6F, 0x1e);
        animation::start_plain(g, p, 0x22);
    }
}

/// Return to hold (step 2), phase to prepare.
pub(super) fn climb_to_hold(g: &mut Ctx, p: Player) {
    p.set_u8(g, Player::STEP, 2);
    p.set_u8(g, climb_field::PHASE, 0);
}

/// Step 0: start (climb speed, oscillation table 1), then climb up to a height of 0x18.
pub(super) fn climb_start(g: &mut Ctx, p: Player) {
    if p.u8(g, climb_field::PHASE) == 0 {
        p.set_u8(g, climb_field::PHASE, 2);
        p.set_u32(g, Obj::VZ, 0x0100_0000);
        p.set_u8(g, climb_field::SWAY_TIMER, 4);
        p.set_u8(g, climb_field::SWAY, 0);
        crate::world::objects::motion::select_speed(g.b, p.obj(), &crate::world::objects::motion::SPEEDS_B, 1);
        return animation::start(g, p, 0x3c);
    }
    move_z(g, p);
    if p.u16(g, Obj::Z) > 0x18 {
        next_step(g, p);
        p.set_u8(g, climb_field::PHASE, 0);
    }
    animation::step(g, p);
}

/// Step 2: hold: the player oscillates in height (table `+0x4c`); on the ground, they resume the pose.
pub(super) fn climb_hold(g: &mut Ctx, p: Player) {
    if p.u8(g, climb_field::PHASE) == 0 {
        p.set_u8(g, climb_field::PHASE, 2);
        return animation::start(g, p, 0x3b);
    }
    let sway = p.u8(g, climb_field::SWAY) as u32;
    let table = p.u32(g, climb_field::SPEED_TABLE);
    let vz = word(g.b, table + 4 * sway + 2);
    p.set_u16(g, Obj::VZ, vz);
    let z = (p.u32(g, Obj::Z) as i32).wrapping_add(((vz as u32) << 16) as i32 >> 8);
    p.set_u32(g, Obj::Z, z as u32);
    if z < 0 {
        p.set_u32(g, Obj::Z, 0);
        to_stand(g, p);
        return;
    }
    let timer = p.u8(g, climb_field::SWAY_TIMER).wrapping_sub(1) & 3;
    p.set_u8(g, climb_field::SWAY_TIMER, timer);
    if timer == 0 {
        p.set_u8(g, climb_field::SWAY_TIMER, 4);
        let sway = (p.u8(g, climb_field::SWAY) + 1) & 0x1f;
        p.set_u8(g, climb_field::SWAY, sway);
    }
    climb_directions(g, p);
}

/// `0x35f92`: choice of the step according to the held directions (frozen game: descend).
pub(super) fn climb_directions(g: &mut Ctx, p: Player) {
    if ram::FROZEN.get(g.b) != 0 {
        p.set_u8(g, Player::STEP, 6);
        return animation::start(g, p, 0x3d);
    }
    let dir = p.u8(g, Player::KEYS_HELD) & 0xf;
    if dir == 0 {
        animation::step(g, p);
        return;
    }
    p.set_u8(g, climb_field::PHASE, 0);
    if dir & 3 == 0 {
        // vertical: bit 3 climbs, otherwise descends
        let (step, anim) = if dir & 8 != 0 { (4, 0x3c) } else { (6, 0x3d) };
        p.set_u8(g, Player::STEP, step);
        return animation::start(g, p, anim);
    }
    let facing = if dir & 1 != 0 { 0x20 } else { 0 };
    let (step, anim) = if facing == p.u8(g, Player::FACING) { (8, 0x3e) } else { (0xa, 0x3f) };
    p.set_u8(g, Player::STEP, step);
    let f = p.u8(g, Player::FACING);
    p.set_u8(g, climb_field::SIDE, f);
    animation::start(g, p, anim);
}

/// Step 4: climb as long as up is held (up to 0x80 above the top of the camera).
pub(super) fn climb_up(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::KEYS_HELD) & 0xf {
        0 => {
            climb_to_hold(g, p);
            animation::step(g, p);
        }
        8 => {
            let cam = if ram::USE_ALT_CAMERA.get(g.b) != 0 { ram::CAMERA_ALT_Y.get(g.b) } else { ram::CAMERA_Y.get(g.b) };
            let top = cam.wrapping_add(0x80);
            let height = p.u16(g, Obj::Y).wrapping_add(p.u16(g, Obj::Z));
            if height as i16 <= top as i16 {
                let z = p.u16(g, Obj::Z).wrapping_add(2);
                p.set_u16(g, Obj::Z, z);
            }
            animation::step(g, p);
        }
        _ => climb_directions(g, p),
    }
}

/// Step 6: descend as long as down is held (or game frozen); on the ground, the pose.
pub(super) fn climb_down(g: &mut Ctx, p: Player) {
    if ram::FROZEN.get(g.b) == 0 {
        match p.u8(g, Player::KEYS_HELD) & 0xf {
            0 => {
                climb_to_hold(g, p);
                animation::step(g, p);
                return;
            }
            4 => {}
            _ => return climb_directions(g, p),
        }
    }
    let z = p.u16(g, Obj::Z).wrapping_sub(2);
    p.set_u16(g, Obj::Z, z);
    if (z as i16) < 0 {
        p.set_u32(g, Obj::Z, 0);
        to_stand(g, p);
        return;
    }
    animation::step(g, p);
}

/// Steps 8 and 0xa (walk), 0xc and 0xe (run, limited by `+0x69`): lateral movement as long as the
/// starting direction (8, 0xc) or the opposite one (0xa, 0xe) is held; otherwise change of direction.
pub(super) fn climb_side(g: &mut Ctx, p: Player, step: u8) {
    let run = step >= 0xc;
    if run {
        let left = p.u8(g, Player::PARAM).wrapping_sub(1);
        p.set_u8(g, Player::PARAM, left);
        if left == 0 {
            climb_to_hold(g, p);
            animation::step(g, p);
            return;
        }
    }
    let dir = p.u8(g, Player::KEYS_HELD) & 0xf;
    if dir == 0 {
        climb_to_hold(g, p);
        animation::step(g, p);
        return;
    }
    if dir & 3 == 0 {
        return climb_directions(g, p);
    }
    let first = step & 2 == 0;
    let side = p.u8(g, climb_field::SIDE) != 0;
    let bit = if side == first { 0 } else { 1 };
    if dir & 3 & (1 << bit) == 0 {
        let (next, anim) = if first { (0xa, 0x3f) } else { (8, 0x3e) };
        p.set_u8(g, Player::STEP, next);
        return animation::start(g, p, anim);
    }
    if run {
        run_speed(g, p);
    } else {
        walk_speed(g, p);
    }
    move_xy(g, p);
    animation::step(g, p);
}

/// Step 0x10: attack from the climb; the animation event launches its effect.
pub(super) fn climb_attack(g: &mut Ctx, p: Player) {
    if animation::step(g, p) {
        climb_to_hold(g, p);
        animation::step(g, p);
        return;
    }
    let event = p.u8(g, field::ANIM_EVENT);
    if event != 0 {
        p.set_u8(g, field::ANIM_EVENT, 0);
        super::super::spawn::weapon_trail(g, p, event);
    }
}

/// Step 0x12: elf's wand spell (`+0x140` = 0x24 to 0x26), cast at the gesture.
pub(super) fn climb_spell(g: &mut Ctx, p: Player) {
    animation::step(g, p);
    let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    if (left as i8) < 0 {
        climb_to_hold(g, p);
    }
    if p.u8(g, field::GESTURE) == 0 {
        return;
    }
    p.set_u8(g, field::GESTURE, 0);
    let shot = match p.u8(g, climb_field::HELD_ITEM) {
        0x24 => Thrown::C,
        0x25 => Thrown::D,
        0x26 => Thrown::E,
        _ => return,
    };
    super::super::spawn::thrown(g, p, shot);
}

// ---- exit (action 0x20, 0x3632c) (h) ----------------------------------------------------------

/// Exit fields.
pub(super) mod exit_field {
    /// Frames remaining before the player is hidden.
    pub const TIMER: u32 = 0x1e8;
    /// Fade step of their palette (0 to 3) and frames before the next step.
    pub const FADE: u32 = 0x1e9;
    pub const FADE_TIMER: u32 = 0x1ea;
    /// Object toward which the player jumps (word, `+0x86`).
    pub const TARGET: u32 = 0x86;
}

/// Fade palettes: per step (0x180 bytes), per character (32 bytes); destination per character.
pub(crate) const EXIT_PALETTES: u32 = 0x3663e;
pub(crate) const EXIT_PALETTE_SLOTS: u32 = 0x36626;

/// Action 0x20 (0x3632c): the player leaves the scene (h). The starting step selects the exit: walk
/// down, up, left, right or in place (0 to 8), sinking (0xa) or jump toward the object
/// `+0x86` (0xc); the player is hidden at the end of the delay, while their palette fades out in three steps.
pub fn exit_scene(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::STEP) {
        0 => exit_walk(g, p, 0x1e, 0, 0x0001_0000, None, 0x25),
        2 => exit_walk(g, p, 0x1e, 0, 0xffff_0000, None, 0x26),
        4 => exit_walk(g, p, 0x28, 0xfe00_0000, 0, Some(0), 2),
        6 => exit_walk(g, p, 0x28, 0x0200_0000, 0, Some(0x20), 2),
        8 => {
            exit_start(g, p, 0xe, 0x1e);
            p.set_u8(g, Player::FACING, 0x20);
            p.set_u32(g, Obj::VX, 0);
            p.set_u32(g, Obj::VY, 0);
            animation::start(g, p, 2);
        }
        0xa => {
            p.set_u8(g, Player::STEP, 0x12);
            p.set_u8(g, Obj::ACTIVE, 2);
            p.set_u8(g, exit_field::TIMER, 0x30);
            animation::start(g, p, 0x38);
            p.set_u8(g, Player::GROUND_MARK, 0);
        }
        0xc => {
            exit_start(g, p, 0x14, 0x1e);
            p.set_u16(g, Obj::VX, 0xff00);
            p.set_u16(g, Obj::AX, 0);
            p.set_u16(g, Obj::VZ, 0x500);
            p.set_u16(g, Obj::AZ, 0xffc8);
            let target = p.u16(g, exit_field::TARGET) as i16 as i32 as u32;
            p.set_u8(g, Player::FACING, 0);
            if (p.u16(g, Obj::X).wrapping_sub(rd16(g.b, target + Obj::X)) as i16) < 0 {
                p.set_u8(g, Player::FACING, 0x20);
                let v = p.u16(g, Obj::VX).wrapping_neg();
                p.set_u16(g, Obj::VX, v);
            }
            crate::world::objects::anim::start_by_character(g.b, p.obj(), &crate::world::objects::anim::SETS_BY_VARIANT, 2);
        }
        0xe => {
            move_xy(g, p);
            animation::step(g, p);
            if exit_countdown(g, p) {
                next_step(g, p);
                p.set_u8(g, Player::GROUND_MARK, 0);
            }
            exit_fade(g, p);
        }
        0x10 => {}
        0x12 => {
            let z = p.u16(g, Obj::Z).wrapping_sub(4);
            p.set_u16(g, Obj::Z, z);
            animation::step(g, p);
            if exit_countdown(g, p) {
                p.set_u8(g, Player::STEP, 0x10);
            }
            p.set_u8(g, Player::GROUND_MARK, 0);
        }
        0x14 => {
            move_z(g, p);
            let target = p.u16(g, exit_field::TARGET) as i16 as i32 as u32;
            if p.u16(g, Obj::X) != rd16(g.b, target + Obj::X) {
                move_x(g, p);
            }
            animation::step(g, p);
            if exit_countdown(g, p) {
                p.set_u8(g, Player::STEP, 0x10);
            }
            p.set_u8(g, Player::GROUND_MARK, 0);
            exit_fade(g, p);
        }
        v => unreachable!("0x3632c : value {v:#x} outside the original jump table"),
    }
}

/// Start of an exit: next step `step`, player displayed without shadow, delay, fade ready.
pub(super) fn exit_start(g: &mut Ctx, p: Player, step: u8, timer: u8) {
    p.set_u8(g, Player::STEP, step);
    p.set_u8(g, Obj::ACTIVE, 2);
    p.set_u8(g, exit_field::TIMER, timer);
    p.set_u8(g, exit_field::FADE, 0);
    p.set_u8(g, exit_field::FADE_TIMER, 1);
}

/// Exit by walking (X and Y speeds as longs, optional orientation); in slow-motion mode, animation 0x35.
pub(super) fn exit_walk(g: &mut Ctx, p: Player, timer: u8, vx: u32, vy: u32, facing: Option<u8>, anim: u16) {
    exit_start(g, p, 0xe, timer);
    if let Some(f) = facing {
        p.set_u8(g, Player::FACING, f);
    }
    p.set_u32(g, Obj::VX, vx);
    p.set_u32(g, Obj::VY, vy);
    let anim = if ram::SLOW_MODE.get(g.b) != 0 { 0x35 } else { anim };
    animation::start(g, p, anim);
}

/// The delay runs out; at zero, the player is hidden (true).
pub(super) fn exit_countdown(g: &mut Ctx, p: Player) -> bool {
    let left = p.u8(g, exit_field::TIMER).wrapping_sub(1);
    p.set_u8(g, exit_field::TIMER, left);
    if left != 0 {
        return false;
    }
    let flags = p.u8(g, Player::FLAGS) | 2;
    p.set_u8(g, Player::FLAGS, flags);
    true
}

/// One fade step every 12 frames (three steps): the character's palette copied from the table.
pub(super) fn exit_fade(g: &mut Ctx, p: Player) {
    let stage = p.u8(g, exit_field::FADE);
    if stage > 2 {
        return;
    }
    let left = p.u8(g, exit_field::FADE_TIMER).wrapping_sub(1);
    p.set_u8(g, exit_field::FADE_TIMER, left);
    if left != 0 {
        return;
    }
    let character = p.u8(g, Player::CHARACTER) as u32;
    let from = EXIT_PALETTES + (0x180 * stage as u32 + (character << 5)) as u16 as i16 as i32 as u32;
    let slot = player_data().exit_palette_slots.word_at(g.b, 2 * character) as i16 as i32 as u32;
    let to = 0x91c000u32.wrapping_add(slot);
    for i in 0..8 {
        let v = long(g.b, from + 4 * i);
        wr32(g.b, to + 4 * i, v);
    }
    p.set_u8(g, exit_field::FADE_TIMER, 0xc);
    p.set_u8(g, exit_field::FADE, stage + 1);
}
