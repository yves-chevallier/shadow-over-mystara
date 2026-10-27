//! Type 2, troglodyte debout (`+5 = 2`) et au repos (`+5 = 10`). `+6` : 0 marche vers la cible et
//! décide d'attaquer, 2 demi-tour, 4 attente, 6 bond d'approche.

use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

pub(super) fn run(x: &mut Cx) {
    if x.u8(BLINK_ON) != 0 {
        let n = x.u16(BLINK_COUNT).wrapping_sub(1);
        x.set16(BLINK_COUNT, n);
        if (n as i16) < 0 {
            x.set8(BLINK_ON, 0);
            x.set_bit(1, 1, false);
        }
    }
    match x.u8(f::SUB2) {
        0 => {
            if x.u8(f::SUB3) == 0 {
                begin_walk(x);
            } else {
                walk(x);
            }
        }
        2 => turn(x),
        4 => wait(x),
        6 => jump(x),
        _ => {}
    }
    if x.a5_u8(0x35) == 0 && (x.u16(Obj::Z) as i16) <= 0 {
        set_state_long(x, IDLE);
    }
}

/// 0x5a08a : le troglodyte se met en marche vers sa cible.
fn begin_walk(x: &mut Cx) {
    x.add8(f::SUB3, 2);
    x.set8(AI_SHOT, 0x14);
    x.set8(VULNERABLE, 1);
    x.set16(LOOK, 0);
    body(x, 2);
    x.c.d[0] = 0;
    walk_speed(x, 0);
    pick_target(x);
    x.call(call::FACE_TARGET);
    x.call(call::TURN_TO_TARGET);
}

/// 0x5a0ce : marche ; se retourne, attend ou attaque ; saisi par un joueur, il part en chute.
fn walk(x: &mut Cx) {
    x.call(call::WALK_STEP);
    if grabbed(x) {
        x.set16(f::STATE, 0x408);
        x.set16(f::SUB2, 0);
        blink_if_running(x);
        return hit::caught(x);
    }
    let n = x.u8(AI_WALK).wrapping_sub(1);
    x.set8(AI_WALK, n);
    if (n as i8) < 0 {
        x.call(call::FACE_TARGET);
        if !pick_target(x) {
            return;
        }
        let n = x.u8(AI_CHOICE).wrapping_sub(1);
        x.set8(AI_CHOICE, n);
        if n == 0 {
            x.call(call::TURN_TO_TARGET);
            x.call(call::RAND_TARGET_CHECK);
            if !x.c.cond(7) {
                return x.set16(f::SUB2, 0x400);
            }
        }
    }
    decide(x);
}

/// 0x5a13c : un joueur à portée ? Si oui, attaque ; sinon avance d'un pas d'animation.
fn decide(x: &mut Cx) {
    if target_in_box(x, -0x20, 0x20, 8) {
        set_state_long(x, 0x0204_1600);
        x.set8(RESUME, 6);
        return blink_if_running(x);
    }
    if front_box(x) {
        return pick_attack_close(x);
    }
    if side_box(x) {
        return pick_attack_far(x);
    }
    if target_in_box_far(x, -0x38, 0x38, 8) {
        let hint = x.u8(f::SUB2);
        x.set8(RESUME, hint);
        x.set16(f::SUB2, 0x1600);
        return x.call(call::ANNOUNCE);
    }
    step(x);
    move_xy(x);
    if x.u8(SERIES) != 0 {
        if x.u8(AI_SHOT) != 0x14 {
            return begin_jump(x);
        }
    } else if x.u8(BLINK_PHASE) != 0 && (x.u8(BLINK_PHASE) as i8) < 0 && (x.u16(BLINK_REST) as i16) < 0 {
        x.set8(BLINK_PHASE, 0);
    }
    if needs_turn_around(x) {
        x.add8(f::SUB2, 2);
        x.set8(f::SUB3, 0);
    }
}

/// 0x5b002 : un coup est tiré dans la ligne de la difficulté ; il sera joué après l'annonce.
pub(super) fn pick_attack_close(x: &mut Cx) {
    let table = if x.u8(SERIES) != 0 { CLOSE_ATTACKS_ARMED } else { CLOSE_ATTACKS };
    let mv = pick_byte(x, table);
    x.c.d[4] = x.c.d[4] & !0xff | mv as u32;
    if mv <= 8 {
        x.call(call::PICK_ATTACK_KIND);
        if !x.c.cond(7) {
            return x.set8(AI_SHOT, 0x15);
        }
    }
    blink_if_running(x);
    x.set8(f::SUB, 4);
    x.set8(RESUME, mv);
    x.set8(f::SUB3, 0);
    x.set8(f::SUB2, 0x16);
}

/// 0x5b474 : coup d'approche, 0x12 ou 0x14 au hasard.
pub(super) fn pick_attack_far(x: &mut Cx) {
    x.add8(f::SUB, 2);
    x.set8(RESUME, 0x12);
    if rand_and(x, 1) != 0 {
        x.add8(RESUME, 2);
    }
    x.set8(f::SUB3, 0);
    x.set8(f::SUB2, 0x16);
    blink_if_running(x);
}

/// 0x5a208 : bond de la série 1 vers la cible.
fn begin_jump(x: &mut Cx) {
    x.set8(f::SUB2, 6);
    x.set8(f::SUB3, 2);
    x.set16(Obj::VZ, 0x500);
    x.set16(Obj::AZ, -0x38i16 as u16);
    x.set16(Obj::VX, 0x300);
    x.set16(Obj::AX, 0);
    if x.u8(f::FLIP) == 0 {
        negate(x, Obj::VX);
    }
    body(x, 0xa);
}

/// `+6` = 6 : bond d'approche (étape 0 : envol, 2 : montée et chute, 4 : réception).
fn jump(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => begin_jump(x),
        2 => {
            x.move_xz();
            if step(x) && (x.u16(Obj::Z) as i16) < 0 {
                x.add8(f::SUB3, 2);
                x.set16(Obj::Z, 0);
                body(x, 0xc);
            }
        }
        4 => {
            if step(x) {
                x.set16(f::SUB2, 0);
                if needs_turn_around(x) {
                    x.set8(f::SUB2, 2);
                    x.set8(f::SUB3, 0);
                }
            }
        }
        _ => {}
    }
}

/// `+6` = 2 : demi-tour (animation 1, puis inversion de l'orientation).
fn turn(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        return body(x, 1);
    }
    if step(x) {
        x.set8(f::SUB2, 0);
        body(x, 2);
    }
}

/// `+6` = 4 : attente.
fn wait(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        x.set8(LOOK_PREV, 1);
        return body(x, 0);
    }
    let n = x.u8(AI_WAIT).wrapping_sub(1);
    x.set8(AI_WAIT, n);
    if (n as i8) < 0 {
        x.set8(f::SUB, 2);
        x.set16(f::SUB2, 0);
    }
    step(x);
}

/// Sous-état 10 : repos jusqu'à ce qu'un joueur soit en vie.
pub(super) fn rest(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        return rest_begin(x);
    }
    if x.a5_u8(0x35) != 0 {
        set_state_long(x, STANCE);
        pick_target(x);
    } else {
        step(x);
    }
}
