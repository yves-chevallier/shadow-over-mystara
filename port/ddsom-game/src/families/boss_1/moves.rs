//! Boss 1, active state: arrival (`+5` = 0), combat (2), attacks (4), wait (6 to 10), grab (12).

use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

// --- group 0: arrival ------------------------------------------------------------------------

/// Speeds (X, Y) of the eight jumps of the crossing, and the animation of each.
const CROSSING_SPEEDS: u32 = 0x7af94;
const CROSSING_ANIMS: u32 = 0x7afb4;

/// 0x7ae5a: the boss crosses the screen in eight jumps (in cutscene), disappears, then falls from the top of the
/// screen.
pub(super) fn intro<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            x.set_a5_u16(0x51e, 4);
            x.set_bit(0x1, 1, false);
            x.set8(0, 2);
            x.set32(f::IMMUNITY, 0xffff_ffff);
            let cx = var::CAMERA1_X.get(x.b).wrapping_add(0x1a8);
            x.set16(0x8, cx);
            let cy = var::CAMERA1_Y.get(x.b).wrapping_add(0x68);
            x.set16(0xc, cy);
            x.set16(0x10, 0);
            x.set8(f::FLIP, 0);
            x.set16(TIMER, 8);
            x.set16(0x14, (-0x1000i16) as u16);
            x.set16(0x18, 0);
            puff(x, 0);
            lib::step(x);
        }
        2 => crossing(x),
        4 => {
            if !expired(x, TIMER) {
                return;
            }
            next6(x);
            x.set_bit(0x1, 1, false);
            let cx = var::CAMERA1_X.get(x.b).wrapping_add(0x100);
            x.set16(0x8, cx);
            let cy = var::CAMERA1_Y.get(x.b);
            let z = cy.wrapping_add(0x58);
            x.set16(0x10, z);
            let h = cy.wrapping_add(0xf0).wrapping_sub(z);
            x.set16(0xc, h);
            x.set8(f::FLIP, 0);
            for off in [0x14, 0x16, 0x18] {
                x.set16(off, 0);
            }
            x.set16(0x1a, (-0x40i16) as u16);
            body(x, 3);
        }
        6 => {
            x.move_xz();
            if (x.u16(0xc) as i16) > 0 {
                return puff(x, 8);
            }
            next6(x);
            x.set16(0xc, 0);
            spawn_double(x);
            x.set16(0x1c, 0x100);
            if (x.random() as u32) & 1 != 0 {
                negate(x, 0x1c);
            }
            x.set32(0x14, 0);
            x.set16(0x1e, 0);
            lib::sound(x, crate::sound::CMD_35F_45F);
            x.set16(0x18, 0x480);
            x.set16(0x1a, (-0x40i16) as u16);
            dust(x, 6);
            body(x, 0xb);
        }
        8 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) != 0 {
                return;
            }
            x.move_xyz();
            if x.u16(0x18) as i16 > 0x100 {
                return;
            }
            next6(x);
            body(x, 0xc);
        }
        10 => {
            x.move_xyz();
            if x.u16(0x18) as i16 > -0x100 {
                return;
            }
            next6(x);
            body(x, 0xd);
        }
        12 => {
            x.move_xyz();
            if x.u16(0xc) as i16 > 0 {
                lib::step(x);
                return;
            }
            next6(x);
            x.set16(0xc, 0);
            dust(x, 6);
            lib::sound(x, crate::sound::CMD_35F_45F);
            body(x, 0xe);
        }
        _ => {
            if !lib::step(x) {
                return;
            }
            x.set_a5_u16(0x51e, 0);
            x.set16(0xc, 0);
            x.set8(0, 1);
            lib::set_immunities(x);
            lib::create_set_14(x);
            walk(x);
        }
    }
}

