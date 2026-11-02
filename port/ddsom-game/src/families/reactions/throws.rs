//! First steps of the reactions: recoils, throws into the air, high throws, slides on the ground, skims and pops.

use super::*;

// Throws (first step of the reactions)

/// 0x3bb9c (code 0): recoil in place; death if the HP are exhausted.
pub(super) fn recoil<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::STEP) != 0 {
        return recoil_shake(x);
    }
    x.set8(0x67, 0);
    apply_pending(x);
    if dead(x) {
        x.set8(f::SUB, 6);
        x.set8(f::ACTIVE, 2);
        return run_entry(x, 0x3dd72);
    }
    recoil_start(x, 0);
}
/// Start of the recoil (16 frames of shaking, small push), animation `n`.
pub(super) fn recoil_start<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    next(x);
    stun(x);
    x.set8(f::COUNTER, 16);
    x.set16(f::VX, 0x180);
    x.set16(f::VY, -56i16 as u16);
    face_attacker(x);
    anim(x, n);
    hit_sound(x);
}
/// 0x3bbec: shaking, then slide (0x3bc10).
pub(super) fn recoil_shake<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::COUNTER) != 0 {
        let c = x.u8(f::COUNTER).wrapping_sub(1);
        x.set8(f::COUNTER, c);
        return shake(x, 2);
    }
    slide(x);
}
/// 0x3bc10 / 0x3d7f8: slide until it stops, then get-up.
pub(super) fn slide<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::VX) == 0 {
        return slide_end(x);
    }
    move_x(x);
    anim_step(x);
}

/// 0x3bf76 (code 0x38 and neighbors): like the recoil, animation 1, death through 0x3ddf6.
pub(super) fn recoil_1<R: Bench>(x: &mut Cx<'_, R>) {
    if dead(x) {
        mark_dead(x);
        return run_entry(x, 0x3ddf6);
    }
    if x.u8(f::STEP) != 0 {
        return recoil_shake(x);
    }
    apply_pending(x);
    next(x);
    x.set8(f::COUNTER, 16);
    stun(x);
    x.set16(f::VX, 0x180);
    x.set16(f::VY, -56i16 as u16);
    face_attacker(x);
    anim(x, 1);
    hit_sound(x);
}

/// Throw into the air: X speed `vx` (long), height 0x400, gravity -64, counter `counter`, animation
/// `anim_n`; `steps` steps of advance (two for some variants); `clear_67`: byte `+0x67` is cleared.
pub(super) fn throw<R: Bench>(x: &mut Cx<'_, R>, vx: u32, counter: u8, anim_n: u32, steps: u8, clear_67: bool) {
    if clear_67 {
        x.set8(0x67, 0);
    }
    apply_pending(x);
    x.set8(f::DOWNED, 0);
    anim(x, anim_n);
    x.add8(f::STEP, 2 * steps);
    x.set8(f::COUNTER, counter);
    x.set32(f::VX, vx);
    x.set16(f::VZ, 0x400);
    x.set16(f::GRAVITY, -64i16 as u16);
    x.set32(f::VX2, 0);
    face_attacker(x);
    stun(x);
    hurt(x);
}

/// 0x3bc42, 0x3bca0, 0x3bd00, 0x3bd60: throws (animation 2), stronger and stronger.
pub(super) fn throw_1<R: Bench>(x: &mut Cx<'_, R>) {
    throw(x, 0x0180_0000, 0, 2, 1, true);
}
pub(super) fn throw_2<R: Bench>(x: &mut Cx<'_, R>) {
    throw(x, 0x0200_0000, 1, 2, 1, true);
}
pub(super) fn throw_3<R: Bench>(x: &mut Cx<'_, R>) {
    throw(x, 0x0280_0000, 1, 2, 1, true);
}
pub(super) fn throw_4<R: Bench>(x: &mut Cx<'_, R>) {
    throw(x, 0x0300_0000, 1, 2, 1, true);
}
/// 0x3bfde, 0x3c03a, 0x3c098, 0x3c0f2: same throws, animation 3 (the first two skip a step).
pub(super) fn throw_1b<R: Bench>(x: &mut Cx<'_, R>) {
    throw(x, 0x0180_0000, 0, 3, 2, false);
}
pub(super) fn throw_2b<R: Bench>(x: &mut Cx<'_, R>) {
    throw(x, 0x0200_0000, 1, 3, 2, false);
}
pub(super) fn throw_3b<R: Bench>(x: &mut Cx<'_, R>) {
    throw(x, 0x0280_0000, 1, 3, 1, false);
}
pub(super) fn throw_4b<R: Bench>(x: &mut Cx<'_, R>) {
    throw(x, 0x0300_0000, 1, 2, 1, false);
}

