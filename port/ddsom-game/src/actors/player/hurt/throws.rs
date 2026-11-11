//! Simple reactions, airborne throws, falls, slides, bounces and getting up (steps shared by the
//! reactions of the hit state and of the downed state).

use crate::actors::player::Player;
use crate::world::objects::Obj;
use super::*;

// ---- simple reactions ------------------------------------------------------------------------

/// 0x3a17e: recoil slide. As long as `+0x6f` lasts, the player trembles by 2 pixels; then
/// the animation plays and the horizontal speed makes them slide.
pub(super) fn flinch(g: &mut Ctx, p: Player) {
    cov!();
    let left = p.u8(g, Player::TIMER_6F);
    if left != 0 {
        let mut shift = 2i16;
        if p.faces_left(g) {
            shift = -shift;
        }
        p.set_u8(g, Player::TIMER_6F, left - 1);
        if (left - 1) & 1 == 0 {
            shift = -shift;
        }
        let x = p.u16(g, Obj::X).wrapping_add(shift as u16);
        p.set_u16(g, Obj::X, x);
        return;
    }
    if animation::step(g, p) {
        return recover(g, p);
    }
    if p.u8(g, Obj::VX) != 0 {
        move_x(g, p);
    }
}

/// Common start of reactions 0 and 10: recoil with animation `anim`.
pub(super) fn start_flinch(g: &mut Ctx, p: Player, anim: u16) {
    cov!();
    next_step(g, p);
    p.set_u16(g, Obj::VX, 0x280);
    p.set_u16(g, Obj::AX, (-0x50i16) as u16);
    let freeze = p.u8(g, Player::HIT_FREEZE);
    p.set_u8(g, Player::HITSTOP, freeze);
    p.set_u8(g, Player::TIMER_6F, 8);
    face_attacker(g, p);
    hurt_animation(g, p, anim);
}

/// 0x3a136: simple recoil.
pub(super) fn reaction_0(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::STEP) != 0 {
        return flinch(g, p);
    }
    if p.u8(g, Player::SLOW) != 0 {
        p.set_u8(g, Player::ACTION, 0x14);
        return hurt_slow(g, p);
    }
    if is_dead(g, p) {
        // 0x3b8f6: on the ground right away, and the "on the ground" handler runs immediately after
        knock_down(g, p);
        return downed(g, p);
    }
    start_flinch(g, p, 0);
}

/// 0x3a79e: recoil with animation 1.
pub(super) fn reaction_10(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::STEP) != 0 {
        return flinch(g, p);
    }
    if p.u8(g, Player::SLOW) != 0 {
        p.set_u8(g, Player::ACTION, 0x14);
        return hurt_slow(g, p);
    }
    if is_dead(g, p) {
        p.set_u16(g, Player::MAIN, 0x0400);
        p.set_u8(g, Player::ACTIVE, 2);
        return jump(g, p, steps::DOWN_A);
    }
    start_flinch(g, p, 1);
}

// ---- airborne throws (shared steps) --------------------------------------------------------

/// Step 0 of the throws: animation, then speed (vx 0x200, vz 0x480).
pub(super) fn launch_with_anim(g: &mut Ctx, p: Player, anim: u16) {
    cov!();
    hurt_animation(g, p, anim);
    next_step(g, p);
    set_launch(g, p, 0x200, 0x480);
    face_attacker(g, p);
    knock_down_if_dead(g, p);
}

/// 0x3a2ae: throw (vx 0x180, vz 0x400), animation 0xa.
pub(super) fn launch_a(g: &mut Ctx, p: Player) {
    cov!();
    hurt_animation(g, p, 0xa);
    launch_common(g, p);
}

// ---- steps reached by no scenario (reactions 0x3a578, 0x3a890, 0x3a9da, 0x3a9f8, 0x3af44, 0x3b570, 0x3b72c, 0x3b7ba,
// on the ground 0x3b8a6 and 0x3b8c4): transcribed from the original, validated by `fuzz_couv` (players_update) ----

/// 0x3a59a, 0x3a5a4: high throw (vx 0x280, vz 0x500) with animation `anim`; with no HP, to the ground.
pub(super) fn launch_high(g: &mut Ctx, p: Player, anim: u16) {
    cov!();
    hurt_animation(g, p, anim);
    next_step(g, p);
    set_launch(g, p, 0x280, 0x500);
    face_attacker(g, p);
    knock_down_if_dead(g, p);
}

