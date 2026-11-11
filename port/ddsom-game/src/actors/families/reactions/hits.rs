//! Reactions held in place or thrown by a special hit: held by a spell, sparks, disintegration, daze, blast, hop,
//! tumble, floored.

use super::*;

pub(super) fn held_start<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(E::VULNERABLE, 0);
    let px = x.u16(E::X);
    x.set16(E::X_SAVED, px);
    let n = x.u16(E::RECOVER).wrapping_sub(10);
    x.set16(E::RECOVER, n);
    x.set8(E::ACTIVE, 2);
    hurt(x);
    next(x);
    stun(x);
    x.set8(E::COUNTER, 16);
    anim(x, 8);
}
pub(super) fn held_shake<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(E::COUNTER) == 0 {
        next(x);
        let px = x.u16(E::X_SAVED);
        return x.set16(E::X, px);
    }
    let c = x.u8(E::COUNTER).wrapping_sub(1);
    x.set8(E::COUNTER, c);
    shake(x, 2);
}
pub(super) fn held_end<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(E::SUB) == 6 {
        next_clear(x);
        return anim_step(x);
    }
    x.set32(E::HIT_LOCK, 0);
    if x.u16(E::RECOVER) as i16 <= 0 {
        x.set8(E::ACTIVE, 2);
    }
    if x.u8(E::CLASS) == BOSS {
        lib::countdown_then_state(x);
        return anim_step(x);
    }
    x.set32(E::AX_LONG, 0);
    x.set8(E::ACTIVE, 1);
    x.set32(E::STATE, 0x0404_0200);
    anim_step(x);
}

/// 0x3c7fa: two sparks (EFFCT type 8, variants 1 and 0) if the terrain allows.
pub(super) fn sparks<R: Bench>(x: &mut Cx<'_, R>) {
    next(x);
    if pools::EFFCT.free_count(x.b) < 2 {
        return;
    }
    for v in [1u8, 0] {
        // Original glitch: the allocation failure is not tested (see `Pool::alloc_unchecked`).
        let o = x.alloc_unchecked(&pools::EFFCT);
        o.set_active(x.b, 1);
        o.set_kind(x.b, 8);
        o.set_variant(x.b, v);
        Effect(o).set_link(x.b, x.o.0 as u16);
    }
}

/// 0x3c828: disintegration (type 1 ENEMY: 0xeb448, otherwise 0xe35a6), then blinking and disappearance.
pub(super) fn disintegrate<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u8(E::STEP) == 0 {
        next(x);
        x.set8(E::SUB, 6);
        x.set8(E::ACTIVE, 2);
        x.set8(E::LOOT, 0);
        x.set8(E::LOOT_ROW, 0);
        anim(x, 12);
        let sound = if x.u8(E::KIND) == 1 { crate::services::sound::CMD_304_404 } else { crate::services::sound::CMD_370_470 };
        lib::sound(x, sound);
        return true;
    }
    if (x.u16(E::ANIM_END) as i16) >= 0 {
        anim_step(x);
        return true;
    }
    if x.u8(E::SUB_STEP) == 0 {
        x.set8(E::COUNTER, 40);
        next_sub(x);
        detach_from_host(x);
    }
    let c = x.u8(E::COUNTER).wrapping_sub(1);
    x.set8(E::COUNTER, c);
    if c == 0 {
        x.set32(E::STATE, 0x0600_0000);
        return true;
    }
    blink(x, c);
    true
}

/// 0x3c8d8: stunned: randomly drawn delay (table 0x3c904), stars (0xabec8) at the end of each delay.
pub(super) fn dazed<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(E::VULNERABLE, 0);
    apply_pending(x);
    next(x);
    anim(x, 3);
    face_attacker(x);
    dazed_delay(x);
}
pub(super) fn dazed_delay<R: Bench>(x: &mut Cx<'_, R>) {
    let r = (x.random() as u32) & 3;
    let d = tables::daze_delays().at8(x.b, r);
    x.set8(E::COUNTER, d);
    hurt(x);
}
pub(super) fn dazed_wait<R: Bench>(x: &mut Cx<'_, R>) {
    if !paused(x) {
        next(x);
    }
    let c = x.u8(E::COUNTER).wrapping_sub(1);
    x.set8(E::COUNTER, c);
    if c != 0 {
        return;
    }
    if effect(x, 0, 0) {
        return dazed_delay(x);
    }
    x.set8(E::COUNTER, 1);
}
/// 0x3c936: predefined throw 1, freeze of one frame.
pub(super) fn knock<R: Bench>(x: &mut Cx<'_, R>) {
    hurt(x);
    launch(x, 1);
    x.set8(E::FREEZE, 1);
    if flipped(x) {
        neg32(x, E::VX);
    }
}

