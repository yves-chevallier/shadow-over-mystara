//! Reactions with their own automaton: crash landing (0x34), hurled out (0x3e), shock (0x22), dizzy (0x24),
//! fallen (0x38), sitting (0x28), frozen (stepped reaction 0x3b30e), held (0x32).

use crate::world::objects::Obj;
use crate::actors::player::Player;
use super::*;

// ---- reaction 0x34 (0x3b4c4): violent fall ------------------------------------------------------

/// 0x3b504: on landing, the screen shakes (0x2824, D0 = 10) and the player loses `+0x6f` HP (outside
/// cinematics); at zero they die.
pub(super) fn crash_landing(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    stop_at_wall(g, p);
    if altitude(g, p) > 0 {
        return;
    }
    crate::services::camera_modes::start_shake(g.b, crate::services::camera_modes::shake::A, 10);
    p.set_u32(g, Obj::Z, 0);
    if ram::FROZEN.get(g.b) == 0 {
        let hp = p.u16(g, Player::HP).wrapping_sub(p.u8(g, Player::TIMER_6F) as u16);
        p.set_u16(g, Player::HP, hp);
        if (hp as i16) <= 0 {
            p.set_u8(g, Player::ACTIVE, 2);
            p.set_u16(g, Player::HP, 0);
            p.set_u16(g, Player::MAIN, 0x0400);
            g.sound(rom::DEATH_EFFECT, p);
            let alive = ram::PLAYERS_ALIVE.get(g.b).wrapping_sub(1);
            ram::PLAYERS_ALIVE.set(g.b, alive);
        }
    }
    next_step(g, p);
    hurt_animation(g, p, 9);
    g.sound(crate::hw::sound::LANDING_EFFECT, p);
}

// ---- reaction 0x3e (0x3a62a): thrown far away -------------------------------------------------------

/// The thrown player has left the screen area (more than 320 pixels after the left edge + 32) or
/// hit an obstacle.
pub(super) fn hurled_out(g: &mut Ctx, p: Player) -> bool {
    cov!();
    if p.u8(g, Player::BLOCKED) != 0 {
        return true;
    }
    let camera = crate::world::objects::cull::camera(g.b);
    let gap = p.u16(g, Obj::X).wrapping_sub(crate::world::mem::rd16(g.b, camera + 8)).wrapping_sub(32);
    gap > 320
}

/// 0x3a656, 0x3ab7c: throw along the ground (vx 0x800, vertical speed `vz`, 18 frames), animation `anim`.
pub(super) fn hurl(g: &mut Ctx, p: Player, vz: u16, anim: u16) {
    cov!();
    next_step(g, p);
    p.set_u16(g, Obj::VX, 0x800);
    p.set_u16(g, Obj::AX, 0);
    p.set_u16(g, Obj::VZ, vz);
    p.set_u16(g, Obj::AZ, (-4i16) as u16);
    let freeze = p.u8(g, Player::HIT_FREEZE);
    p.set_u8(g, Player::HITSTOP, freeze);
    face_attacker(g, p);
    p.set_u8(g, Player::TIMER_6F, 18);
    hurt_animation(g, p, anim);
    knock_down_if_dead(g, p);
}

/// 0x3a68a, 0x3abb0: low flight (18 frames), then braking (animation `brake`); an obstacle or the edge of
/// the screen stops the flight dead (fall at step `out`, animation 25).
pub(super) fn hurled(g: &mut Ctx, p: Player, brake: u16, out: u8) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    if left == 0 {
        next_step(g, p);
        p.set_u16(g, Obj::AZ, (-64i16) as u16);
        let ax: u16 = if p.faces_left(g) { 32 } else { (-32i16) as u16 };
        p.set_u16(g, Obj::AX, ax);
        return hurt_animation(g, p, brake);
    }
    if !hurled_out(g, p) {
        return;
    }
    p.set_u32(g, Obj::VX, 0);
    p.set_u16(g, Obj::VZ, 0);
    p.set_u16(g, Obj::AZ, (-64i16) as u16);
    p.set_u8(g, Player::STEP, out);
    hurt_animation(g, p, 25);
}

/// 0x3a706, 0x3ac2c: falls back; on the ground, animation `anim` (and dust if `effect`); an obstacle or the edge
/// stops the slide.
pub(super) fn hurled_landing(g: &mut Ctx, p: Player, anim: u16, effect: bool) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    if altitude(g, p) <= 0 {
        p.set_u32(g, Obj::Z, 0);
        next_step(g, p);
        hurt_animation(g, p, anim);
        if effect {
            g.sound(crate::hw::sound::LANDING_EFFECT, p);
        }
        return;
    }
    if hurled_out(g, p) {
        p.set_u32(g, Obj::VX, 0);
    }
}

