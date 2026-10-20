//! Type 15, sous-état 2 (debout : approche, demi-tour, attente, sortie d'écran) et sous-état 10 (promenade tant
//! qu'aucun joueur n'est en vie).

use super::*;

pub(super) fn run(x: &mut Cx) {
    if x.u8(NO_ZONE) != 0 || grabbed(x) {
        set_state_long(x, HELD);
        return grab(x);
    }
    if x.a5_u8(0x35) == 0 {
        return set_state_long(x, IDLE);
    }
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => approach(x),
        2 => turn(x),
        4 => wait(x),
        6 => walk_off(x),
        _ => {}
    }
}

/// 0x6de46 : (re)commence la marche vers la cible.
pub(super) fn approach_start(x: &mut Cx) {
    x.add8(STEP, 2);
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
        x.set8(SHOT_KIND, 0x1a);
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
    x.set8(LOOK_PREV, 0x80);
    x.anim_step();
    if x.u8(KEY) == 0 {
        x.call(sub::MOVE_TABLE_XY);
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

/// +6 = 6 (et sous-état 4, +6 = 20) : le Goblin quitte l'écran en marchant. L'original rend la main à la boucle
/// avec une remise en liste (`jmp 0x1ee1a`) à plusieurs endroits : on la rejoue.
pub(super) fn walk_off(x: &mut Cx) {
    let step = x.u8(STEP);
    if step == 2 {
        x.call(call::SCREEN_CHECK);
        if x.c.cond(5) {
            return x.add8(f::STATE, 4);
        }
        x.anim_step();
        if x.u8(KEY) == 0 {
            x.call(sub::MOVE_TABLE_XY);
        }
        return x.reinsert();
    }
    if step > 2 {
        if x.anim_step() {
            set_speed(x, 0x21);
            x.add8(STEP, 0xfe);
            let flip = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
        }
        return x.reinsert();
    }
    x.add8(STEP, 2);
    x.set8(ANGLE, 8);
    let mut want = 0x20u8;
    let base = if x.a5_u8(0x36) != 0 { 0x556 } else { 0x506 };
    let d = x.u16(X).wrapping_sub(x.a5_u16(base));
    if (d as i16) <= 0xc0 {
        x.set8(ANGLE, 8);
        want = 0;
    }
    x.c.d[4] = x.c.d[4] & !0xff | want as u32;
    if want != x.u8(f::FLIP) {
        x.add8(STEP, 4);
        mv(x, 2);
    } else {
        set_speed(x, 0x21);
        mv(x, 1);
    }
    x.reinsert();
}

/// Sous-état 10 : promenade sur place tant qu'aucun joueur n'est en vie, puis retour debout.
pub(super) fn idle(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set8(VULNERABLE, 1);
        return mv(x, 0);
    }
    let n = x.u8(AI_WAIT).wrapping_sub(1);
    x.set8(AI_WAIT, n);
    if (n as i8) < 0 {
        x.set8(f::SUB, 2);
        x.set16(f::SUB2, 0);
    }
    x.anim_step();
}
