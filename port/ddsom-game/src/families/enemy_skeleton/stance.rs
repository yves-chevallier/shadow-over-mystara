//! Type 1, squelette debout (`+5 = 2`). `+6` : 0 et 6 début de l'idée (cible, regard), 2 marche,
//! 4 demi-tour. Une idée qui dure (`+7 != 0`) marche vers la cible et décide d'attaquer.

use super::*;
use crate::objects::Obj;

pub(super) fn run(x: &mut Cx) {
    if special_hit(x, 0x5803c) {
        return;
    }
    if grabbed(x) || x.u8(CAUGHT) != 0 {
        return set_state_long(x, KNOCKED);
    }
    match x.u8(f::SUB2) {
        0 => {
            if x.u8(f::SUB3) != 0 {
                walk(x);
            } else {
                pick_target(x);
                begin_idle(x);
            }
        }
        2 => wait(x),
        4 => turn(x),
        6 => {
            if x.u8(f::SUB3) != 0 {
                walk(x);
            } else {
                x.add8(f::SUB3, 2);
                x.set8(VULNERABLE, 1);
                x.set8(AI_SHOT, 0xc);
                body(x, 0);
                x.set32(MOVE_TABLE, 0x1c74b6);
                pick_target(x);
                x.call(call::FACE_TARGET);
                x.call(call::FLASH);
                x.call(call::TURN_TO_TARGET);
            }
        }
        _ => {}
    }
    if x.a5_u8(0x35) == 0 {
        set_state_long(x, IDLE);
    }
    
    zone_shadow(x);
    if x.c.d[5] as u8 != 0 {
        x.call(call::DEAD_STEP);
    }
}

/// 0x580ae : l'idée commence, le squelette se tourne vers sa cible.
fn begin_idle(x: &mut Cx) {
    x.add8(f::SUB3, 2);
    x.set8(VULNERABLE, 1);
    x.set8(AI_SHOT, 0xb);
    body(x, 0);
    x.set32(MOVE_TABLE, 0x1c74b6);
    x.call(call::FACE_TARGET);
    x.call(call::TURN_TO_TARGET);
}

/// `+6 = 2` : attend la fin de l'animation en marchant, puis tire l'idée suivante.
fn wait(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        if x.u8(f::KEY_FRAME) != 0xff {
            move_xy(x);
            step(x);
            return;
        }
        x.add8(f::SUB3, 2);
    }
    let t = x.u8(AI_WAIT).wrapping_sub(1);
    x.set8(AI_WAIT, t);
    if t != 0 {
        return;
    }
    set_state_long(x, STANCE);
    let r = random(x) & 0xf;
    x.c.d[0] = x.c.d[0] & 0xffff_0000 | r;
    set_dw(x, 1, 0x1111);
    if 0x1111u32 >> r & 1 != 0 {
        pick_target(x);
    }
    begin_idle(x);
}

/// `+6 = 4` : demi-tour (animation 1 puis inversion de l'orientation).
fn turn(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        if x.u16(ANIM_STATE) != 0xff {
            move_xy(x);
            step(x);
            return;
        }
        x.add8(f::SUB3, 2);
        return body(x, 1);
    }
    if (x.u8(ANIM_STATE) as i8) < 0 {
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        set_state_long(x, AFTER_TURN);
        body(x, 0);
    } else {
        step(x);
    }
}

/// 0x58126 : marche vers la cible ; décide de se retourner, d'attendre ou d'attaquer.
fn walk(x: &mut Cx) {
    x.call(call::SPEED_STEP);
    let (y, z) = (x.u16(WALK_ARGS_Y), x.u16(WALK_ARGS_Z));
    set_dw(x, 1, y);
    set_dw(x, 2, z);
    x.call(call::WALK_PATH);
    let n = x.u8(AI_WALK).wrapping_sub(1);
    x.set8(AI_WALK, n);
    if (n as i8) <= 0 {
        x.call(call::FACE_TARGET);
        if x.u16(f::TARGET) == 0 {
            return;
        }
        let target = load_target(x);
        let mine = x.u16(Obj::X);
        set_dw(x, 0, mine);
        let want = if mine as i16 > target.u16(x.b, Obj::X) as i16 { 0 } else { 0x20 };
        x.c.d[1] = want;
        if want != x.u8(f::FLIP) as u32 {
            return x.set16(f::SUB2, 0x400);
        }
    }
    let n = x.u8(AI_CHOICE).wrapping_sub(1);
    x.set8(AI_CHOICE, n);
    if (n as i8) <= 0 {
        x.call(call::TURN_TO_TARGET);
        x.call(call::RAND_TARGET_CHECK);
        if !x.c.cond(7) {
            return x.set16(f::SUB2, 0x200);
        }
    }
    attack_choice(x);
}

/// 0x58194 : un joueur à portée ? Si oui, attaque ; sinon avance d'un pas d'animation.
fn attack_choice(x: &mut Cx) {
    if x.u8(SERIES) != 0 && target_in_box(x, -0x20, 0x20, 0xc) {
        set_state_long(x, DASH);
        return x.set16(COUNTER, 0);
    }
    if target_in_box(x, -0x20, 0x20, 0x10) {
        x.set8(SIDE, x.c.d[0] as u8);
        x.set16(f::STATE, 0x204);
        x.call(call::ANNOUNCE);
        return pick_move(x, CLOSE_ATTACKS, 0x204);
    }
    if front_attack(x) || far_attack(x) || target_in_box_far(x, -8, 0x68, 8) {
        return x.call(call::ANNOUNCE);
    }
    step(x);
    if (x.u8(f::KEY_FRAME) as i8) >= 0 {
        move_xy(x);
    }
}

/// 0x59254 : cible devant ; l'attaque est lancée si la décision par masque le veut.
fn front_attack(x: &mut Cx) -> bool {
    if !target_in_box(x, -8, 0x68, 8) {
        x.c.d[0] = 0;
        return false;
    }
    x.set8(SIDE, x.c.d[0] as u8);
    if !decision(x) {
        x.c.d[0] = 0;
        return false;
    }
    pick_move(x, CLOSE_ATTACKS, 0x204);
    x.c.d[0] = 1;
    true
}

/// 0x592a4 : cible plus loin (boîte large) ; même décision, attaque de loin.
fn far_attack(x: &mut Cx) -> bool {
    if !target_in_box(x, -0xa0, 0x30, 8) {
        x.c.d[0] = 0;
        return false;
    }
    x.set8(SIDE, x.c.d[0] as u8);
    if !decision(x) {
        x.c.d[0] = 0;
        return false;
    }
    pick_move(x, FAR_ATTACKS, 0x204);
    x.c.d[0] = 1;
    true
}

/// Décision par masque (0xa4c2 si cible et squelette regardent du même côté, 0xa4b4 sinon).
fn decision(x: &mut Cx) -> bool {
    if x.u16(f::TARGET) == 0 {
        return false;
    }
    let target = load_target(x);
    let same = target.u8(x.b, f::FLIP) == x.u8(f::FLIP);
    x.call(if same { 0xa4c2 } else { 0xa4b4 });
    !x.c.cond(7)
}
