//! Type 5, ghoul debout (`+5 = 2`) et en patrouille (`+5 = 10`). `+6` : 0 et 6 marche vers la cible et
//! décide d'attaquer, 2 demi-tour, 4 attente.

use super::*;
use crate::objects::Obj;

pub(super) fn run(x: &mut Cx) {
    if special_hit(x) {
        return;
    }
    if grabbed(x) {
        return set_state_long(x, KNOCKED);
    }
    match x.u8(f::SUB2) {
        0 | 6 => {
            if x.u8(f::SUB3) == 0 {
                begin_walk(x);
            } else {
                walk(x);
            }
        }
        2 => turn(x),
        4 => wait(x),
        _ => {}
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        set_state_long(x, IDLE);
    }
}

/// L'aspect voulu `v` ; l'animation `anim` n'est lancée (et rendu vrai) que si l'aspect change.
fn set_look(x: &mut Cx, v: u8, anim: u32) -> bool {
    x.set8(LOOK, v);
    if v == x.u8(LOOK_PREV) {
        return false;
    }
    x.set8(LOOK_PREV, v);
    body(x, anim);
    true
}

/// 0x5fcec : le ghoul se met en marche vers sa cible.
pub(super) fn begin_walk(x: &mut Cx) {
    x.add8(f::SUB3, 2);
    x.set8(VULNERABLE, 1);
    set_look(x, 0, 2);
    set_speed(x, 0xd);
    pick_target(x);
    lib::draw_stance(x);
    lib::draw_6b(x);
}

/// 0x5fd3e : marche ; se retourne, attend ou attaque.
fn walk(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        x.set8(AI_SHOT, 0);
        lib::walk_near_point(x);
    } else {
        x.set8(AI_SHOT, 9);
        lib::approach_target(x);
    }
    let n = x.u8(AI_WALK).wrapping_sub(1);
    x.set8(AI_WALK, n);
    if (n as i8) < 0 {
        lib::draw_stance(x);
        if !pick_target(x) {
            return;
        }
        let n = x.u8(AI_CHOICE).wrapping_sub(1);
        x.set8(AI_CHOICE, n);
        if n == 0 {
            lib::draw_6b(x);
            if lib::maybe_draw_6e(x) {
                return x.set16(f::SUB2, 0x400);
            }
        }
    }
    decide(x);
}

/// 0x5fd96 : un joueur à portée ? Si oui, attaque ; sinon avance d'un pas d'animation.
fn decide(x: &mut Cx) {
    if lib::target_in_box(x, -0x20, 0x20, 8) {
        x.set16(f::STATE, 0x204);
        return pick_attack(x, CLOSE_ATTACKS, 0);
    }
    if box_attack(x, 0, 0x50, 8, CLOSE_ATTACKS, 0) || box_attack(x, -0x80, 0x30, 8, FAR_ATTACKS, 0xa) {
        return;
    }
    if lib::guard_in_front_strict(x, -8, 0x58, 0x18) {
        return lib::pick_byte(x);
    }
    step(x);
    move_xy(x);
    if needs_turn_around(x) {
        x.set16(f::SUB2, 0x200);
    }
}

/// `+6` = 2 : demi-tour (animation 1, puis inversion de l'orientation).
fn turn(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        look(x, 2);
        x.set8(VULNERABLE, 1);
        return body(x, 1);
    }
    if step(x) {
        x.set16(f::SUB2, 0);
        body(x, 0);
    }
}

/// `+6` = 4 : attente.
fn wait(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        if set_look(x, 4, 0) {
            return;
        }
    }
    let n = x.u8(AI_WAIT).wrapping_sub(1);
    x.set8(AI_WAIT, n);
    if (n as i8) < 0 {
        x.set16(f::SUB2, 0);
    }
    step(x);
}

/// Sous-état 10 : patrouille tant qu'aucun joueur n'est en vie.
pub(super) fn patrol(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(TIMER, 0x20);
            set_angle(x, 8, 0x18);
            set_speed(x, 3);
            body(x, 2);
        }
        2 => {
            let t = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, t);
            if t == 0 {
                x.add8(f::SUB2, 2);
                x.set8(TIMER, 0x40);
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                body(x, 1);
            } else {
                move_xy(x);
                step(x);
            }
        }
        4 => {
            if step(x) {
                x.set8(f::SUB2, 2);
                x.add8(Obj::SPEED_INDEX, 0x10);
                let a = x.u8(Obj::SPEED_INDEX) & 0x1f;
                x.set8(Obj::SPEED_INDEX, a);
                body(x, 2);
            }
        }
        _ => {}
    }
    if var::PLAYERS_ALIVE.get(x.b) != 0 {
        set_state_long(x, STANCE);
    }
}
