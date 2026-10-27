//! Boss 1, état actif : arrivée (`+5` = 0), combat (2), attaques (4), attente (6 à 10), saisie (12).

use super::*;
use crate::objects::Courant;

// --- groupe 0 : arrivée ------------------------------------------------------------------------

/// Vitesses (X, Y) des huit sauts de la traversée, et animation de chacun.
const CROSSING_SPEEDS: u32 = 0x7af94;
const CROSSING_ANIMS: u32 = 0x7afb4;

/// 0x7ae5a : le boss traverse l'écran en huit sauts (en cinématique), disparaît, puis tombe du haut de l'écran.
pub(super) fn intro(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            x.set_a5_u16(0x51e, 4);
            x.set_bit(0x1, 1, false);
            x.set8(0, 2);
            x.set32(f::IMMUNITY, 0xffff_ffff);
            let cx = x.a5_u16(var::CAMERA1_X).wrapping_add(0x1a8);
            x.set16(0x8, cx);
            let cy = x.a5_u16(var::CAMERA1_Y).wrapping_add(0x68);
            x.set16(0xc, cy);
            x.set16(0x10, 0);
            x.set8(f::FLIP, 0);
            x.set16(TIMER, 8);
            x.set16(0x14, (-0x1000i16) as u16);
            x.set16(0x18, 0);
            puff(x, 0);
            x.c.d[0] = 0;
            x.c.a[4] = SET_BODY;
            step(x);
        }
        2 => crossing(x),
        4 => {
            if !expired(x, TIMER) {
                return;
            }
            next6(x);
            x.set_bit(0x1, 1, false);
            let cx = x.a5_u16(var::CAMERA1_X).wrapping_add(0x100);
            x.set16(0x8, cx);
            let cy = x.a5_u16(var::CAMERA1_Y);
            let z = cy.wrapping_add(0x58);
            x.set16(0x10, z);
            let h = cy.wrapping_add(0xf0).wrapping_sub(z);
            set_dw(x, 0, h);
            x.set16(0xc, h);
            x.set8(f::FLIP, 0);
            x.c.d[0] = 0;
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
            if random(x) & 1 != 0 {
                negate(x, 0x1c);
            }
            x.c.d[0] = 0;
            x.set32(0x14, 0);
            x.set16(0x1e, 0);
            x.call(call::LANDING);
            x.set16(0x18, 0x480);
            x.set16(0x1a, (-0x40i16) as u16);
            dust(x, 6);
            body(x, 0xb);
        }
        8 => {
            step(x);
            if x.u8(KEY) != 0 {
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
                step(x);
                return;
            }
            next6(x);
            x.set16(0xc, 0);
            dust(x, 6);
            x.call(call::LANDING);
            body(x, 0xe);
        }
        _ => {
            if !step(x) {
                return;
            }
            x.set_a5_u16(0x51e, 0);
            x.set16(0xc, 0);
            x.set8(0, 1);
            x.call(rom::IMMUNITIES);
            x.call(0xd5f30);
            walk(x);
        }
    }
}

/// 0x7aeda : la traversée, maintenue dans l'écran ; un saut par la table des vitesses à chaque bord touché.
fn crossing(x: &mut Cx) {
    x.move_xz();
    let mut bumped = false;
    let cam_x = x.a5_u16(var::CAMERA1_X);
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
    let cam_y = x.a5_u16(var::CAMERA1_Y);
    let h = x.u16(0xc) as i16;
    let low = cam_y.wrapping_add(0x10);
    let high = cam_y.wrapping_add(0xf0);
    if low as i16 >= h {
        bumped = true;
        x.set16(0xc, low);
        setw0(x, low);
    } else if high as i16 <= h {
        bumped = true;
        x.set16(0xc, high);
        setw0(x, high);
    } else {
        setw0(x, high);
    }
    x.c.d[5] = bumped as u32;
    if !bumped {
        return puff(x, 6);
    }
    if sub16(x, TIMER, 1).mi() {
        next6(x);
        x.set16(TIMER, 0x1e);
        x.set_bit(0x1, 1, true);
        return puff(x, 0);
    }
    let i = x.u16(TIMER) as u32 * 4;
    let vx = x.rom16(CROSSING_SPEEDS + i);
    x.set16(0x14, vx);
    let vy = x.rom16(CROSSING_SPEEDS + i + 2);
    x.set16(0x18, vy);
    let side = if (vx as i16) < 0 { 0 } else { 0x20 };
    x.set8(f::FLIP, side);
    let t = x.u16(TIMER) as u32;
    let n = x.rom16(CROSSING_ANIMS + t * 2);
    body(x, n as u32);
}

