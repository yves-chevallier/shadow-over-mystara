//! Common steps of the reaction routines: falls, bounces, get-up, slide end, death.

use super::*;

// Common steps

/// 0x3bdf0: waits for the end of the animation.
pub(super) fn wait_anim<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(E::ANIM_END) != 0 {
        next(x);
    } else {
        anim_step(x);
    }
}

/// 0x3be02: fall (X movement and height); on the ground, next step and landing.
pub(super) fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    move_xz(x);
    if x.u16(E::HEIGHT) as i16 > 0 {
        if x.u8(E::BLOCKED) != 0 {
            x.set32(E::VX, 0);
            x.set8(E::COUNTER, 0);
        }
        return anim_step(x);
    }
    next_clear(x);
    x.set32(E::HEIGHT, 0);
    x.set32(E::VZ, 0);
    landed(x);
}

/// 0x3be38: fall, byte `+0x38` at zero.
pub(super) fn fall_38<R: Bench>(x: &mut Cx<'_, R>) {
    fall(x);
    x.set8(E::FRAME_TAG, 0);
}

/// 0x3e22e: next step; the non-grabbed type 3 boss chains its own reaction (0x81ae0).
pub(super) fn boss_3_hook<R: Bench>(x: &mut Cx<'_, R>) {
    next(x);
    if x.u8(E::KIND) == 3 && x.u8(E::CLASS) == BOSS && x.u8(E::SUB) != 6 && x.u8(E::GRABBED) == 0 {
        crate::families::boss_3::counter_reaction(x);
    }
}

/// 0x3be52: bounce on the ground (animation 5).
pub(super) fn bounce_start<R: Bench>(x: &mut Cx<'_, R>) {
    x.set16(E::AX, 0);
    x.set16(E::VZ, 0x300);
    x.set16(E::GRAVITY, -72i16 as u16);
    next(x);
    x.set8(E::SUB_STEP, 0);
    anim(x, 5);
}
/// 0x3be40: bounce backward.
pub(super) fn bounce_back<R: Bench>(x: &mut Cx<'_, R>) {
    x.set16(E::VX, 0x200);
    if flipped(x) {
        neg16(x, E::VX);
    }
    bounce_start(x);
}
/// 0x3be70: bounce (animation 2) after a link.
pub(super) fn bounce_linked<R: Bench>(x: &mut Cx<'_, R>) {
    take_pending(x);
    x.set16(E::VZ, 0x300);
    x.set16(E::GRAVITY, -72i16 as u16);
    next(x);
    x.set8(E::SUB_STEP, 0);
    anim(x, 2);
}
/// 0x3cf74: bounce (animation 2).
pub(super) fn bounce_2<R: Bench>(x: &mut Cx<'_, R>) {
    x.set16(E::VZ, 0x300);
    x.set16(E::GRAVITY, -72i16 as u16);
    next(x);
    x.set8(E::SUB_STEP, 0);
    anim(x, 2);
}

/// 0x3bea8: the bounce, four sub-steps (`+7`).
pub(super) fn bounce<R: Bench>(x: &mut Cx<'_, R>) {
    let s = x.u8(E::SUB_STEP);
    match s {
        0 => {
            anim_step(x);
            if x.u8(E::KEY_FRAME) != 0 {
                next_sub(x);
            }
        }
        2 => {
            move_xz(x);
            if x.u8(E::BLOCKED) != 0 {
                x.set32(E::VX, 0);
            }
            if x.u16(E::HEIGHT) as i16 > 0 {
                return;
            }
            next_sub(x);
            landed(x);
        }
        4 => {
            if (x.u8(E::ANIM_END) as i8) >= 0 {
                return anim_step(x);
            }
            if x.u8(E::COUNTER) == 0 {
                return next_clear(x);
            }
            x.set16(E::VX, 0x280);
            x.set16(E::AX, -64i16 as u16);
            if flipped(x) {
                neg16(x, E::VX);
                neg16(x, E::AX);
            }
            next_sub(x);
        }
        6 => {
            move_x(x);
            if x.u8(E::VX) == 0 {
                next_clear(x);
            }
        }
        _ => {}
    }
}