/// 0x3c970: big skimming throw (stars 0xabec8(2)).
pub(super) fn blast<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(E::VULNERABLE, 0);
    next(x);
    apply_pending(x);
    x.set8(E::SUB_STEP, 0);
    x.set8(E::COUNTER, 0xff);
    hurt(x);
    stun(x);
    x.set16(E::VX, 0x800);
    x.set16(E::AX, 0);
    x.set32(E::VZ, 0x0100_0000);
    x.set16(E::GRAVITY, 0);
    anim(x, 1);
    if flipped(x) {
        neg16(x, E::VX);
    }
    x.set8(E::COUNTER, 18);
    effect(x, 2, 0);
}

/// 0x3c9ec: animation 19 (before a throw).
pub(super) fn wind_up<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(E::VULNERABLE, 0);
    apply_pending(x);
    next(x);
    x.set8(E::SUB_STEP, 0);
    anim(x, 19);
}
/// 0x3ca04: the animation plays while the game is paused, then next step.
pub(super) fn wind_up_wait<R: Bench>(x: &mut Cx<'_, R>) {
    anim_step(x);
    if !paused(x) {
        next(x);
    }
}
/// 0x3ca18 / 0x3ca3a: predefined throw 3 (animation 2), HP at zero for the first.
pub(super) fn launch_3<R: Bench>(x: &mut Cx<'_, R>, kill: bool) {
    anim(x, 2);
    if kill {
        x.set16(E::HP, 0);
    }
    hurt(x);
    launch(x, 3);
    if flipped(x) {
        neg32(x, E::VX);
    }
}
pub(super) fn launch_3_kill<R: Bench>(x: &mut Cx<'_, R>) {
    launch_3(x, true);
}
pub(super) fn launch_3_hurt<R: Bench>(x: &mut Cx<'_, R>) {
    launch_3(x, false);
}

/// 0x3ca96: vertical hop (animation 0).
pub(super) fn hop<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(E::VULNERABLE, 0);
    apply_pending(x);
    next(x);
    if dead(x) {
        mark_dead(x);
    }
    x.set16(E::VZ, 0x400);
    x.set16(E::GRAVITY, 64);
    x.set32(E::VX, 0);
    anim(x, 0);
}
/// 0x3cacc: rises to the top of the screen (0x110 above the camera), then 0x3cafc.
pub(super) fn hop_rise<R: Bench>(x: &mut Cx<'_, R>) {
    move_xz(x);
    let camera = if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA_ALT_Y.get(x.b) } else { var::SCROLL_Y.get(x.b) };
    let above = x.u16(E::HEIGHT).wrapping_add(x.u16(E::Y)).wrapping_sub(camera);
    if above >= 0x110 {
        next(x);
    }
    hop_top(x);
}
/// 0x3cafc: when the linked object (`+0x54`) is at sub-step 4, falls back from a random height (animation 11).
pub(super) fn hop_top<R: Bench>(x: &mut Cx<'_, R>) {
    let holder = E(x.linked(E::HOLDER));
    if holder.sub_step(x.b) != 4 {
        return;
    }
    let r = ((x.random() as u32) & 7) << 4;
    x.add16(E::HEIGHT, r as u16);
    x.set8(E::STEP, 6);
    x.set16(E::VZ, 0xfc00);
    x.set16(E::GRAVITY, -128i16 as u16);
    anim(x, 11);
}
/// 0x3cb32: fall; on the ground, sound and 0x2824 (D0 = 8).
pub(super) fn hop_land<R: Bench>(x: &mut Cx<'_, R>) {
    fall(x);
    if x.u16(E::HEIGHT) != 0 {
        return;
    }
    hurt(x);
    lib::shake_54(x, 8);
}