/// 0x3aa2e: the slow walk ends, throw (vx 0x200, vz 0x480), animation 0xe.
pub(super) fn launch_slow(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u8(g, Player::SLOW, 0);
    hurt_animation(g, p, 0xe);
    next_step(g, p);
    set_launch(g, p, 0x200, 0x480);
    face_attacker(g, p);
}

/// 0x3aff8: slide on the ground (dust every other frame), stopped by an obstacle; at rest, next step and 10 frames.
pub(super) fn slide_to_rest(g: &mut Ctx, p: Player) {
    cov!();
    move_x(g, p);
    if p.u16(g, Obj::VX) != 0 && ram::FRAME_COUNTER.get(g.b) & 1 != 0 {
        shock_dust(g, p);
    }
    stop_at_wall(g, p);
    if p.u8(g, Obj::VX) == 0 {
        next_step(g, p);
        p.set_u8(g, Player::TIMER_6F, 0xa);
    }
}

/// 0x3b592: stars (0xb378a, variant 0, setting 0), 20 frames of shaking (animation 0).
pub(super) fn stars_start(g: &mut Ctx, p: Player) {
    cov!();
    crate::actors::families::effects_creators::for_players::blow(&mut g.leftovers, g.b, p.obj(), 0, 0);
    next_step(g, p);
    p.set_u8(g, Player::TIMER_6F, 0x14);
    p.set_u8(g, Player::HITSTOP, 1);
    hurt_animation(g, p, 0);
}

/// 0x3b5b2: the player shakes one pixel left and right; at the end of the count they lose 16 HP and are thrown
/// (dead: to the ground), or, without altitude and still alive, slide backward (vx 0x280, deceleration 0x50).
pub(super) fn stars_shake(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::TIMER_6F) == 0 {
        return stars_end(g, p);
    }
    let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    if (left as i8) > 0 {
        let mut dx: u16 = 1;
        if left & 1 != 0 {
            dx = dx.wrapping_neg();
        }
        if p.faces_left(g) {
            dx = dx.wrapping_neg();
        }
        let x = p.u16(g, Obj::X).wrapping_add(dx);
        p.set_u16(g, Obj::X, x);
        return;
    }
    next_step(g, p);
    let hp = p.u16(g, Player::HP).wrapping_sub(0x10);
    p.set_u16(g, Player::HP, hp);
    if (hp as i16) <= 0 {
        p.set_u16(g, Player::HP, 0);
        g.sound(rom::DEATH_EFFECT, p);
        let alive = ram::PLAYERS_ALIVE.get(g.b).wrapping_sub(1);
        ram::PLAYERS_ALIVE.set(g.b, alive);
        p.set_u8(g, Player::ACTIVE, 2);
        p.set_u16(g, Player::MAIN, 0x0400);
        return stars_thrown(g, p);
    }
    if (p.u16(g, Obj::Z) as i16) > 0 {
        return stars_thrown(g, p);
    }
    p.set_u16(g, Obj::VX, 0x280);
    p.set_u16(g, Obj::AX, (-0x50i16) as u16);
    reverse_if_facing_left(g, p);
}

/// 0x3b5e8: throw (vx 0x200, vz 0x480), animation 0x16 or 0x17 every other frame.
pub(super) fn stars_thrown(g: &mut Ctx, p: Player) {
    cov!();
    next_step(g, p);
    p.set_u8(g, Player::HITSTOP, 1);
    p.set_u16(g, Obj::VX, 0x200);
    p.set_u16(g, Obj::AX, 0);
    p.set_u16(g, Obj::VZ, 0x480);
    p.set_u16(g, Obj::AZ, (-0x40i16) as u16);
    p.set_u32(g, Obj::VY, 0);
    let anim = if ram::FRAME_COUNTER.get(g.b) & 1 != 0 { 0x17 } else { 0x16 };
    hurt_animation(g, p, anim);
    reverse_if_facing_left(g, p);
}

/// 0x3b656: end of the animation: back to the standing state; otherwise the slide goes on while there is speed.
pub(super) fn stars_end(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        return recover(g, p);
    }
    if p.u8(g, Obj::VX) != 0 {
        move_x(g, p);
    }
}

/// 0x3b952: the horizontal speed (and acceleration) reversed for a player facing left.
pub(super) fn reverse_if_facing_left(g: &mut Ctx, p: Player) {
    cov!();
    if p.faces_left(g) {
        let v = p.u32(g, Obj::VX).wrapping_neg();
        p.set_u32(g, Obj::VX, v);
    }
}

