//! Whirls and spins: the object turns around, rises, falls and bounces (codes of the spells and big hits).

use super::*;

/// Start angle of a whirlwind: random, centered on the left (8) or on the right (24) depending on the orientation.
pub(super) fn spin_angle<R: Bench>(x: &mut Cx<'_, R>) {
    let d2 = if flipped(x) { 24 } else { 8 };
    let r = random(x);
    let a = (r as u16).wrapping_add(d2 as u16 & 15) & 31;
    x.set8(f::ANGLE, a as u8);
    anim(x, 3);
}
/// 0x3d950: carried off by a whirlwind.
pub(super) fn whirl<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x67, 0);
    x.set8(f::DOWNED, 0);
    apply_pending(x);
    next(x);
    x.set32(f::VX, 0);
    x.set32(f::VZ, 0);
    x.set32(f::VX2, 0);
    x.set8(f::GRABBED, 0);
    x.set8(f::DOWNED, 0);
    let c = if x.counter() & 1 != 0 { 1 } else { 2 };
    x.set8(f::COUNTER, c);
    spin_angle(x);
}
/// Next turn of the angle (step of 2 out of 32); the orientation follows the half-circle.
pub(super) fn turn_angle<R: Bench>(x: &mut Cx<'_, R>) {
    let a = x.u8(f::ANGLE).wrapping_add(2) & 31;
    x.set8(f::ANGLE, a);
    x.set8(f::FLIP, 0);
    let d = a.wrapping_sub(8);
    if d >= 16 {
        x.set8(f::FLIP, 32);
    }
}
/// 0x3da0e: position on the circle (table 0x1c94b6: X, depth), speeds added.
pub(super) fn orbit<R: Bench>(x: &mut Cx<'_, R>) {
    const CIRCLE: u32 = 0x1c94b6;
    let i = 4 * x.u8(f::ANGLE) as u32;
    let vx = rd16(x.b, CIRCLE + i).wrapping_add(x.u16(f::VY));
    x.set16(f::VX, vx);
    x.add32(f::X, ((vx as i16 as i32) << 8) as u32);
    let vy = rd16(x.b, CIRCLE + i + 2).wrapping_add(x.u16(f::VY2));
    x.set16(f::VX2, vy);
    let d = ((vy as i16 as i32) << 8) as u32;
    x.add32(f::Y, d);
}
/// 0x3d9b2: rises while spinning up to 96, then falls back.
pub(super) fn whirl_rise<R: Bench>(x: &mut Cx<'_, R>) {
    x.add32(f::HEIGHT, 0x2_0000);
    if x.u16(f::HEIGHT) as i16 >= 96 {
        next(x);
        x.set16(f::VZ, 0x200);
        x.set16(f::GRAVITY, -128i16 as u16);
        return;
    }
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    if c == 0 {
        x.set8(f::COUNTER, 1);
        turn_angle(x);
    }
    orbit(x);
}
/// 0x3da44: falls back; on the ground, landing and sound.
pub(super) fn whirl_fall<R: Bench>(x: &mut Cx<'_, R>) {
    anim_step(x);
    move_xyz(x);
    if x.u16(f::HEIGHT) as i16 > 0 {
        if x.u8(f::BLOCKED) != 0 {
            x.set16(f::VX, 0);
            x.set16(f::VY, 0);
            x.set8(f::COUNTER, 0);
        }
        return;
    }
    next(x);
    x.set32(f::HEIGHT, 0);
    x.set32(f::VZ, 0);
    landed(x);
    hurt(x);
}