/// Mot faible de D0 (bornes calculées par la traversée).
fn setw0(x: &mut Cx, v: u16) {
    set_dw(x, 0, v);
}

// --- groupe 2 : combat -------------------------------------------------------------------------

pub(super) fn fight(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => idle(x),
        2 => turning(x),
        4 => hop_walk(x, false),
        6 => hop_walk(x, true),
        8 => leap_attack(x),
        _ => backing(x),
    }
}

/// Animation de repos : 4, ou 5 si la cible est à terre (`+0x66`).
fn rest_anim(x: &mut Cx) {
    let t = target_obj(x);
    let n = if t.u8(x.b, 0x66) != 0 { 5 } else { 4 };
    body(x, n);
}

/// 0x7b150 : debout ; sorti de l'écran il bondit, au bout de l'attente il marche, cible proche : attaque.
fn idle(x: &mut Cx) {
    if !alive(x) {
        return idle_alone(x);
    }
    if x.u8(0x7) == 0 {
        next7(x);
        x.call(call::ASK_RAND_TIMER);
        x.c.d[0] = 0;
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
    if target_in_box(x, -0x28, 0x30, 8) {
        return from_close(x);
    }
    if turn_needed(x) {
        return turn(x);
    }
    step(x);
}

/// 0x7b1d0 : demi-tour (animation 6), puis reprise de l'état mémorisé.
fn turning(x: &mut Cx) {
    if x.u8(0x7) == 0 {
        next7(x);
        x.c.d[0] = 1;
        x.set8(ANIM_KIND, 1);
        return body(x, 6);
    }
    if !step(x) {
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

/// 0x7b236 et 0x7b422 : marche par bonds vers des points tirés près de la cible.
fn hop_walk(x: &mut Cx, second: bool) {
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
            step(x);
            if x.u8(KEY) != 0 {
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
                step(x);
                return;
            }
            next7(x);
            x.set16(0xc, 0);
            dust(x, 6);
            x.call(call::LANDING);
            hop_anim(x, 3);
        }
        8 => {
            if !step(x) {
                return;
            }
            next7(x);
            let pause = if x.u8(ANIM_KIND) == 0 { 8 } else { 0x10 };
            x.c.d[0] = pause;
            x.set16(TIMER, pause as u16);
            rest_anim(x);
        }
        _ => between_hops(x),
    }
}

/// Animation `k` du bond, à partir de la première animation du sens (`+0xb4`).
fn hop_anim(x: &mut Cx, k: u8) {
    let n = k.wrapping_add(x.u8(HOP_ANIM));
    body(x, n as u32);
}

/// 0x7b268 : un bond vers un point près de la cible, sauf s'il faut d'abord se retourner.
fn hop(x: &mut Cx) {
    x.set8(0xbb, 0);
    if turn_needed(x) {
        return turn(x);
    }
    x.set8(0x7, 2);
    x.c.d[0] = 0;
    x.set16(0x16, 0);
    x.set16(0x1e, 0);
    aim_near_target(x, 0x7c410, 7);
    speed(x, 0x17);
    x.set16(0x18, 0x500);
    x.set16(0x1a, (-0x70i16) as u16);
    hop_height(x);
    x.c.d[0] = 7;
    x.call(call::FACING);
    let first = if zero(x) { 0xf } else { 7 };
    x.c.d[0] = first;
    x.set8(HOP_ANIM, first as u8);
    body(x, first);
}

/// 0x7b38e : entre deux bonds ; les boîtes devant le boss décident d'une attaque.
fn between_hops(x: &mut Cx) {
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
    if target_in_box(x, -0x28, 0x30, 8) {
        return from_close(x);
    }
    if in_wide_box(x, -0x68, 0x24, 8) {
        return from_wide_box(x);
    }
    step(x);
}

/// 0x7b45c : grand bond vers la cible ou le milieu de l'écran, puis attaque si on lui a riposté.
fn leap_attack(x: &mut Cx) {
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
                let center = x.a5_u16(var::CAMERA1_X).wrapping_add(0xc0);
                let side = if center as i16 >= x.u16(0x8) as i16 { 0x20 } else { 0 };
                x.c.d[0] = side;
                x.set8(f::FLIP, side as u8);
                mx = center;
                mz = x.u16(0x10);
            }
            set_dw(x, 1, mx);
            set_dw(x, 2, mz);
            x.set16(MARK_X, mx);
            x.set16(MARK_Z, mz);
            aim_at_mark(x);
            speed(x, 0x35);
            x.c.d[0] = 0;
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
            step(x);
            if x.u8(KEY) != 0 {
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
                step(x);
                return;
            }
            next7(x);
            x.set16(0xc, 0);
            x.c.d[0] = 0xa;
            x.call(call::SOUND_EVENT);
            dust(x, 6);
            x.call(call::LANDING);
            body(x, 0x16);
        }
        8 => {
            if !step(x) {
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
            if target_in_box(x, -0x60, 0x35, 6) {
                return go(x, 0x7c344);
            }
            go(x, 0x7c300)
        }
        _ => {
            if !expired(x, TIMER) {
                step(x);
                return;
            }
            walk(x)
        }
    }
}