/// 0x3b740: 240 frames; airborne, the player falls straight down (animation 3), on the ground they kneel at once.
pub(super) fn kneel_start(g: &mut Ctx, p: Player) {
    cov!();
    next_step(g, p);
    p.set_u8(g, Player::TIMER_6F, 0xf0);
    if p.u16(g, Obj::Z) == 0 {
        return kneel_stars(g, p);
    }
    p.set_u32(g, Obj::VX, 0);
    p.set_u16(g, Obj::VZ, 0);
    p.set_u16(g, Obj::AZ, (-0x40i16) as u16);
    hurt_animation(g, p, 3);
}

/// 0x3b766: kneeling (animation 0x11) under stars: up to six (0xb378a, variant 2, settings 5 down to 0); a full pool
/// skips a step.
pub(super) fn kneel_stars(g: &mut Ctx, p: Player) {
    cov!();
    next_step(g, p);
    hurt_animation(g, p, 0x11);
    for setting in (0..=5).rev() {
        if crate::actors::families::effects_creators::for_players::blow(&mut g.leftovers, g.b, p.obj(), 2, setting).is_none() {
            next_step(g, p);
            return;
        }
    }
}

/// 0x3b78a: falling straight down; on the ground, kneeling (`kneel_stars`).
pub(super) fn kneel_fall(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    if (p.u16(g, Obj::Z) as i16) > 0 {
        return;
    }
    p.set_u16(g, Obj::Z, 0);
    kneel_stars(g, p);
}

/// 0x3a2b4: throw (vx 0x180, vz 0x400).
pub(super) fn launch_common(g: &mut Ctx, p: Player) {
    cov!();
    next_step(g, p);
    set_launch(g, p, 0x180, 0x400);
    face_attacker(g, p);
    knock_down_if_dead(g, p);
}

/// 0x3aaac: throw (vx 0x200, vz 0x200), animation 0x16 or 0x17 every other frame.
pub(super) fn launch_b(g: &mut Ctx, p: Player) {
    cov!();
    next_step(g, p);
    set_launch(g, p, 0x200, 0x200);
    face_attacker(g, p);
    let anim = if ram::FRAME_COUNTER.get(g.b) & 1 != 0 { 0x17 } else { 0x16 };
    hurt_animation(g, p, anim);
    knock_down_if_dead(g, p);
}

/// 0x3ad4c: throw like `launch_common`, then depth speed (`VY` 0x80), animation 0x12.
pub(super) fn launch_c(g: &mut Ctx, p: Player) {
    cov!();
    launch_common(g, p);
    p.set_u16(g, Obj::VY, 0x80);
    p.set_u16(g, Obj::AY, 0);
    p.set_u8(g, Player::HITSTOP, 1);
    hurt_animation(g, p, 0x12);
}

/// Contact with an obstacle (`+0x29`): the horizontal speed is cancelled.
pub(super) fn stop_at_wall(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::BLOCKED) != 0 {
        p.set_u32(g, Obj::VX, 0);
    }
}

/// Altitude (high word of `Z`, signed).
pub(super) fn altitude(g: &mut Ctx, p: Player) -> i16 {
    cov!();
    p.u16(g, Obj::Z) as i16
}

/// 0x3a2e6: waits for the end of the animation, then chains animation `anim`.
pub(super) fn next_animation(g: &mut Ctx, p: Player, anim: u16) {
    cov!();
    if animation::step(g, p) {
        next_step(g, p);
        hurt_animation(g, p, anim);
    }
}

/// 0x3a300: airborne, up to the peak (vertical speed turned negative).
pub(super) fn fall_to_apex(g: &mut Ctx, p: Player, anim: u16) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    stop_at_wall(g, p);
    if (p.u16(g, Obj::VZ) as i16) >= 0 {
        return;
    }
    next_step(g, p);
    hurt_animation(g, p, anim);
}

/// 0x3a330: falls back to the ground.
pub(super) fn land(g: &mut Ctx, p: Player, anim: u16) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    stop_at_wall(g, p);
    if altitude(g, p) >= 0 {
        return;
    }
    p.set_u32(g, Obj::Z, 0);
    next_step(g, p);
    hurt_animation(g, p, anim);
    g.sound(crate::hw::sound::LANDING_EFFECT, p);
}

