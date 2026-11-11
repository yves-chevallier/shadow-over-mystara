//! Guard, slow end, rare poses (actions 0x26, 0x2c, 0x30), hop, fall-in, push and spell fizzle.

use crate::world::objects::Obj;
use crate::actors::player::Player;
use super::*;
use crate::actors::player::tables::{player_data, pc16, Aligned};

// ---- guard (actions 0x16 and 0x18, 0x35d52 and 0x35dca) ---------------------------------------------

/// Shield guard as long as the button `held` (bit 7: special, bit 4: attack) is held: the
/// player slowly backs away (directions opposite to their original direction, or vertical) without
/// turning; when released, they resume the pose (or attack, or jump).
pub fn guard(g: &mut Ctx, p: Player, held: u8) {
    if p.u8(g, Player::STEP) == 0 {
        next_step(g, p);
        p.set_u8(g, Player::SUBACTION, 0);
        let facing = p.u8(g, Player::FACING) ^ 0x20;
        p.set_u8(g, field::HELD_FACING, facing);
        animation::start(g, p, 25);
        return;
    }
    let keys = p.u8(g, Player::KEYS_HELD);
    if keys & held == 0 {
        if !attack_button(g, p) && !jump_button(g, p) {
            to_stand(g, p);
        }
        return;
    }
    let dir = keys & 0xf;
    let moving = dir != 0 && (dir & 3 == 0 || facing_for(dir) == p.u8(g, field::HELD_FACING));
    if !moving {
        animation::start(g, p, 25);
        return;
    }
    crawl_speed(g, p);
    move_xy(g, p);
    animation::step(g, p);
}

// ---- action 0x28 (0x36be0): end of slowdown (h) --------------------------------------------

/// Countdown of action 0x28 (`+0x1e8`).
pub(super) const SLOW_END_TIMER: u32 = 0x1e8;

/// 0x36be0: the slowed player stays 64 frames in pose 0x39, then the slowdown (`+0x66`) ends and they
/// stand back up.
pub fn slow_end(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, SLOW_END_TIMER, 0x40);
            animation::start(g, p, 0x39);
        }
        2 => {
            let left = p.u8(g, SLOW_END_TIMER).wrapping_sub(1);
            p.set_u8(g, SLOW_END_TIMER, left);
            if left == 0 {
                p.set_u8(g, Player::SLOW, 0);
                to_stand(g, p);
            }
        }
        v => unreachable!("0x36be0 : value {v:#x} outside the original jump table"),
    }
}

// ---- actions 0x26, 0x2c and 0x30: rare poses ----------------------------------------------------

/// Countdown of the rare poses (`+0x1e8`, byte) and number of the next puff.
pub(super) const RARE_TIMER: u32 = 0x1e8;

/// 0x36b88: action 0x26, the cursed weapon falls (`checks::item_curse`, `common::curse_check`, `combat`): pose 0x23
/// and sound 0xe24d8, then 192 frames of animation; at each gesture (`+0x33`), flash (0xb97ba) and SET object 0x29
/// (0xc8312) marked at `+0x42`. At the end, the slowdown (`+0x66`) ends and the player stands back up.
pub fn curse_drop(g: &mut Ctx, p: Player) {
    if p.u8(g, Player::STEP) == 0 {
        next_step(g, p);
        p.set_u8(g, RARE_TIMER, 0xc0);
        animation::start(g, p, 0x23);
        g.sound(ext::sound::FANFARE, p);
        return;
    }
    animation::step(g, p);
    let left = p.u8(g, RARE_TIMER).wrapping_sub(1);
    p.set_u8(g, RARE_TIMER, left);
    if left == 0 {
        p.set_u8(g, Player::SLOW, 0);
        to_stand(g, p);
        return;
    }
    if p.u8(g, field::GESTURE) == 0 {
        return;
    }
    p.set_u8(g, field::GESTURE, 0);
    crate::actors::families::effects_creators::for_players::posed_flash_b(&mut g.leftovers, g.b, p.obj());
    // 0xc8312, D0.b = 0x29
    if let Some(set) = crate::actors::families::creators::scene_set(g.b, rom::SCENE_SET_VARIANT) {
        wr8(g.b, set.0 + 0x42, 1);
    }
}

/// Dust puffs of action 0x2c (0x36d2c, rows of 4 words: D0, D1, D2 of 0xb10c8, height).
pub(super) const PUFFS: u32 = 0x36d2c;