/// 0x3cbaa: backward throw (counter 1, X 0x200, height 0x600).
pub(super) fn knockback_speeds<R: Bench>(x: &mut Cx<'_, R>) {
    next(x);
    x.set8(E::COUNTER, 1);
    x.set32(E::VX, 0x0200_0000);
    x.set16(E::VZ, 0x600);
    x.set16(E::GRAVITY, -64i16 as u16);
}
/// Hit sound, or death and death sound, the object remaining active (`+0` = 2).
pub(super) fn hurt_active<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(E::ACTIVE, 2);
    if dead(x) {
        x.set8(E::SUB, 6);
        death_sound(x);
    } else {
        hit_sound(x);
    }
}
/// 0x3cc3e: on the ground, the link goes back to pending, animation 13.
pub(super) fn tumble_anim<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(E::VULNERABLE, 0);
    x.set8(E::DOWNED, 0);
    defer_link(x);
    anim(x, 13);
}
/// 0x3cb6c: backward throw, somersault.
pub(super) fn tumble<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(E::VULNERABLE, 0);
    x.set16(E::AY, 0);
    x.set16(E::VY, 64);
    x.set8(E::GRABBED, 0);
    knockback_speeds(x);
    face_attacker(x);
    hurt_active(x);
    tumble_anim(x);
}
/// 0x3cbca: like the somersault, animation 18.
pub(super) fn tumble_18<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(E::VULNERABLE, 0);
    x.set8(E::DOWNED, 0);
    x.set8(E::GRABBED, 0);
    knockback_speeds(x);
    face_attacker(x);
    hurt_active(x);
    defer_link(x);
    anim(x, 18);
}
/// 0x3cc00: somersault, animation 18 and stars 0xabec8(5).
pub(super) fn tumble_18_stars<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(E::VULNERABLE, 0);
    x.set8(E::DOWNED, 0);
    x.set8(E::GRABBED, 0);
    knockback_speeds(x);
    face_attacker(x);
    anim(x, 18);
    effect(x, 5, 0);
    hurt_active(x);
}
/// 0x3cd0c: somersault, animation 13 and stars 0xabec8(7).
pub(super) fn tumble_13_stars<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(E::VULNERABLE, 0);
    x.set16(E::AY, 0);
    x.set16(E::VY, 64);
    x.set8(E::GRABBED, 0);
    x.set8(E::DOWNED, 0);
    knockback_speeds(x);
    face_attacker(x);
    anim(x, 13);
    effect(x, 7, 0);
    hurt_active(x);
}
/// 0x3cc6e: rise in three dimensions; at the top, next step (active if not dead).
pub(super) fn tumble_rise<R: Bench>(x: &mut Cx<'_, R>) {
    move_xyz(x);
    if x.u16(E::VZ) as i16 > 0 {
        return tumble_blocked(x);
    }
    next(x);
    if x.u8(E::SUB) != 6 {
        x.set8(E::ACTIVE, 1);
    }
}
/// 0x3cc90: descent in three dimensions down to the ground.
pub(super) fn tumble_fall<R: Bench>(x: &mut Cx<'_, R>) {
    move_xyz(x);
    if x.u16(E::HEIGHT) as i16 > 0 {
        return tumble_blocked(x);
    }
    next_clear(x);
    x.set32(E::HEIGHT, 0);
    x.set32(E::VZ, 0);
    landed(x);
}
pub(super) fn tumble_blocked<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(E::BLOCKED) != 0 {
        x.set16(E::VX, 0);
        x.set16(E::AX, 0);
        x.set8(E::COUNTER, 0);
    }
    anim_step(x);
}
/// 0x3ccca: next step, the pending link becomes effective.
pub(super) fn tumble_link<R: Bench>(x: &mut Cx<'_, R>) {
    next(x);
    take_pending(x);
}

/// 0x3cdb0 / 0x3cf1e: knocked over (animation 17, sound 0xebbda); the second adds the stars 0xabec8(8).
pub(super) fn floored<R: Bench>(x: &mut Cx<'_, R>, stars: bool) {
    x.set8(E::VULNERABLE, 0);
    x.set8(E::DOWNED, 0);
    next(x);
    x.set8(E::GRABBED, 0);
    if dead(x) {
        mark_dead(x);
    }
    speeds(x, 0x200, 0, 0x400, -128i16 as u16);
    face_attacker(x);
    if !stars {
        defer_link(x);
    }
    anim(x, 17);
    lib::sound(x, crate::services::sound::CMD_31E_41E_B);
    if stars {
        effect(x, 8, 0);
    }
}
pub(super) fn floored_plain<R: Bench>(x: &mut Cx<'_, R>) {
    floored(x, false);
}
pub(super) fn floored_stars<R: Bench>(x: &mut Cx<'_, R>) {
    floored(x, true);
}
/// 0x3ce4c / 0x3ce84: 30 frames on the ground; the first variant launches 0xab7d8(1).
pub(super) fn floored_wait<R: Bench>(x: &mut Cx<'_, R>, cry: bool) {
    if x.u8(E::SUB_STEP) == 0 {
        next_sub(x);
        x.set8(E::COUNTER, 30);
    }
    let c = x.u8(E::COUNTER).wrapping_sub(1);
    x.set8(E::COUNTER, c);
    if c != 0 {
        return;
    }
    next_clear(x);
    x.set8(E::COUNTER, 0);
    if cry {
        lib::debris_spray(x, 1);
    }
}
pub(super) fn floored_wait_cry<R: Bench>(x: &mut Cx<'_, R>) {
    floored_wait(x, true);
}
pub(super) fn floored_wait_quiet<R: Bench>(x: &mut Cx<'_, R>) {
    floored_wait(x, false);
}
/// 0x3ceb4 / 0x3cecc: outside pause, next step (the first launches 0xab7d8(1)).
pub(super) fn unpaused_cry<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    next_clear(x);
    lib::debris_spray(x, 1);
}
pub(super) fn unpaused_next<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    next_clear(x);
}

/// 0x3d6f8: knocked over without a link (animation 17, stars 0xabec8(9)), oriented speeds.
pub(super) fn floored_9<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(E::VULNERABLE, 0);
    x.set8(E::DOWNED, 0);
    next(x);
    x.set8(E::GRABBED, 0);
    x.set8(E::ACTIVE, 2);
    if dead(x) {
        x.set8(E::SUB, 6);
    }
    speeds(x, 0x200, 0, 0x400, -128i16 as u16);
    orient_speeds(x);
    anim(x, 17);
    lib::sound(x, crate::services::sound::CMD_31E_41E_B);
    effect(x, 9, 0);
}