/// 0x3a522: end of the fall, small bounce (vz 0x240).
pub(super) fn bounce_up(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        next_step(g, p);
        p.set_u16(g, Obj::VZ, 0x240);
        p.set_u16(g, Obj::AZ, (-0x40i16) as u16);
        hurt_animation(g, p, 8);
    }
}

/// 0x3a362: higher bounce (vz 0x1c0).
pub(super) fn bounce_up_small(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        next_step(g, p);
        p.set_u16(g, Obj::VZ, 0x1c0);
        p.set_u16(g, Obj::AZ, (-0x40i16) as u16);
        hurt_animation(g, p, 8);
    }
}

/// 0x3a384: falls back from the bounce.
pub(super) fn land_with_effect(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    if altitude(g, p) >= 0 {
        return;
    }
    p.set_u32(g, Obj::Z, 0);
    next_step(g, p);
    hurt_animation(g, p, 9);
    g.sound(crate::hw::sound::LANDING_EFFECT, p);
}

/// 0x3a544: start of the slide on the ground (vx 0x300 in the recoil direction).
pub(super) fn slide_start(g: &mut Ctx, p: Player) {
    cov!();
    start_slide(g, p, 0x300);
}

/// 0x3a3ae: same with vx 0x280.
pub(super) fn slide_start_short(g: &mut Ctx, p: Player) {
    cov!();
    start_slide(g, p, 0x280);
}

pub(super) fn start_slide(g: &mut Ctx, p: Player, vx: u16) {
    cov!();
    if !animation::step(g, p) {
        return;
    }
    next_step(g, p);
    p.set_u8(g, Player::TIMER_6F, 0x14);
    p.set_u16(g, Obj::VX, vx);
    p.set_u16(g, Obj::AX, (-0x40i16) as u16);
    if p.faces_left(g) {
        // `neg.l` on the (speed, acceleration) pair: both words are negated together
        let both = p.u32(g, Obj::VX).wrapping_neg();
        p.set_u32(g, Obj::VX, both);
    }
    hurt_animation(g, p, 0xf);
}

/// 0x3a3e2: slide; dust every other frame; stops when the speed drops to zero.
pub(super) fn slide(g: &mut Ctx, p: Player) {
    cov!();
    move_x(g, p);
    if p.u16(g, Obj::VX) != 0 && ram::FRAME_COUNTER.get(g.b) & 1 != 0 {
        shock_dust(g, p);
    }
    stop_at_wall(g, p);
    if p.u8(g, Obj::VX) == 0 {
        next_step(g, p);
    }
}

/// 0x3b960: scripted fall (cinematic 0x2447e): the player is thrown without losing life (HP kept in
/// `+0x64`, brought back to 1), rises, falls back, bounces, lands then slides; at rest, next action (`+6`).
/// The odd variants of the character (`+2`) take the animations of the second series (+4).
pub fn fall(g: &mut Ctx, p: Player) {
    cov!();
    let second = p.u8(g, Player::INDEX) & 1 != 0;
    let anim = |n: u16| if second { n + 4 } else { n };
    match p.u8(g, Player::STEP) {
        0 => {
            hurt_animation(g, p, anim(6));
            let hp = p.u16(g, Player::HP);
            p.set_u16(g, Player::HP_PREV, hp);
            p.set_u16(g, Player::HP, 1);
            next_step(g, p);
            for (field, v) in [(Obj::VX, 0x180), (Obj::AX, 0), (Obj::VZ, 0x400), (Obj::AZ, 0xffc0), (Obj::VY, 0xffc0), (Obj::AY, 0)] {
                p.set_u16(g, field, v);
            }
            p.set_u8(g, Player::HITSTOP, 0);
            if p.faces_left(g) {
                let both = p.u32(g, Obj::VX).wrapping_neg();
                p.set_u32(g, Obj::VX, both);
            }
            g.sound(rom::BEFORE_HURT_A, p);
        }
        2 => next_animation(g, p, anim(7)),
        4 => fall_to_apex_xyz(g, p, anim(8)),
        6 => fall_to_apex_xyz(g, p, anim(9)),
        8 => {
            if animation::step(g, p) {
                next_step(g, p);
                p.set_u16(g, Obj::VZ, 0x1c0);
                p.set_u16(g, Obj::AZ, 0xffc0);
                hurt_animation(g, p, 8);
            }
        }
        10 => {
            animation::step(g, p);
            move_xyz(g, p);
            if altitude(g, p) < 0 {
                p.set_u32(g, Obj::Z, 0);
                next_step(g, p);
                hurt_animation(g, p, 9);
                g.sound(crate::hw::sound::LANDING_EFFECT, p);
            }
        }
        12 => {
            if animation::step(g, p) {
                next_step(g, p);
                p.set_u16(g, Obj::VX, 0x280);
                p.set_u16(g, Obj::AX, 0xffc0);
                if p.faces_left(g) {
                    let both = p.u32(g, Obj::VX).wrapping_neg();
                    p.set_u32(g, Obj::VX, both);
                }
                hurt_animation(g, p, 15);
            }
        }
        _ => {
            move_x(g, p);
            if p.u16(g, Obj::VX) != 0 && ram::FRAME_COUNTER.get(g.b) & 1 != 0 {
                shock_dust(g, p);
            }
            stop_xy_at_wall(g, p);
            if p.u8(g, Obj::VX) == 0 {
                let action = p.u8(g, Player::ACTION).wrapping_add(2);
                p.set_u8(g, Player::ACTION, action);
                p.set_u8(g, Player::STEP, 0);
            }
        }
    }
}