/// 0x3a75a, 0x3acba: slide on the ground (dust every other frame); at rest, step `stop`.
pub(super) fn hurled_slide(g: &mut Ctx, p: Player, stop: u8) {
    cov!();
    move_x(g, p);
    if p.u16(g, Obj::VX) != 0 && ram::FRAME_COUNTER.get(g.b) & 1 != 0 {
        shock_dust(g, p);
    }
    stop_at_wall(g, p);
    if p.u8(g, Obj::VX) == 0 {
        p.set_u8(g, Player::STEP, stop);
    }
}

// ---- reaction 0x22 (0x3ad96): grabbed (h) — thrown, then struggles --------------------------------

/// Change of the held directions since the previous frame (the player "struggles").
pub(super) fn struggling(g: &mut Ctx, p: Player) -> bool {
    p.u8(g, Player::KEYS_HELD) & 15 != p.u8(g, Player::KEYS_PREV) & 15
}

/// Duration of the hold, counted down by the jolts.
pub(super) const HOLD: u32 = 0x1f0;

/// 0x3adbe: small throw, palette 1/19, 30 frames; if no HP left, on the ground.
pub(super) fn shock_start(g: &mut Ctx, p: Player) {
    cov!();
    if is_dead(g, p) {
        knock_down(g, p);
        return downed(g, p);
    }
    next_step(g, p);
    p.set_u8(g, HOLD, 8);
    p.set_u16(g, Obj::VX, 0x200);
    p.set_u16(g, Obj::AX, 0);
    p.set_u16(g, Obj::VZ, 0x400);
    p.set_u16(g, Obj::AZ, (-128i16) as u16);
    face_attacker(g, p);
    p.set_u8(g, Player::PALETTE_GROUP, 1);
    p.set_u8(g, Player::PALETTE, 19);
    p.set_u8(g, Player::TIMER_6F, 30);
    p.set_u8(g, Player::HITSTOP, 1);
    hurt_animation(g, p, 27);
    g.sound(crate::hw::sound::CMD_31E_41E_B, p);
}

/// 0x3ae0e: falls back (without animation).
pub(super) fn shock_fall(g: &mut Ctx, p: Player) {
    cov!();
    move_xz(g, p);
    stop_at_wall(g, p);
    if altitude(g, p) > 0 {
        return;
    }
    p.set_u32(g, Obj::Z, 0);
    next_step(g, p);
}

/// 0x3ae32: each jolt advances the animation and wears down the hold; hold worn out: released. After
/// `+0x6f` frames, what follows (`shock_release`).
pub(super) fn shock_struggle(g: &mut Ctx, p: Player) {
    cov!();
    if struggling(g, p) {
        animation::step(g, p);
        let left = p.u8(g, HOLD).wrapping_sub(1);
        p.set_u8(g, HOLD, left);
        if left == 0 {
            crate::actors::families::effects_creators::for_players::debris(&mut g.leftovers, g.b, p.obj(), 1);
            p.set_u16(g, Player::PALETTE_GROUP, 0);
            return recover(g, p);
        }
    }
    let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    if left != 0 {
        return;
    }
    next_step(g, p);
    crate::actors::families::effects_creators::for_players::debris(&mut g.leftovers, g.b, p.obj(), 1);
}

/// 0x3ae7a: palette restored, throw.
pub(super) fn shock_release(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u16(g, Player::PALETTE_GROUP, 0);
    launch_a(g, p);
    p.set_u8(g, Player::HITSTOP, 1);
}

// ---- reaction 0x24 (0x3ae8a): stunned -----------------------------------------------------------

/// 0x3aeaa: stars, 40 frames of stun.
pub(super) fn dizzy_start(g: &mut Ctx, p: Player) {
    cov!();
    crate::actors::families::effects_creators::for_players::blow(&mut g.leftovers, g.b, p.obj(), 0, 0);
    next_step(g, p);
    p.set_u8(g, Player::TIMER_6F, 40);
    p.set_u8(g, Player::HITSTOP, 1);
    hurt_animation(g, p, 0);
}