/// 0x7aeda: the crossing, kept within the screen; a jump from the speed table at each edge touched.
fn crossing<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xz();
    let mut bumped = false;
    let cam_x = var::CAMERA1_X.get(x.b);
    let px = x.u16(0x8) as i16;
    let right = cam_x.wrapping_add(0x1b0);
    let left = cam_x.wrapping_sub(0x20);
    if right as i16 <= px {
        bumped = true;
        x.set16(0x8, right);
    } else if left as i16 >= px {
        bumped = true;
        x.set16(0x8, left);
    }
    let cam_y = var::CAMERA1_Y.get(x.b);
    let h = x.u16(0xc) as i16;
    let low = cam_y.wrapping_add(0x10);
    let high = cam_y.wrapping_add(0xf0);
    if low as i16 >= h {
        bumped = true;
        x.set16(0xc, low);
    } else if high as i16 <= h {
        bumped = true;
        x.set16(0xc, high);
    }
    *x.c.d5() = bumped as u32;
    if !bumped {
        return puff(x, 6);
    }
    if sub16(x, TIMER, 1).mi() {
        next6(x);
        x.set16(TIMER, 0x1e);
        x.set_bit(0x1, 1, true);
        return puff(x, 0);
    }
    // index `(d8,PC,D0.w)`: quadruple computed on a word, sign-extended
    let i = x.u16(TIMER).wrapping_mul(4) as i16 as i32 as u32;
    let vx = x.rom16(CROSSING_SPEEDS.wrapping_add(i));
    x.set16(0x14, vx);
    let vy = x.rom16(CROSSING_SPEEDS.wrapping_add(i).wrapping_add(2));
    x.set16(0x18, vy);
    let side = if (vx as i16) < 0 { 0 } else { 0x20 };
    x.set8(f::FLIP, side);
    let t = x.u16(TIMER).wrapping_mul(2) as i16 as i32 as u32;
    let n = x.rom16(CROSSING_ANIMS.wrapping_add(t));
    body(x, n as u32);
}

// --- groupe 2 : combat -------------------------------------------------------------------------

pub(super) fn fight<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => idle(x),
        2 => turning(x),
        4 => hop_walk(x, false),
        6 => hop_walk(x, true),
        8 => leap_attack(x),
        _ => backing(x),
    }
}

/// Rest animation: 4, or 5 if the target is on the ground (`+0x66`).
fn rest_anim<R: Bench>(x: &mut Cx<'_, R>) {
    let t = target_obj(x);
    let n = if t.u8(x.b, 0x66) != 0 { 5 } else { 4 };
    body(x, n);
}

/// 0x7b150: standing; off screen it leaps, at the end of the wait it walks, target close: attack.
fn idle<R: Bench>(x: &mut Cx<'_, R>) {
    if !alive(x) {
        return idle_alone(x);
    }
    if x.u8(0x7) == 0 {
        next7(x);
        lib::draw_6e(x);
        x.set8(ANIM_KIND, 0);
        x.set8(ANIM_LAST, 0);
        return rest_anim(x);
    }
    if outside(x, -0x50, 0x1d0) {
        return leap(x, false);
    }
    if !sub8(x, IDLE_COUNT, 1).gt() {
        return walk(x);
    }
    if lib::target_in_box(x, -0x28, 0x30, 8) {
        return from_close(x);
    }
    if turn_needed(x) {
        return turn(x);
    }
    lib::step(x);
}

/// 0x7b1d0: about-face (animation 6), then resumption of the saved state.
fn turning<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x7) == 0 {
        next7(x);
        x.set8(ANIM_KIND, 1);
        return body(x, 6);
    }
    if !lib::step(x) {
        return;
    }
    let side = x.u8(f::FLIP) ^ 0x20;
    x.set8(f::FLIP, side);
    if !alive(x) {
        return idle_alone(x);
    }
    let s = x.u32(SAVED_STATE);
    x.set32(f::STATE, s);
    let last = x.u8(ANIM_LAST);
    if last == 2 || last == 3 {
        x.set8(ANIM_KIND, last);
        return hop(x);
    }
    body(x, 4);
}

/// 0x7b236 and 0x7b422: walk by leaps toward points drawn near the target.
fn hop_walk<R: Bench>(x: &mut Cx<'_, R>, second: bool) {
    match x.u8(0x7) {
        0 => {
            if second {
                draw_steps(x, 0x7c060);
                if !pick_target(x) {
                    return idle_alone(x);
                }
                aim_near_target(x, 0x7c400, 3);
                kind(x, 3);
            } else {
                if !pick_target(x) {
                    return idle_alone(x);
                }
                draw_steps(x, 0x7c040);
                kind(x, 2);
            }
            hop(x)
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) != 0 {
                return;
            }
            x.move_xyz();
            if x.u16(0x18) as i16 > 0x100 {
                return;
            }
            next7(x);
            hop_anim(x, 1);
        }
        4 => {
            x.move_xyz();
            if x.u16(0x18) as i16 > -0x100 {
                return;
            }
            next7(x);
            hop_anim(x, 2);
        }
        6 => {
            x.move_xyz();
            if x.u16(0xc) as i16 > 0 {
                lib::step(x);
                return;
            }
            next7(x);
            x.set16(0xc, 0);
            dust(x, 6);
            lib::sound(x, crate::sound::CMD_35F_45F);
            hop_anim(x, 3);
        }
        8 => {
            if !lib::step(x) {
                return;
            }
            next7(x);
            let pause = if x.u8(ANIM_KIND) == 0 { 8 } else { 0x10 };
            x.set16(TIMER, pause as u16);
            rest_anim(x);
        }
        _ => between_hops(x),
    }
}