/// 0x7b5fe : bond en arrière, glissade, puis riposte si la cible est devant.
fn backing(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            kind(x, 5);
            body(x, 0x17);
        }
        2 => {
            step(x);
            if x.u8(KEY) == 0 {
                return;
            }
            next7(x);
            let (angle, vz) = if x.u8(f::FLIP) != 0 { (0x18u32, 0x20u16) } else { (8, (-0x20i16) as u16) };
            x.c.d[0] = angle;
            set_dw(x, 1, vz);
            x.set8(ANGLE, angle as u8);
            x.set16(0x16, vz);
            x.call(call::RANDOM_TURN);
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
                step(x);
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
            set_dw(x, 0, v);
            x.set16(0x16, v);
            x.move_x();
            x.call(call::LANDING);
            x.call(0xecdba);
            body(x, 0x18);
        }
        _ => {
            if !sub16(x, TIMER2, 1).gt() {
                x.set16(TIMER2, 4);
                dust(x, 3);
            }
            if !expired(x, TIMER) {
                x.move_x();
                step(x);
                return;
            }
            if !alive(x) {
                return idle_alone(x);
            }
            if target_in_box(x, -0x60, 0x35, 6) {
                return go(x, 0x7c344);
            }
            if target_in_box(x, -0xe0, 0x30, 8) {
                return go(x, 0x7c2fa);
            }
            walk(x)
        }
    }
}

// --- groupe 4 : attaques -----------------------------------------------------------------------

pub(super) fn attacks(x: &mut Cx) {
    match x.u8(f::SUB2) {
        k @ 0..=6 => strike(x, k / 2),
        8 => grab_attempt(x),
        _ => split(x),
    }
}