/// 0x3aeca: the player staggers one pixel left and right (sometimes new stars), then falls
/// backward (animation 22 or 23 every other frame).
pub(super) fn dizzy(g: &mut Ctx, p: Player) {
    cov!();
    let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    if left != 0 {
        if g.random() & 127 == 0 {
            crate::actors::families::effects_creators::for_players::blow(&mut g.leftovers, g.b, p.obj(), 0, 1);
        }
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
    p.set_u16(g, Obj::VX, 0x180);
    p.set_u16(g, Obj::AX, 0);
    p.set_u32(g, Obj::VY, 0);
    p.set_u16(g, Obj::VZ, 0x400);
    p.set_u16(g, Obj::AZ, (-64i16) as u16);
    if p.faces_left(g) {
        // 0x3b952: `neg.l` of the (speed, acceleration) pair
        let pair = p.u32(g, Obj::VX).wrapping_neg();
        p.set_u32(g, Obj::VX, pair);
    }
    let anim = if ram::FRAME_COUNTER.get(g.b) & 1 != 0 { 23 } else { 22 };
    hurt_animation(g, p, anim);
    p.set_u8(g, Player::HITSTOP, 0);
    knock_down_if_dead(g, p);
}

// ---- reaction 0x38 (0x3b66e): knocked over, stars, then gets up ---------------------------------------

/// 0x3b684: with no HP, on the ground right away (0x3b8f6). Otherwise 240 frames of stars (`+0x6f`); airborne, the
/// player falls back (animation 3), on the ground they lie down at once.
pub(super) fn fallen_start(g: &mut Ctx, p: Player) {
    cov!();
    if is_dead(g, p) {
        knock_down(g, p);
        return downed(g, p);
    }
    next_step(g, p);
    p.set_u8(g, Player::TIMER_6F, 0xf0);
    if p.u16(g, Obj::Z) == 0 {
        return fallen_lie_down(g, p);
    }
    p.set_u32(g, Obj::VX, 0);
    p.set_u16(g, Obj::VZ, 0);
    p.set_u16(g, Obj::AZ, (-0x40i16) as u16);
    hurt_animation(g, p, 3);
}

/// 0x3b6b2: lying down (animation 0x23); slowed (`+0x66`), they go straight to the stars (0x3b6c4).
pub(super) fn fallen_lie_down(g: &mut Ctx, p: Player) {
    cov!();
    next_step(g, p);
    if p.u8(g, Player::SLOW) != 0 {
        return fallen_stars_start(g, p);
    }
    hurt_animation(g, p, 0x23);
}

/// 0x3b6c4: stars above the lying player (animation 0x1c).
pub(super) fn fallen_stars_start(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u8(g, Player::ACTIVE, 1);
    p.set_u8(g, Player::INVULN, 0);
    next_step(g, p);
    hurt_animation(g, p, 0x1c);
}

/// 0x3b6da: the fall down to the ground.
pub(super) fn fallen_drop(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    if p.u16(g, Obj::Z) as i16 > 0 {
        return;
    }
    p.set_u16(g, Obj::Z, 0);
    fallen_lie_down(g, p);
}

/// 0x3b6f4: waits for the end of the lying player's animation.
pub(super) fn fallen_lie(g: &mut Ctx, p: Player) {
    cov!();
    if animation::step(g, p) {
        fallen_stars_start(g, p);
    }
}

/// 0x3b700: new stars every 16 frames (0xb378a, D0 = 3; D1, the setting, is the offset left
/// by the dispatch); at the end of the count, the player gets up (animation 0x21).
pub(super) fn fallen_stars(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    let left = p.u8(g, Player::TIMER_6F);
    if left & 0xf == 0 {
        // Original glitch: the setting is D1.b, the offset of the step left by the dispatch
        let setting = g.jumped.offset as u8;
        crate::actors::families::effects_creators::for_players::blow(&mut g.leftovers, g.b, p.obj(), 3, setting);
    }
    let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    if left != 0 {
        return;
    }
    next_step(g, p);
    hurt_animation(g, p, 0x21);
}

// ---- reaction 0x28 (0x3b1e4): sitting on the ground, struggles to get up ------------------------------

/// 0x3b1fa.
pub(super) fn sit_start(g: &mut Ctx, p: Player) {
    cov!();
    if is_dead(g, p) {
        p.set_u16(g, Player::MAIN, 0x0400);
        p.set_u8(g, Player::ACTIVE, 2);
        p.set_u8(g, Player::HITSTOP, 1);
        return jump(g, p, steps::DOWN_6);
    }
    p.set_u8(g, Player::TIMER_6F, 120);
    next_step(g, p);
    if p.u16(g, Obj::Z) != 0 {
        p.set_u32(g, Obj::VX, 0);
        p.set_u16(g, Obj::VZ, 0);
        p.set_u16(g, Obj::AZ, (-64i16) as u16);
        return hurt_animation(g, p, 3);
    }
    sit(g, p);
}

/// 0x3b23c: sitting (animation 35), or in slow walk directly up (0x3b24e).
pub(super) fn sit(g: &mut Ctx, p: Player) {
    cov!();
    next_step(g, p);
    if p.u8(g, Player::SLOW) == 0 {
        return hurt_animation(g, p, 35);
    }
    sit_down(g, p);
}

/// 0x3b24e: the struggle begins (animation 34), no more invulnerability.
pub(super) fn sit_down(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u8(g, Player::ACTIVE, 1);
    p.set_u8(g, Player::INVULN, 0);
    next_step(g, p);
    hurt_animation(g, p, 34);
}

/// 0x3b264: falls back, then sits down.
pub(super) fn sit_fall(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    move_xz(g, p);
    if altitude(g, p) > 0 {
        return;
    }
    p.set_u16(g, Obj::Z, 0);
    sit(g, p);
}

/// 0x3b28a: each jolt uses two frames of `+0x6f` (one otherwise); at the end, they get up (animation 32,
/// 48 frames of invulnerability).
pub(super) fn sit_struggle(g: &mut Ctx, p: Player) {
    cov!();
    animation::step(g, p);
    if !struggle_out(g, p) {
        return;
    }
    next_step(g, p);
    p.set_u8(g, Player::INVULN, 48);
    hurt_animation(g, p, 32);
}

/// `+0x6f` drops by one frame, by two if the player struggles; true when exhausted.
pub(super) fn struggle_out(g: &mut Ctx, p: Player) -> bool {
    cov!();
    let mut left = p.u8(g, Player::TIMER_6F);
    if struggling(g, p) {
        left = left.wrapping_sub(1);
    }
    left = left.wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    (left as i8) <= 0
}

// ---- stepped reaction 0x3b30e (0x3b326..0x3b44a) -------------------------------------------------------

/// Sounds of the reaction: 0xe21a0 (death), 0xe7b3c (hit); 0xb6ba8 (`effects_creators::for_players::type_57`)
/// creates the step-end effect.
pub(super) const FROZEN_DEATH_SOUND: crate::hw::sound::Sound = crate::hw::sound::CMD_A4_BY_CHARACTER;
pub(super) const FROZEN_SOUND: crate::hw::sound::Sound = crate::hw::sound::CMD_13E_23E;
/// Comparison bytes of the wait steps (`+0x6c`, `+0x6d`) and animation counter (`+0x1f0`).
pub(super) const WAIT_LEFT: u32 = 0x6c;
pub(super) const WAIT_RIGHT: u32 = 0x6d;
pub(super) const WAIT_ANIMATIONS: u32 = 0x1f0;

/// Zero speeds (X and Z) and vertical acceleration of -0x80: common end of the entry.
pub(super) fn frozen_drift(g: &mut Ctx, p: Player) {
    p.set_u16(g, Obj::VZ, 0);
    p.set_u16(g, Obj::AZ, (-0x80i16) as u16);
    p.set_u32(g, Obj::VX, 0);
}

/// 0x3b326: step 0. With hit points, normal entry (animation 19, 60 frames); otherwise death on the spot
/// (animation 21) with a sound.
pub(super) fn frozen_start(g: &mut Ctx, p: Player) {
    cov!();
    if (p.u16(g, Player::HP) as i16) > 0 {
        next_step(g, p);
        p.set_u8(g, Player::TIMER_6F, 60);
        let index = crate::actors::families::small::player_table_byte(g.b, p.obj());
        hurt_animation(g, p, 19);
        g.sound(FROZEN_SOUND, p);
        if p.u16(g, Obj::Z) == 0 {
            next_step(g, p);
            // Original glitch: variant and setting are the D0.b and D1.b left by the code before: the index of the
            // table 0x3baf8 and the offset of the step left by the dispatch
            let (variant, setting) = (index as u8, g.jumped.offset as u8);
            crate::actors::families::effects_creators::for_players::type_57(&mut g.leftovers, g.b, p.obj(), variant, setting);
        } else {
            frozen_drift(g, p);
        }
        return;
    }
    p.set_state(g, 0x0400_2c0a);
    p.set_u8(g, Player::HITSTOP, 1);
    g.sound(FROZEN_DEATH_SOUND, p);
    p.set_u8(g, Player::TIMER_6F, 60);
    hurt_animation(g, p, 21);
    if (p.u16(g, Obj::Z) as i16) <= 0 {
        return;
    }
    p.set_state(g, 0x0400_2c02);
    frozen_drift(g, p);
}

/// 0x3b3aa: step 2, fall; on the ground, step 10 if the player is on the ground (main state 4), otherwise next step
/// and effect.
pub(super) fn frozen_fall(g: &mut Ctx, p: Player) {
    cov!();
    move_xz(g, p);
    if (p.u16(g, Obj::Z) as i16) > 0 {
        return;
    }
    p.set_u32(g, Obj::Z, 0);
    if p.u8(g, Player::MAIN) == 4 {
        p.set_u8(g, Player::STEP, 10);
        return;
    }
    next_step(g, p);
    // Original glitch: the creator takes the variant from D0.b, the one left by the movement 0x1da6 (`swap` then
    // `asr.l #8` of the vertical speed), whose low byte is zero, and the setting from D1.b, the offset of the step
    // left by the dispatch
    let variant = ((p.u16(g, Obj::VZ) as i16 as i32) << 8) as u8;
    let setting = g.jumped.offset as u8;
    crate::actors::families::effects_creators::for_players::type_57(&mut g.leftovers, g.b, p.obj(), variant, setting);
}

/// 0x3b3d8 / 0x3b3de: step 4 / 6, wait. As long as the two comparison bytes differ, the animation
/// advances and counts down `+0x1f0` (at zero: end of the reaction); then 60 frames of pause before the animation
/// `next` of the following step.
pub(super) fn frozen_wait(g: &mut Ctx, p: Player, next: u16) {
    cov!();
    if p.u8(g, WAIT_LEFT) & 15 != p.u8(g, WAIT_RIGHT) & 15 {
        animation::step(g, p);
        let left = p.u8(g, WAIT_ANIMATIONS).wrapping_sub(1);
        p.set_u8(g, WAIT_ANIMATIONS, left);
        if left == 0 {
            return recover(g, p);
        }
    }
    let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
    p.set_u8(g, Player::TIMER_6F, left);
    if left == 0 {
        next_step(g, p);
        p.set_u8(g, Player::TIMER_6F, 60);
        hurt_animation(g, p, next);
    }
}

/// 0x3b420: step 8. Game stopped (`A5+0x42`): end of the reaction; otherwise a counter (`A5+0x35`) drops and
/// the player goes to main state 4.
pub(super) fn frozen_end(g: &mut Ctx, p: Player) {
    cov!();
    if ram::FROZEN.get(g.b) != 0 {
        p.set_u8(g, Player::INVULN, 0x20);
        return recover(g, p);
    }
    let left = ram::PLAYERS_ALIVE.get(g.b).wrapping_sub(1);
    ram::PLAYERS_ALIVE.set(g.b, left);
    p.set_u16(g, Player::MAIN, 0x0400);
    p.set_u8(g, Player::TIMER_6F, 60);
    next_step(g, p);
    p.set_u16(g, Player::HP, 0);
}

// ---- reaction 0x32 (0x3b44c): held by an enemy ----------------------------------------------------

/// Enemy holding the player (16-bit pointer) and "held" flag.
pub(super) const HOLDER: u32 = Player::HOLDER_ENEMY;
pub(super) const HELD: u32 = Player::HELD;
/// State of the enemy holding its grip.
pub(super) const HOLDING: u16 = 0x020c;

/// 0x3b44c: the held player struggles: 20 new directions free them; they are also freed if
/// the enemy lets go (its state is no longer 0x020c).
pub(super) fn held(g: &mut Ctx, p: Player) {
    cov!();
    if p.u8(g, Player::STEP) == 0 {
        next_step(g, p);
        p.set_u8(g, Player::TIMER_6F, 20);
        return hurt_animation(g, p, 31);
    }
    animation::step(g, p);
    let pressed = p.u8(g, Player::KEYS_HELD) & 15 & !(p.u8(g, Player::KEYS_PREV) & 15);
    if pressed != 0 {
        let left = p.u8(g, Player::TIMER_6F).wrapping_sub(1);
        p.set_u8(g, Player::TIMER_6F, left);
        if left == 0 {
            return break_free(g, p);
        }
    }
    let holder = p.u16(g, HOLDER) as i16 as i32 as u32;
    if crate::world::mem::rd16(g.b, holder + 4) != HOLDING {
        break_free(g, p);
    }
}

/// Freed: reaction 0x34 (released).
pub(super) fn break_free(g: &mut Ctx, p: Player) {
    cov!();
    p.set_u8(g, HELD, 0);
    p.set_u8(g, Player::ACTIVE, 1);
    p.set_state(g, 0x0204_3400);
    p.set_u8(g, Player::TIMER_6F, 0);
}