// ---------------------------------------------------------------------------------------------
// High throws (0x3c14e and neighbors) and slides on the ground

/// 0x3c15e: hit that makes it take off (X speeds 0x600, height 0x280) after animation `n`.
pub(super) fn uppercut<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    anim(x, n);
    x.set8(0x67, 0);
    apply_pending(x);
    x.set8(f::DOWNED, 0);
    hurt(x);
    next(x);
    stun(x);
    speeds(x, 0x600, 0, 0x280, -64i16 as u16);
    face_attacker(x);
}
pub(super) fn uppercut_2<R: Bench>(x: &mut Cx<'_, R>) {
    uppercut(x, 2);
}
pub(super) fn uppercut_3<R: Bench>(x: &mut Cx<'_, R>) {
    uppercut(x, 3);
}

/// 0x3c192: high bounce (animation 5).
pub(super) fn bounce_high<R: Bench>(x: &mut Cx<'_, R>) {
    next(x);
    x.set16(f::VX, 0x300);
    x.set16(f::VZ, 0x300);
    x.set16(f::GRAVITY, -72i16 as u16);
    orient_speeds(x);
    x.set8(f::SUB_STEP, 0);
    anim(x, 5);
}

/// Start of a slide on the ground (animation 6) at speed `vx`.
pub(super) fn slide_start<R: Bench>(x: &mut Cx<'_, R>, vx: u16) {
    next(x);
    x.set8(f::SUB_STEP, 0);
    x.set16(f::VX, vx);
    x.set16(f::VY, -64i16 as u16);
    orient_speeds(x);
    anim(x, 6);
}
pub(super) fn slide_start_fast<R: Bench>(x: &mut Cx<'_, R>) {
    slide_start(x, 0x600);
}
pub(super) fn slide_start_slow<R: Bench>(x: &mut Cx<'_, R>) {
    slide_start(x, 0x400);
}

/// Slide dust, every other frame (0xb10c8).
pub(super) fn dust<R: Bench>(x: &mut Cx<'_, R>) {
    let was = x.bit(f::COUNTER, 0);
    x.set_bit(f::COUNTER, 0, !was);
    if !was {
        return;
    }
    lib::shock(x, 0, 1, 0, 0xffff);
}

/// 0x3c270: slide; stopped, the type 0 ENEMY waits for its animation's event for animation 7.
pub(super) fn slide_fast<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB_STEP) != 0 {
        anim_step(x);
        if x.u8(f::ANIM_EVENT) == 0 {
            return dust(x);
        }
        return next(x);
    }
    anim_step(x);
    move_x(x);
    if x.u8(f::BLOCKED) != 0 {
        x.set32(f::VX, 0);
    }
    if x.u8(f::VX) == 0 {
        x.set32(f::VX, 0);
        if x.u8(f::KIND) == 0 && x.u8(f::CLASS) == ENEMY {
            if x.u8(f::ANIM_EVENT) != 0 {
                next_sub(x);
                return anim(x, 7);
            }
        } else {
            next(x);
        }
    }
    dust(x)
}

/// 0x3c500: slow slide; stopped, animation 7 when the current animation signals it.
pub(super) fn slide_slow<R: Bench>(x: &mut Cx<'_, R>) {
    anim_step(x);
    if x.u8(f::SUB_STEP) != 0 {
        if x.u8(f::ANIM_EVENT) == 0 {
            return dust(x);
        }
        return next_clear(x);
    }
    move_x(x);
    if x.u8(f::BLOCKED) != 0 {
        x.set16(f::VX, 0);
        x.set16(f::VY, 0);
    }
    if x.u8(f::VX) == 0 {
        x.set16(f::VX, 0);
        x.set16(f::VY, 0);
        if x.u8(f::ANIM_EVENT) != 0 {
            next_sub(x);
            return anim(x, 7);
        }
    }
    dust(x)
}

/// 0x3c30a: low hit (animation 4).
pub(super) fn low_hit<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x67, 0);
    apply_pending(x);
    hurt(x);
    next(x);
    stun(x);
    speeds(x, 0x200, 0, 0x180, -72i16 as u16);
    face_attacker(x);
    anim(x, 4);
}