/// Leap animation `k`, starting from the first animation of the direction (`+0xb4`).
fn hop_anim<R: Bench>(x: &mut Cx<'_, R>, k: u8) {
    let n = k.wrapping_add(x.u8(HOP_ANIM));
    body(x, n as u32);
}

/// 0x7b268: a leap toward a point near the target, unless it must turn around first.
fn hop<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0xbb, 0);
    if turn_needed(x) {
        return turn(x);
    }
    x.set8(0x7, 2);
    x.set16(0x16, 0);
    x.set16(0x1e, 0);
    aim_near_target(x, 0x7c410, 7);
    speed(x, 0x17);
    x.set16(0x18, 0x500);
    x.set16(0x1a, (-0x70i16) as u16);
    hop_height(x);
    let first = if lib::moving_forward(x) { 0xf } else { 7 };
    x.set8(HOP_ANIM, first as u8);
    body(x, first);
}

/// 0x7b38e: between two leaps; the boxes in front of the boss decide on an attack.
fn between_hops<R: Bench>(x: &mut Cx<'_, R>) {
    if !alive(x) {
        return idle_alone(x);
    }
    if expired(x, TIMER) {
        if try_split_leap(x) {
            return split_leap(x);
        }
        if !sub8(x, WALK_STEPS, 1).gt() {
            return walk_b(x);
        }
        if outside(x, 0x10, 0x170) {
            return leap(x, true);
        }
        return hop(x);
    }
    if outside(x, -0x50, 0x1d0) {
        return leap(x, false);
    }
    if let Some(g) = ai_box(x, -0x70, 0x1a, 6) {
        return from_near_gesture(x, g);
    }
    if let Some(g) = ai_box(x, -0xe0, 0x30, 8) {
        return from_far_gesture(x, g);
    }
    if lib::target_in_box(x, -0x28, 0x30, 8) {
        return from_close(x);
    }
    if in_wide_box(x, -0x68, 0x24, 8) {
        return from_wide_box(x);
    }
    lib::step(x);
}

/// 0x7b45c: big leap toward the target or the middle of the screen, then attack if it was countered.
fn leap_attack<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            let (mx, mz);
            if x.u8(LEAP_TO_CENTER) == 0 {
                let mut d = 0x60u16;
                if x.u8(f::FLIP) == 0 {
                    d = d.wrapping_neg();
                }
                let t = target_obj(x);
                mx = d.wrapping_add(t.u16(x.b, 0x8));
                mz = t.u16(x.b, 0x10);
            } else {
                let center = var::CAMERA1_X.get(x.b).wrapping_add(0xc0);
                let side = if center as i16 >= x.u16(0x8) as i16 { 0x20 } else { 0 };
                x.set8(f::FLIP, side as u8);
                mx = center;
                mz = x.u16(0x10);
            }
            *x.c.d2() = *x.c.d2() & 0xffff_0000 | mz as u32;
            x.set16(MARK_X, mx);
            x.set16(MARK_Z, mz);
            aim_at_mark(x, (mx, mz));
            speed(x, 0x35);
            x.set16(0x16, 0);
            x.set16(0x1e, 0);
            x.set16(0x18, 0x500);
            x.set16(0x1a, (-0x40i16) as u16);
            hop_height(x);
            next7(x);
            kind(x, 4);
            body(x, 0x13);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) != 0 {
                return;
            }
            x.move_xyz();
            if x.u16(0x18) as i16 > 0x100 {
                return;
            }
            next7(x);
            body(x, 0x14);
        }
        4 => {
            x.move_xyz();
            if x.u16(0x18) as i16 > -0x100 {
                return;
            }
            next7(x);
            body(x, 0x15);
        }
        6 => {
            x.move_xyz();
            if x.u16(0xc) as i16 > 0 {
                lib::step(x);
                return;
            }
            next7(x);
            x.set16(0xc, 0);
            lib::shake_54(x, 0xa);
            dust(x, 6);
            lib::sound(x, crate::sound::CMD_35F_45F);
            body(x, 0x16);
        }
        8 => {
            if !lib::step(x) {
                return;
            }
            next7(x);
            let side = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, side);
            body(x, 4);
            if x.u8(INVULNERABLE) == 0 {
                x.set16(TIMER, 0xc);
                return;
            }
            x.set8(INVULNERABLE, 0);
            if turn_needed(x) {
                return turn(x);
            }
            if lib::target_in_box(x, -0x60, 0x35, 6) {
                return go(x, 0x7c344);
            }
            go(x, 0x7c300)
        }
        _ => {
            if !expired(x, TIMER) {
                lib::step(x);
                return;
            }
            walk(x)
        }
    }
}

