//! Type 13, sous-état 2 (debout : approche, demi-tour, attente) et sous-état 10 (promenade tant qu'aucun joueur
//! n'est en vie).

use super::*;
use crate::objects::Courant;

pub(super) fn run(x: &mut Cx) {
    // la variante 10 est saisie dès qu'elle se met debout
    if x.u8(VARIANT) == 0xa || grabbed(x) {
        x.set16(f::STATE, 0x0408);
        x.set16(f::SUB2, 0);
        return grab(x);
    }
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => approach(x),
        2 => turn(x),
        4 => wait(x),
        _ => {}
    }
    if x.a5_u8(0x35) == 0 {
        set_state_long(x, IDLE);
    }
}

/// 0x6affa : (re)commence la marche vers la cible.
pub(super) fn approach_start(x: &mut Cx) {
    x.add8(STEP, 2);
    x.set8(LOOK_PREV, 0xf0);
    x.set8(SHOT_KIND, 0x1a);
    approach_restart(x);
}

/// 0x6b00a : suite de `approach_start` sans avancer l'étape (appelé directement après un coup).
pub(super) fn approach_restart(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    set_speed(x, 0x12);
    mv(x, 1);
    pick_target(x);
    x.call(call::FACE_TARGET);
    x.call(call::TURN_TO_TARGET);
}

/// +6 = 0 : marche vers le joueur ; choisit un geste dès qu'il est à portée.
fn approach(x: &mut Cx) {
    if x.u8(STEP) == 0 {
        return approach_start(x);
    }
    x.call(call::WALK_STEP);
    let n = x.u8(AI_CHARGE).wrapping_sub(1);
    x.set8(AI_CHARGE, n);
    if (n as i8) < 0 {
        x.call(call::FACE_TARGET);
        if !pick_target(x) {
            return;
        }
        let m = x.u8(AI_WANDER).wrapping_sub(1);
        x.set8(AI_WANDER, m);
        if m == 0 {
            x.call(call::TURN_TO_TARGET);
            x.call(call::RAND_TARGET_CHECK);
            if !x.c.cond(7) {
                x.set16(f::SUB2, 0x400);
                return;
            }
        }
    }
    // joueur au contact : coup immédiat (4 ou 6), au hasard
    if target_in_box(x, -0x20, 0x20, 0xc) {
        x.set16(f::STATE, 0x0204);
        let r = rand_and(x, 1);
        let chosen = (r * 2 + 4) as u8;
        x.c.d[0] = x.c.d[0] & !0xff | chosen as u32;
        x.set8(CHOSEN, chosen);
        x.set16(f::SUB2, 0);
        return;
    }
    if near_box(x) {
        pick_attack(x, NEAR_TABLE);
        return;
    }
    if far_box(x) {
        pick_attack(x, FAR_TABLE);
        return;
    }
    if target_in_box_far(x, -0x38, 0x38, 0xc) {
        let chosen = x.u8(f::SUB2);
        x.set8(CHOSEN, chosen);
        x.set16(f::SUB2, 0);
        return x.call(call::ANNOUNCE);
    }
    x.anim_step();
    if x.u8(KEY) & 1 == 0 {
        x.move_by_table_xy();
    }
    if faces_wrong(x) {
        x.add8(f::SUB2, 2);
        x.set8(STEP, 0);
    }
}

/// +6 = 2 : demi-tour.
fn turn(x: &mut Cx) {
    if x.u8(STEP) == 0 {
        x.add8(STEP, 2);
        x.set8(VULNERABLE, 1);
        return mv(x, 2);
    }
    if x.anim_step() {
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        x.set8(f::SUB2, 0);
        mv(x, 1);
    }
}

/// +6 = 4 : attente sur place.
fn wait(x: &mut Cx) {
    if x.u8(STEP) == 0 {
        x.add8(STEP, 2);
        x.set8(VULNERABLE, 1);
        return mv(x, 0);
    }
    let n = x.u8(AI_WAIT).wrapping_sub(1);
    x.set8(AI_WAIT, n);
    if (n as i8) < 0 {
        x.set16(f::SUB2, 0);
    }
    x.anim_step();
}

/// Sous-état 10 : promenade sur place tant qu'aucun joueur n'est en vie, puis retour debout.
pub(super) fn idle(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set8(VULNERABLE, 1);
        return mv(x, 0);
    }
    if x.a5_u8(0x35) == 0 {
        x.anim_step();
        return;
    }
    set_state_long(x, STANCE);
    pick_target(x);
}