/// 0x3c374: skimming throw (18 frames, animation 1).
pub(super) fn skim<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x67, 0);
    apply_pending(x);
    hurt(x);
    next(x);
    x.set8(f::SUB_STEP, 0);
    stun(x);
    x.set8(f::COUNTER, 18);
    speeds(x, 0x800, 0, 0x100, -4i16 as u16);
    face_attacker(x);
    anim(x, 1);
}

/// 0x3c3ce: skimming flight (`+7`): 0 glide, 4 fall, 2 fall that restarts backward.
pub(super) fn skim_flight<R: Bench>(x: &mut Cx<'_, R>) {
    let s = x.u8(f::SUB_STEP);
    match s {
        0 => {
            move_xz(x);
            let c = x.u8(f::COUNTER).wrapping_sub(1);
            x.set8(f::COUNTER, c);
            if c == 0 {
                x.add8(f::SUB_STEP, 4);
                x.set8(f::COUNTER, 1);
                x.set16(f::GRAVITY, -64i16 as u16);
                x.set16(f::VY, -64i16 as u16);
                if flipped(x) {
                    neg16(x, f::VY);
                }
                return;
            }
            if x.u32(f::HEIGHT) as i32 <= 0 {
                return next(x);
            }
            let camera = if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA_ALT_X.get(x.b) } else { var::SCROLL_X.get(x.b) };
            let dx = x.u16(f::X).wrapping_sub(camera);
            if dx <= 0x180 && x.u8(f::BLOCKED) == 0 {
                return;
            }
            next_sub(x);
            x.set8(f::COUNTER, 0);
            x.set32(f::VX, 0);
            x.set16(f::VZ, 0);
            x.set16(f::GRAVITY, -64i16 as u16);
        }
        2 => {
            move_xz(x);
            if x.u16(f::HEIGHT) as i16 > 0 {
                return;
            }
            next(x);
            x.set32(f::HEIGHT, 0);
            x.set32(f::VX, 0);
            landed(x);
        }
        4 => {
            move_xz(x);
            if x.u16(f::HEIGHT) as i16 > 0 {
                return;
            }
            next(x);
            x.set32(f::HEIGHT, 0);
            x.set16(f::VY, 0);
            landed(x);
            x.set16(f::VX, 0x200);
            if flipped(x) {
                neg16(x, f::VX);
            }
        }
        _ => {}
    }
}

/// High throw (animation `n`, X speed `vx`, height `vz`, gravity `g`), counter 1.
pub(super) fn pop<R: Bench>(x: &mut Cx<'_, R>, n: u32, vx: u16, vz: u16, g: u16, clear_67: bool) {
    if clear_67 {
        x.set8(0x67, 0);
    }
    apply_pending(x);
    next(x);
    x.set8(f::SUB_STEP, 0);
    stun(x);
    anim(x, n);
    hurt(x);
    x.set8(f::COUNTER, 1);
    speeds(x, vx, 0, vz, g);
    face_attacker(x);
}

/// 0x3c58c: throw whose height depends on the reaction code (table 0x3c5f2), animation 9.
pub(super) fn pop_by_code<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x67, 0);
    apply_pending(x);
    next(x);
    x.set8(f::SUB_STEP, 0);
    stun(x);
    anim(x, 9);
    hurt(x);
    x.set8(f::COUNTER, 1);
    let i = (x.u8(f::REACTION) as u32).wrapping_sub(68) & 0xffff;
    let at = 0x3c5f2u32.wrapping_add(i as u16 as i16 as i32 as u32);
    let (vz, g) = (x.rom16(at), x.rom16(at + 2));
    speeds(x, 0x100, 0, vz, g);
    face_attacker(x);
}
pub(super) fn pop_10<R: Bench>(x: &mut Cx<'_, R>) {
    pop(x, 10, 64, 0xc00, -128i16 as u16, true);
}
pub(super) fn pop_11<R: Bench>(x: &mut Cx<'_, R>) {
    pop(x, 11, 0x600, 0xf800, -64i16 as u16, false);
}

/// 0x3c5fa: fall that animates while descending.
pub(super) fn fall_animated<R: Bench>(x: &mut Cx<'_, R>) {
    move_xz(x);
    if x.u16(f::HEIGHT) as i16 > 0 {
        if x.u8(f::BLOCKED) != 0 {
            x.set16(f::VX, 0);
            x.set16(f::VY, 0);
            x.set8(f::COUNTER, 0);
        }
        if (x.u8(f::VZ) as i8) < 0 {
            anim_step(x);
        }
        return;
    }
    next(x);
    x.set32(f::HEIGHT, 0);
    x.set32(f::VZ, 0);
    landed(x);
}