/// 0x3dc30: boss whirlwind (60 frames).
pub(super) fn spin<R: Bench>(x: &mut Cx<'_, R>) {
    next(x);
    x.set8(f::COUNTER, 60);
    apply_pending(x);
    x.set32(f::VX, 0);
    x.set32(f::VZ, 0);
    x.set32(f::VX2, 0);
    let g = if x.counter() & 1 != 0 { 1 } else { 2 };
    x.set8(f::GRABBED, g);
    spin_angle(x);
}
/// 0x3dc86: spins while rising up to 64.
pub(super) fn spin_rise<R: Bench>(x: &mut Cx<'_, R>) {
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    if c == 0 {
        return next(x);
    }
    x.add32(f::HEIGHT, 0x2_0000);
    if x.u16(f::HEIGHT) as i16 >= 64 {
        x.set32(f::HEIGHT, 0x0040_0000);
    }
    let g = x.u8(f::GRABBED).wrapping_sub(1);
    x.set8(f::GRABBED, g);
    if g == 0 {
        x.set8(f::GRABBED, 1);
        turn_angle(x);
    }
    orbit(x);
}
/// 0x3db4e: the boss becomes active again (state 4) and falls back knocked over (animation 17).
pub(super) fn spin_drop<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(f::STATE, 4);
    x.set8(f::ACTIVE, 2);
    next(x);
    x.set8(f::COUNTER, 30);
    x.set8(f::SUB_STEP, 0);
    x.set8(0x67, 0);
    x.set8(f::DOWNED, 0);
    x.set8(f::GRABBED, 0);
    x.set32(f::VX, 0);
    x.set16(f::VZ, 0x600);
    x.set16(f::GRAVITY, -128i16 as u16);
    orient_speeds(x);
    defer_link(x);
    anim(x, 17);
    lib::sound(x, crate::sound::CMD_31E_41E_B);
}
/// 0x3dada: variant: animation 17 and stars 0xabec8(8).
pub(super) fn spin_drop_stars<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x67, 0);
    next(x);
    x.set8(f::COUNTER, 30);
    x.set8(f::SUB_STEP, 0);
    x.set32(f::VX, 0);
    x.set16(f::VZ, 0x600);
    x.set16(f::GRAVITY, -128i16 as u16);
    orient_speeds(x);
    anim(x, 17);
    lib::sound(x, crate::sound::CMD_31E_41E_B);
    effect(x, 8, 0);
}
/// 0x3dbb4: animation; fall to the ground, then wait for the counter to end.
pub(super) fn spin_fall<R: Bench>(x: &mut Cx<'_, R>) {
    anim_step(x);
    if x.u8(f::SUB_STEP) == 0 {
        move_xz(x);
    }
    let c = x.u8(f::COUNTER).wrapping_sub(1);
    x.set8(f::COUNTER, c);
    if c == 0 {
        return next_clear(x);
    }
    if x.u16(f::HEIGHT) as i16 > 0 {
        return;
    }
    x.set32(f::HEIGHT, 0);
    next_sub(x);
}
/// 0x3dbec: high bounce (60 frames, animation 13).
pub(super) fn spin_bounce<R: Bench>(x: &mut Cx<'_, R>) {
    next(x);
    x.set32(f::VX, 0);
    x.set8(f::COUNTER, 60);
    x.set16(f::VZ, 0x800);
    x.set16(f::GRAVITY, -64i16 as u16);
    hit_sound(x);
    defer_link(x);
    anim(x, 13);
}
/// 0x3db18: variant: stars 0xabec8(7), then the sound.
pub(super) fn spin_bounce_stars<R: Bench>(x: &mut Cx<'_, R>) {
    next(x);
    x.set8(f::COUNTER, 60);
    x.set32(f::VX, 0);
    x.set32(f::VX2, 0);
    x.set16(f::VZ, 0x800);
    x.set16(f::GRAVITY, -64i16 as u16);
    orient_speeds(x);
    anim(x, 13);
    effect(x, 7, 0);
    hit_sound(x);
}
/// 0x3dcea: final skimming throw (stars 0xabec8(2)).
pub(super) fn spin_blast<R: Bench>(x: &mut Cx<'_, R>) {
    next(x);
    apply_pending(x);
    x.set8(f::COUNTER, 18);
    x.set16(f::VX, 0x1000);
    x.set16(f::VY, 0);
    x.set32(f::VZ, 0x0100_0000);
    x.set16(f::GRAVITY, 0);
    anim(x, 1);
    if flipped(x) {
        neg16(x, f::VX);
    }
    effect(x, 2, 0);
}
/// 0x3dd2e: boss death; outside ENEMY, `A5+0x4f` signals the end; some types keep their step.
pub(super) fn spin_death<R: Bench>(x: &mut Cx<'_, R>) {
    next(x);
    x.set8(f::ACTIVE, 2);
    x.set16(f::HP, 0);
    x.set8(f::SUB, 6);
    if x.u8(f::CLASS) != ENEMY {
        x.set_a5_u8(0x4f, 0xff);
        return x.set16(f::STEP, 0);
    }
    if [8, 18, 27].contains(&x.u8(f::KIND)) {
        x.set16(f::STEP, 0);
    }
}

/// 0x3dd8c / 0x3dd94: death in the air: sound, then low throw (animation 2 or 3).
pub(super) fn death_throw<R: Bench>(x: &mut Cx<'_, R>) {
    death_sound(x);
    throw_1(x);
}
pub(super) fn death_throw_b<R: Bench>(x: &mut Cx<'_, R>) {
    death_sound(x);
    throw_1b(x);
}