/// 0x36cb8: action 0x2c (an effect gives it to the player, `families/effects_f.rs`): pose 0x35, six puffs of
/// dust (0xb10c8) spaced 6 frames apart, then 6 frames of waiting before standing back up.
pub fn dust_puffs(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, RARE_TIMER, 0);
            p.set_u8(g, Player::TIMER_6F, 1);
            animation::start(g, p, 0x35);
        }
        2 => {
            let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
            p.set_u8(g, Player::TIMER_6F, left);
            if left != 0 {
                return;
            }
            let row = PUFFS + 8 * p.u8(g, RARE_TIMER) as u32;
            let height = pc16(g.b, row + 6);
            let (variant, setting, vx) = (pc16(g.b, row) as u8, pc16(g.b, row + 2), pc16(g.b, row + 4));
            // the speeds and the height stay in D2-D4 (leftovers of the original)
            crate::world::mem::set_low_word(&mut g.leftovers.d2, vx);
            g.leftovers.d4 = height as u32;
            crate::world::mem::set_low_word(&mut g.leftovers.d3, 0xffff);
            // Original glitch: only the carry is tested; in the scene without shocks (41: no effect, carry clear) the
            // height is written through the A1 left by the code before. That A1 is not known here (no scenario plays
            // it): the write is left out (`docs/code-mort.md`, "Sortie du Cpu, partie 6").
            if let Some(puff) = g.shock(p, variant, setting, vx, 0xffff) {
                wr16(g.b, puff.0 + Obj::Z, height);
            }
            p.set_u8(g, Player::TIMER_6F, 6);
            let n = p.u8(g, RARE_TIMER).wrapping_add(1);
            p.set_u8(g, RARE_TIMER, n);
            if n == 6 {
                next_step(g, p);
                p.set_u8(g, Player::TIMER_6F, 6);
            }
        }
        4 => {
            let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
            p.set_u8(g, Player::TIMER_6F, left);
            if left == 0 {
                to_stand(g, p);
            }
        }
        v => unreachable!("0x36cb8 : value {v:#x} outside the original jump table"),
    }
}

/// 0x36f1a: action 0x30 (the player hit a crate, `families/boxes.rs`): pose 0x36 until its end, then the
/// slowdown (`+0x66`) ends and the player stands back up.
pub fn box_recoil(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            animation::start(g, p, 0x36);
        }
        2 => {
            if animation::step(g, p) {
                p.set_u8(g, Player::SLOW, 0);
                to_stand(g, p);
            }
        }
        v => unreachable!("0x36f1a : value {v:#x} outside the original jump table"),
    }
}

/// Natives of the rare poses, for the differential validation (`fuzz_player player_action`): `actions::act` calls them
/// directly.
macro_rules! rare_native {
    ($name:ident, $entry:expr, $label:expr, $f:path) => {
        pub const $name: crate::Native = crate::Native {
            entry: $entry,
            name: $label,
            run: crate::Body::Handler(|b, a| super::super::Ctx::on_player(b, a, $f)),
            touches_sound: false,
            outputs: 0,
        };
    };
}
rare_native!(CURSE_DROP, 0x36b88, "player_action_26", curse_drop);
rare_native!(DUST_PUFFS, 0x36cb8, "player_action_2c", dust_puffs);
rare_native!(BOX_RECOIL, 0x36f1a, "player_action_30", box_recoil);

// ---- action 0x2a (0x36c12): jump in place with bounce (h) ---------------------------------------

/// 0x36c12: takeoff, landing then walk on the height; the height oscillates by 2 pixels.
pub fn hop(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            p.set_u8(g, Player::TIMER_6F, 0);
            p.set_u32(g, Obj::VX, 0);
            p.set_u16(g, Obj::VZ, (-0x900i16) as u16);
            p.set_u16(g, Obj::AZ, 40);
            animation::start(g, p, 56);
        }
        2 => {
            move_z(g, p);
            if p.u16(g, Obj::Z) as i16 <= 0 {
                next_step(g, p);
                p.set_u32(g, Obj::Z, 0);
                p.set_u32(g, Obj::VZ, 0);
            }
            animation::step(g, p);
        }
        4 => {
            walk_speed(g, p);
            move_x(g, p);
            let y = p.u16(g, Obj::Y).wrapping_sub(4);
            p.set_u16(g, Obj::Y, y);
            animation::step(g, p);
            p.set_u8(g, Player::GROUND_MARK, 0);
            clamp_x(g.b, p);
        }
        v => unreachable!("0x36c12 : value {v:#x} outside the original jump table"),
    }
    p.set_u8(g, Player::GROUND_MARK, 0);
    let z = p.u16(g, Obj::Z).wrapping_add(2);
    p.set_u16(g, Obj::Z, z);
    let phase = p.u8(g, Player::TIMER_6F).wrapping_add(1) & 1;
    p.set_u8(g, Player::TIMER_6F, phase);
    if phase == 0 {
        let z = p.u16(g, Obj::Z).wrapping_sub(4);
        p.set_u16(g, Obj::Z, z);
    }
}