/// 0x3e0dc: get-up; death (substate 6): next step.
pub(super) fn get_up<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(E::AX_LONG, 0);
    x.set8(E::VULNERABLE, 0);
    if x.u8(E::SUB) == 6 {
        return next_clear(x);
    }
    x.set8(E::DOWNED, 1);
    if !kept_down(x) {
        get_up_done(x);
    }
}
/// 0x3df7c: end of the slide: the object gets up.
pub(super) fn slide_end<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(E::AX_LONG, 0);
    if var::GAME_ON.get(x.b) != 0 {
        x.set8(E::VULNERABLE, 1);
    }
    x.set8(E::ACTIVE, 1);
    x.set8(E::REACTION, 0);
    if !kept_down(x) {
        resume_after_slide(x);
    }
}

/// 0x3e19a: a grabbed object (`+0x87`) on the ground may stay down (reaction code 104, or 108 if it was
/// down): true in that case; otherwise the caller continues (A6 designates its continuation in the original: `jmp
/// (A6)`).
pub(super) fn kept_down<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u8(E::GRABBED) != 0 {
        // `bhi` on the last comparison of 0x1e92 (zero margin): an object exactly on the edge in width counts as
        // inside
        if !crate::objects::cull::beyond_arena(x.b, x.o, 0) {
            x.set16(E::RECOVER, 1);
            let i = type_index(x);
            let keep = tables::no_grab_down().at8(x.b, i);
            if keep == 0 {
                let code = if x.u8(E::DOWNED) != 0 { 108 } else { 104 };
                x.set8(E::REACTION, code);
                x.set16(E::STEP, 0);
                return true;
            }
        }
    }
    x.set8(E::GRABBED, 0);
    false
}

/// 0x3e0fc: standing, the main state resumes (0x04040000); the boss's get-up (0xf118) except code 108.
pub(super) fn get_up_done<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(E::ACTIVE, 1);
    x.set32(E::STATE, 0x0404_0000);
    x.set32(E::HIT_LOCK, 0);
    x.set8(E::VULNERABLE, 0);
    x.set16(E::RECOVER, 1);
    if x.u8(E::REACTION) != 108 {
        lib::draw_84(x);
    }
}

/// 0x3df9e: after the slide, the saved state (`+0x8a`) resumes if it can, otherwise the active state and the
/// type's get-up routine.
pub(super) fn resume_after_slide<R: Bench>(x: &mut Cx<'_, R>) {
    let saved = x.u32(E::RESUME_STATE);
    if saved != 0 {
        x.set32(E::HIT_LOCK, 0);
        x.set8(E::ACTIVE, 1);
        if x.u16(E::RESUME_STATE) == 0x205 && lib::roll_78(x) {
                x.set32(E::STATE, saved);
                x.set8(E::SUB_STEP, 0);
                x.set32(E::RESUME_STATE, 0);
                x.set8(E::AI_SHOT, 0xff);
                return;
            }
    }
    x.set32(E::STATE, 0x0202_0000);
    x.set8(E::AI_SHOT, 0xff);
    let base = if x.u8(E::CLASS) == BOSS { 32 } else { 0 };
    let i = (base + x.u8(E::KIND) as u32) & 0xff;
    get_up_by_kind(x, i);
}

/// 0x3dd9c: death; 40 frames of blinking, then the object disappears (0x19ad6).
pub(super) fn death<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(E::SUB_STEP) == 0 {
        x.set8(E::COUNTER, 40);
        next_sub(x);
        detach_from_host(x);
        lib::drop_rewards(x);
    }
    let c = x.u8(E::COUNTER).wrapping_sub(1);
    x.set8(E::COUNTER, c);
    if c == 0 {
        x.set32(E::STATE, 0x0600_0000);
        return lib::object_event(x);
    }
    blink(x, c);
}
/// Blinking: bit 1 of `+1` (hidden) one frame out of four... according to the two low bits of the counter.
pub(super) fn blink<R: Bench>(x: &mut Cx<'_, R>, c: u8) {
    x.set_bit(E::DISPLAY, 1, false);
    if c & 3 != 0 {
        x.set_bit(E::DISPLAY, 1, true);
    }
}

/// 0x3e158: next step, then 0x51c06.
pub(super) fn step_51c06<R: Bench>(x: &mut Cx<'_, R>) {
    next(x);
    crate::families::creators::weapon_shot(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o);
}
/// 0x3e162: next step.
pub(super) fn skip<R: Bench>(x: &mut Cx<'_, R>) {
    next(x);
}
/// `rts`.
pub(super) fn idle<R: Bench>(_: &mut Cx<'_, R>) {}