/// 0x7b5fe: backward leap, slide, then counterattack if the target is ahead.
fn backing<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            kind(x, 5);
            body(x, 0x17);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            next7(x);
            let (angle, vz) = if x.u8(f::FLIP) != 0 { (0x18u32, 0x20u16) } else { (8, (-0x20i16) as u16) };
            x.set8(Obj::SPEED_INDEX, angle as u8);
            x.set16(0x16, vz);
            lib::drift_heading(x);
            speed(x, 0x35);
            x.set16(0x18, 0x300);
            x.set16(0x1a, (-0x40i16) as u16);
            x.move_xz();
        }
        4 => {
            x.move_xz();
            if x.u16(0x18) as i16 <= 0 {
                next7(x);
                x.set16(0x16, 0);
            }
        }
        6 => {
            x.move_xz();
            if x.u16(0xc) as i16 > 0 {
                lib::step(x);
                return;
            }
            next7(x);
            x.set16(TIMER, 0xa);
            x.set16(TIMER2, 0);
            x.set16(0xc, 0);
            x.set8(INVULNERABLE, 0);
            let mut v = (-0x48i16) as u16;
            if x.u8(f::FLIP) != 0 {
                v = v.wrapping_neg();
            }
            x.set16(0x16, v);
            x.move_x();
            lib::sound(x, crate::sound::CMD_35F_45F);
            lib::sound(x, crate::sound::CMD_329_429);
            body(x, 0x18);
        }
        _ => {
            if !sub16(x, TIMER2, 1).gt() {
                x.set16(TIMER2, 4);
                dust(x, 3);
            }
            if !expired(x, TIMER) {
                x.move_x();
                lib::step(x);
                return;
            }
            if !alive(x) {
                return idle_alone(x);
            }
            if lib::target_in_box(x, -0x60, 0x35, 6) {
                return go(x, 0x7c344);
            }
            if lib::target_in_box(x, -0xe0, 0x30, 8) {
                return go(x, 0x7c2fa);
            }
            walk(x)
        }
    }
}

// --- groupe 4 : attaques -----------------------------------------------------------------------

pub(super) fn attacks<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        k @ 0..=6 => strike(x, k / 2),
        8 => grab_attempt(x),
        _ => split(x),
    }
}

/// 0x7b764 to 0x7b86a: hit `k` (0 to 3): cry (0xe3da6), animation `k`, puffs at the key frames.
fn strike<R: Bench>(x: &mut Cx<'_, R>, k: u8) {
    if x.u8(0x7) == 0 {
        next7(x);
        lib::sound(x, crate::sound::CMD_30C_40C);
        kind(x, 0x10 + k);
        return attack(x, k as u32);
    }
    if !lib::step(x) {
        if x.u8(f::KEY_FRAME) == 0 {
            return;
        }
        x.set8(f::KEY_FRAME, 0);
        lib::sound(x, crate::sound::CMD_35C_45C);
        return dust(x, 7);
    }
    // 0x7b7ba: end of the hit; the chain continues if the hit connected
    x.set8(INVULNERABLE, 0);
    if sub8(x, COMBO, 1).gt() && x.bit(0x57, 2) {
        if x.u16(0x54) != 0 {
            let victim = x.linked(0x54);
            victim.set_u8(x.b, 0x7e, 0);
            victim.set_u8(x.b, 0x53, 0);
        }
        if (x.u8(PHASE) as i8) < 2 {
            return go(x, 0x7c304);
        }
        return go(x, 0x7c2de);
    }
    if !alive(x) {
        return idle_alone(x);
    }
    if try_split_leap(x) {
        return split_leap(x);
    }
    stand(x)
}

/// 0x7b89e: grab: a target within range of the key frame is caught (0xfd64).
fn grab_attempt<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            kind(x, 0x14);
            attack(x, 4);
        }
        2 => {
            if lib::step(x) {
                next7(x);
                x.set8(INVULNERABLE, 0);
                return attack(x, 7);
            }
            if x.u8(f::KEY_FRAME) != 0 {
                return;
            }
            if let Some(player) = lib::attack_hits_a_player(x, 6) {
                lib::seize_player(x, player, 0);
            }
        }
        _ => {
            if !lib::step(x) {
                return;
            }
            if try_split_leap(x) {
                return split_leap(x);
            }
            walk(x)
        }
    }
}