// ---- action 0x24 (0x36b26): fall to the ground (h) -----------------------------------------------

/// 0x36b26: the player falls (animation 13), the fall follows the speed in X, Y and Z until the height
/// is no longer positive; they then touch the ground (animation 14, landing sound) then, the animation over,
/// stand back up. Step 2 (animation 18) is never reached: step 0 advances by two notches.
pub fn fall_in(g: &mut Ctx, p: Player) {
    match p.u8(g, Player::STEP) {
        0 => {
            next_step(g, p);
            next_step(g, p);
            animation::start(g, p, 13);
        }
        2 => {
            next_step(g, p);
            animation::start(g, p, 18);
        }
        4 => {
            move_xyz(g, p);
            if p.u16(g, Obj::Z) as i16 > 0 {
                return;
            }
            next_step(g, p);
            p.set_u32(g, Obj::Z, 0);
            animation::start(g, p, 14);
            g.sound(ext::sound::LANDING, p);
        }
        6 => {
            if animation::step(g, p) {
                to_stand(g, p);
            }
        }
        v => unreachable!("0x36b26 : value {v:#x} outside the original jump table"),
    }
}

// ---- push (action 0x2e, 0x36d68) ------------------------------------------------------------

/// 0x36d68: pushes the linked object (`+0x86`) in one of the four directions (steps 2 to 8: up, down,
/// left, right) as long as the direction is held.
pub fn push(g: &mut Ctx, p: Player) {
    let step = p.u8(g, Player::STEP);
    let key = match step {
        0 => {
            let first = p.u8(g, Player::CONTACT);
            p.set_u8(g, Player::STEP, first);
            p.set_u8(g, Player::TIMER_6F, 32);
            animation::start(g, p, 58);
            p.set_u8(g, field::GESTURE, 0);
            return;
        }
        2 => 8,
        4 => 4,
        6 => 2,
        8 => 1,
        v => unreachable!("0x36d68 : value {v:#x} outside the original jump table"),
    };
    if p.u8(g, Player::KEYS_HELD) & key == 0 {
        let held = p.u32(g, HELD);
        wr8(g.b, held + 0x66, 0);
        wr16(g.b, held + 0x6c, 0);
        to_stand(g, p);
        return;
    }
    let wait = p.u8(g, Player::TIMER_6F);
    if wait != 0 {
        p.set_u8(g, Player::TIMER_6F, wait - 1);
        if wait - 1 != 0 {
            return;
        }
        p.set_u8(g, field::GESTURE, 1);
    }
    animation::step(g, p);
    if step >= 6 {
        push_dust(g, p);
    }
    if p.u8(g, field::ANIM_EVENT) != 0 {
        p.set_u8(g, field::ANIM_EVENT, 0);
        g.sound(rom::PUSH_SOUND, p);
    }
}

/// 0x36eb0: dust of the pushing gesture (settings per class, table 0x36eea).
pub(super) fn push_dust(g: &mut Ctx, p: Player) {
    if !dust_turn(g, p) || p.u8(g, field::GESTURE) == 0 {
        return;
    }
    let (t, at) = (&player_data().slow_end_dust, 8 * p.class(g) as u32);
    let (d0, d1, d2, d3) = (t.word_at(g.b, at), t.word_at(g.b, at + 2), t.word_at(g.b, at + 4), t.word_at(g.b, at + 6));
    // D4.w: the offset of the row (leftover of the original)
    let row_offset = 8 * p.class(g) as u16;
    crate::world::mem::set_low_word(&mut g.leftovers.d4, row_offset);
    dust(g, p, d0, d1, d2, d3);
}

// ---- action 0x32 (0x36f48) ---------------------------------------------------------------------

/// 0x36f48: animation 55 for 24 frames, then end of the slow walk.
pub fn spell_fizzle(g: &mut Ctx, p: Player) {
    if p.u8(g, Player::STEP) == 0 {
        next_step(g, p);
        p.set_u8(g, Player::TIMER_6F, 24);
        animation::start(g, p, 55);
        return;
    }
    let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    if left == 0 {
        p.set_u8(g, Player::SLOW, 0);
        to_stand(g, p);
    } else {
        animation::step(g, p);
    }
}