/// 0x7b764 à 0x7b86a : coup `k` (0 à 3) : cri (0xe3da6), animation `k`, bouffées aux images-clés.
fn strike(x: &mut Cx, k: u8) {
    if x.u8(0x7) == 0 {
        next7(x);
        x.call(0xe3da6);
        kind(x, 0x10 + k);
        return attack(x, k as u32);
    }
    if !step(x) {
        if x.u8(KEY) == 0 {
            return;
        }
        x.set8(KEY, 0);
        x.call(0xe3f02);
        return dust(x, 7);
    }
    // 0x7b7ba : fin du coup ; l'enchaînement continue si le coup a porté
    x.set8(INVULNERABLE, 0);
    if sub8(x, COMBO, 1).gt() && x.bit(0x57, 2) {
        if x.u16(0x54) != 0 {
            let victim = x.linked(0x54);
            x.c.a[1] = victim.0;
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

/// 0x7b89e : saisie : une cible à portée de l'image-clé est attrapée (0xfd64).
fn grab_attempt(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            kind(x, 0x14);
            attack(x, 4);
        }
        2 => {
            if step(x) {
                next7(x);
                x.set8(INVULNERABLE, 0);
                return attack(x, 7);
            }
            if x.u8(KEY) != 0 {
                return;
            }
            x.c.d[0] = 6;
            x.call(call::GRAB_FIND);
            if zero(x) {
                return;
            }
            x.c.d[0] = 0;
            x.call(call::GRAB);
        }
        _ => {
            if !step(x) {
                return;
            }
            if try_split_leap(x) {
                return split_leap(x);
            }
            walk(x)
        }
    }
}

/// 0x7b91c : bond en ciseaux : un double (ENEMY 0x1a) saute de l'autre côté.
fn split(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            next7(x);
            spawn_double(x);
            x.set16(0x1c, 0x100);
            if random(x) & 1 != 0 {
                negate(x, 0x1c);
            }
            x.c.d[0] = 0;
            x.set32(0x14, 0);
            x.set16(0x1e, 0);
            x.set16(0x18, 0x480);
            x.set16(0x1a, (-0x40i16) as u16);
            body(x, 0xb);
        }
        2 => {
            step(x);
            if x.u8(KEY) != 0 {
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
                step(x);
                return;
            }
            next7(x);
            x.set16(0xc, 0);
            dust(x, 6);
            x.call(call::LANDING);
            body(x, 0xe);
        }
        _ => {
            if !step(x) {
                return;
            }
            x.set16(0xc, 0);
            x.set8(0, 1);
            x.call(rom::IMMUNITIES);
            walk(x)
        }
    }
}

// --- groupes 6 à 10 : attente --------------------------------------------------------------------

/// 0x7ba18 : tant qu'aucun joueur n'est en vie, le boss reste en pose.
pub(super) fn wait_for_players(x: &mut Cx) {
    if (x.a5_u8(var::PLAYERS_ALIVE) as i8) > 0 {
        return walk(x);
    }
    if x.u8(f::SUB2) == 0 {
        next6(x);
        x.set8(ANIM_LAST, 0xff);
        return body(x, 4);
    }
    step(x);
}

// --- groupe 12 : un joueur saisi ---------------------------------------------------------------

/// Durée entre deux morsures, selon la phase.
const BITE_DELAYS: u32 = 0x7bb3e;

/// 0x7ba50 : le joueur saisi (`+0x50`) est tenu (0xfda2) et mordu (4 PV) jusqu'au relâchement.
pub(super) fn holding(x: &mut Cx) {
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
            x.call(call::HOLD);
        }
        2 => {
            if !sub16(x, TIMER2, 1).gt() {
                x.call(0xedd96);
                // bouffée de sang à la gueule (0xaaa40)
                x.c.d[1] = 2;
                let mut dx = (-0x69i16) as u16;
                if x.u8(f::FLIP) != 0 {
                    dx = dx.wrapping_neg();
                }
                set_dw(x, 0, dx);
                let (px, ph, pz) = (x.u16(0x8), x.u16(0xc), x.u16(0x10));
                set_dw(x, 2, px.wrapping_add(dx));
                set_dw(x, 3, ph.wrapping_add(0x28));
                set_dw(x, 4, pz.wrapping_sub(1));
                x.call(0xaaa40);
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
                x.c.d[0] = phase * 2;
                let d = x.rom16(BITE_DELAYS + phase * 2);
                x.set16(TIMER2, d);
            }
            step(x);
            x.call(call::HOLD);
        }
        _ => {
            if step(x) {
                walk(x)
            }
        }
    }
}

/// 0x7bb1c : relâche le joueur (0xfd8c) et rejoue l'animation 7.
fn release(x: &mut Cx) {
    let t = target_obj(x);
    t.set_u8(x.b, 0x6f, 1);
    x.call(call::RELEASE);
    next7(x);
    attack(x, 7);
}