/// 0x3b9ec: airborne (movement on the three axes), up to the peak.
pub(super) fn fall_to_apex_xyz(g: &mut Ctx, p: Player, anim: u16) {
    cov!();
    animation::step(g, p);
    move_xyz(g, p);
    stop_xy_at_wall(g, p);
    if (p.u16(g, Obj::VZ) as i16) < 0 {
        next_step(g, p);
        hurt_animation(g, p, anim);
    }
}

/// Against a wall (`+0x29`), no more speed in x or in depth.
pub(super) fn stop_xy_at_wall(g: &mut Ctx, p: Player) {
    if p.u8(g, Player::BLOCKED) != 0 {
        p.set_u32(g, Obj::VX, 0);
        p.set_u32(g, Obj::VY, 0);
    }
}

/// 0x3a424: waits `+0x6f` frames on the ground then gets up (animation 0x10 and sound).
pub(super) fn get_up(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::MAIN) == 4 {
        // death: the player stays on the ground for a while, then the resurrection blinking begins
        let step = p.u8(g, Player::STEP).wrapping_add(4);
        p.set_u8(g, Player::STEP, step);
        p.set_u8(g, Player::TIMER_6F, 0x3c);
        return invulnerable_blink(g, p);
    }
    let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    if left != 0 {
        return;
    }
    next_step(g, p);
    hurt_animation(g, p, 0x10);
    let sound = if ram::FRAME_COUNTER.get(g.b) & 2 != 0 { rom::GET_UP_SOUND_B } else { rom::GET_UP_SOUND_A };
    g.sound(sound, p);
}

/// 0x3a466: end of the get-up animation: return to the standing state.
pub(super) fn finish_get_up(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        recover(g, p);
    }
}

/// 0x3a472: invulnerability blinking for `+0x6f` frames (the player is hidden one
/// frame out of four in the last 40); at the end, the status effects are cleared.
pub(super) fn invulnerable_blink(g: &mut Ctx, p: Player) {
    cov!();
    let flags = p.u8(g, Player::FLAGS) & !2;
    p.set_u8(g, Player::FLAGS, flags);
    let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    if left == 0 {
        let sub = p.u8(g, Player::SUB).wrapping_add(2);
        p.set_u8(g, Player::SUB, sub);
        p.set_u16(g, Player::ACTION, 0);
        for field in [Player::AURA, Player::AURA_B, Player::AURA_C, Player::POWERUP] {
            p.set_u8(g, field, 0);
        }
        p.set_u16(g, Player::AURA_TIMER, 0);
        p.set_u8(g, Player::SLOW, 0);
        return;
    }
    if (left as i8) > 0x28 || left & 3 == 0 {
        return;
    }
    let flags = p.u8(g, Player::FLAGS) | 2;
    p.set_u8(g, Player::FLAGS, flags);
}

/// 0x3ad66: throw with slide in depth.
pub(super) fn fall_c(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_xyz(g, p);
    stop_at_wall(g, p);
    if altitude(g, p) > 0 {
        return;
    }
    p.set_u32(g, Obj::Z, 0);
    next_step(g, p);
    hurt_animation(g, p, 0xd);
}

/// 0x3aae8: throw (reaction 26): falls back.
pub(super) fn fall_b(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    stop_at_wall(g, p);
    if altitude(g, p) > 0 {
        return;
    }
    p.set_u32(g, Obj::Z, 0);
    next_step(g, p);
    hurt_animation(g, p, 9);
}