/// 0x7b91c: scissor leap: a double (ENEMY 0x1a) jumps to the other side.
fn split<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            spawn_double(x);
            x.set16(0x1c, 0x100);
            if (x.random() as u32) & 1 != 0 {
                negate(x, 0x1c);
            }
            x.set32(0x14, 0);
            x.set16(0x1e, 0);
            x.set16(0x18, 0x480);
            x.set16(0x1a, (-0x40i16) as u16);
            body(x, 0xb);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) != 0 {
                return;
            }
            x.move_xyz();
            if x.u16(0x18) as i16 > 0x100 {
                return;
            }
            next7(x);
            body(x, 0xc);
        }
        4 => {
            x.move_xyz();
            if x.u16(0x18) as i16 > -0x100 {
                return;
            }
            next7(x);
            body(x, 0xd);
        }
        6 => {
            x.move_xyz();
            if x.u16(0xc) as i16 > 0 {
                lib::step(x);
                return;
            }
            next7(x);
            x.set16(0xc, 0);
            dust(x, 6);
            lib::sound(x, crate::sound::CMD_35F_45F);
            body(x, 0xe);
        }
        _ => {
            if !lib::step(x) {
                return;
            }
            x.set16(0xc, 0);
            x.set8(0, 1);
            lib::set_immunities(x);
            walk(x)
        }
    }
}

// --- groups 6 to 10: wait --------------------------------------------------------------------

/// 0x7ba18: while no player is alive, the boss stays in its pose.
pub(super) fn wait_for_players<R: Bench>(x: &mut Cx<'_, R>) {
    if (var::PLAYERS_ALIVE.get(x.b) as i8) > 0 {
        return walk(x);
    }
    if x.u8(f::SUB2) == 0 {
        next6(x);
        x.set8(ANIM_LAST, 0xff);
        return body(x, 4);
    }
    lib::step(x);
}

// --- group 12: a grabbed player ---------------------------------------------------------------

/// Time between two bites, depending on the phase.
const BITE_DELAYS: u32 = 0x7bb3e;

/// 0x7ba50: the grabbed player (`+0x50`) is held (0xfda2) and bitten (4 HP) until released.
pub(super) fn holding<R: Bench>(x: &mut Cx<'_, R>) {
    let t = target_obj(x);
    if t.u8(x.b, 0) == 0 || t.u8(x.b, 0x188) == 0 {
        return stand(x);
    }
    match x.u8(0x7) {
        0 => {
            next7(x);
            let n = if (x.u8(PHASE) as i8) < 2 { 5 } else { 6 };
            attack(x, n);
            x.set8(INVULNERABLE, 0);
            x.set16(TIMER, 6);
            x.set16(TIMER2, 0);
            lib::hold_seized(x);
        }
        2 => {
            if !sub16(x, TIMER2, 1).gt() {
                lib::sound(x, crate::sound::CMD_341_441);
                // puff of blood at the mouth (0xaaa40)
                let mut dx = (-0x69i16) as u16;
                if x.u8(f::FLIP) != 0 {
                    dx = dx.wrapping_neg();
                }
                let (px, ph, pz) = (x.u16(0x8), x.u16(0xc), x.u16(0x10));
                let pos = (px.wrapping_add(dx), ph.wrapping_add(0x28), pz.wrapping_sub(1));
                *x.c.d2() = *x.c.d2() & 0xffff_0000 | pos.0 as u32;
                *x.c.d3() = *x.c.d3() & 0xffff_0000 | pos.1 as u32;
                *x.c.d4() = *x.c.d4() & 0xffff_0000 | pos.2 as u32;
                // the other object of the spark is A1, the target loaded at the head of the state
                crate::families::effects_creators::spark_at(x.b, x.o, t.0, 2, pos);
                let t = target_obj(x);
                let hp = t.u16(x.b, f::HP);
                t.set_u16(x.b, f::HP, hp.wrapping_sub(4));
                if Fl::sub16(hp, 4).le() {
                    t.set_u16(x.b, f::HP, 1);
                    return release(x);
                }
                if expired(x, TIMER) {
                    return release(x);
                }
                let phase = x.u8(PHASE) as u32;
                let d = x.rom16(BITE_DELAYS + phase * 2);
                x.set16(TIMER2, d);
            }
            lib::step(x);
            lib::hold_seized(x);
        }
        _ => {
            if lib::step(x) {
                walk(x)
            }
        }
    }
}

/// 0x7bb1c: releases the player (0xfd8c) and replays animation 7.
fn release<R: Bench>(x: &mut Cx<'_, R>) {
    let t = target_obj(x);
    t.set_u8(x.b, 0x6f, 1);
    lib::release_player(x, t);
    next7(x);
    attack(x, 7);
}
