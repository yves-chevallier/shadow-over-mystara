//! Type 14, sous-état 2 (debout : approche, demi-tour, attente, sortie d'écran) et sous-état 10 (promenade tant
//! qu'aucun joueur n'est en vie).

use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

pub(super) fn run(x: &mut Cx) {
    if x.u8(NO_ZONE) != 0 || grabbed(x) {
        set_state_long(x, HELD);
        return grab(x);
    }
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => approach(x),
        2 => turn(x),
        4 => wait(x),
        6 => walk_off(x),
        _ => {}
    }
    // plus aucun joueur en vie : le Troll se met en attente (les quatre octets d'état des joueurs)
    let alive = [0x5de, 0x7de, 0x9de, 0xbde].iter().fold(0u8, |sum, &off| sum.wrapping_add(x.a5_u8(off)));
    x.c.d[0] = x.c.d[0] & !0xff | alive as u32;
    if alive == 0 {
        set_state_long(x, IDLE);
    }
}

/// 0x6c81c : (re)commence la marche vers la cible.
pub(super) fn approach_start(x: &mut Cx) {
    x.add8(f::SUB3, 2);
    x.set8(VULNERABLE, 1);
    set_speed(x, 0x12);
    mv(x, 1);
    pick_target(x);
    x.call(call::FACE_TARGET);
    x.call(call::TURN_TO_TARGET);
}

/// +6 = 0 : marche vers le joueur ; choisit un geste dès qu'il est à portée.
fn approach(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
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
    // joueur au contact : coup immédiat (lancer ou saut), au hasard
    if target_in_box(x, -0x20, 0x20, 0xc) {
        x.set16(f::STATE, 0x0204);
        let r = rand_and(x, 4);
        let chosen = if r != 0 { 4 } else { 6 };
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
    x.set8(LOOK_PREV, 0x80);
    x.anim_step();
    x.move_by_table_xy();
    if faces_wrong(x) {
        x.add8(f::SUB2, 2);
        x.set8(f::SUB3, 0);
    }
}

/// +6 = 2 : demi-tour.
fn turn(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
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
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
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

/// +6 = 6 (et sous-état 4, +6 = 20) : le Troll quitte l'écran en marchant. L'original rend la main à la boucle
/// avec une remise en liste (`jmp 0x1ee1a`) à plusieurs endroits : on la rejoue.
pub(super) fn walk_off(x: &mut Cx) {
    let step = x.u8(f::SUB3);
    if step == 2 {
        if x.in_arena_default() {
            return x.add8(f::STATE, 4);
        }
        x.move_by_table_xy();
        x.anim_step();
        return x.reinsert();
    }
    if step > 2 {
        if x.anim_step() {
            set_speed(x, 0x21);
            x.add8(f::SUB3, 0xfe);
            let flip = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
        }
        return x.reinsert();
    }
    x.add8(f::SUB3, 2);
    x.set8(Obj::SPEED_INDEX, 8);
    let mut want = 0x20u8;
    let base = if x.a5_u8(0x36) != 0 { 0x55e } else { 0x50e };
    let d = x.u16(Obj::X).wrapping_sub(x.a5_u16(base));
    if (d as i16) <= 0xc0 {
        x.set8(Obj::SPEED_INDEX, 8);
        want = 0;
    }
    x.c.d[4] = x.c.d[4] & !0xff | want as u32;
    if want != x.u8(f::FLIP) {
        x.add8(f::SUB3, 4);
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
