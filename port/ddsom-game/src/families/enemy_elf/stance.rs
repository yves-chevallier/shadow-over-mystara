//! Type 12, elfe debout (`+5 = 2`) et au repos (`+5 = 10`). `+6` : 0 marche vers la cible et
//! décide d'attaquer, 2 demi-tour, 4 attente.

use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

pub(super) fn run(x: &mut Cx) {
    if grabbed(x) {
        x.set16(f::STATE, 0x408);
        x.set16(f::SUB2, 0);
        return hit::caught(x);
    }
    match x.u8(f::SUB2) {
        0 => {
            if x.u8(f::SUB3) == 0 {
                begin_walk_step(x);
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

/// 0x696b0 : l'elfe se met en marche vers sa cible.
pub(super) fn begin_walk(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    x.select_speed(&crate::objects::motion::SPEEDS_B, 0x12);
    body(x, 1);
    pick_target(x);
    lib::draw_stance(x);
    lib::draw_6b(x);
}

/// 0x696a0 : début de la marche en comptant une étape de plus.
pub(super) fn begin_walk_step(x: &mut Cx) {
    x.add8(f::SUB3, 2);
    x.set8(WALK_ANIM, 0xf0);
    x.set8(0x79, 0x1a);
    begin_walk(x);
}

/// 0x696de : marche ; se retourne, attend ou attaque.
fn walk(x: &mut Cx) {
    lib::approach_target(x);
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

/// 0x69718 : un joueur à portée ? Si oui, attaque ; sinon avance d'un pas d'animation.
fn decide(x: &mut Cx) {
    if lib::target_in_box(x, -0x20, 0x20, 0xc) {
        x.set16(f::STATE, 0x204);
        let n = rand_and(x, 1) * 2 + 4;
        x.set8(NEXT_ATTACK, n as u8);
        return x.set16(f::SUB2, 0);
    }
    if front_box(x) {
        return pick_attack(x, CLOSE_ATTACKS);
    }
    if side_box(x) {
        return pick_attack(x, FAR_ATTACKS);
    }
    if lib::guard_in_front_strict(x, -0x38, 0x38, 0xc) {
        let hint = x.u8(f::SUB2);
        x.set8(NEXT_ATTACK, hint);
        x.set16(f::SUB2, 0);
        return lib::pick_byte(x);
    }
    step(x);
    if x.u8(f::KEY_FRAME) & 1 == 0 {
        move_xy(x);
    }
    if needs_turn_around(x) {
        x.add8(f::SUB2, 2);
        x.set8(f::SUB3, 0);
    }
}

/// 0x6a6a6 : cible devant, à portée courte.
pub(super) fn front_box(x: &mut Cx) -> bool {
    x.c.d[0] = 0;
    box_attack(x, 0, 0x70, 0x10)
}

/// 0x6a698 : cible derrière ou loin devant (boîte large).
pub(super) fn side_box(x: &mut Cx) -> bool {
    box_attack(x, -0x80, 0x30, 0x10)
}

/// Boîte de détection puis décisions par masque : de même orientation, les deux (0xa4c2 puis 0xa4b4)
/// doivent accepter ; sinon seule 0xa4b4.
fn box_attack(x: &mut Cx, d0: i16, d1: u16, d2: u16) -> bool {
    let found = lib::target_in_box(x, d0, d1, d2);
    let ok = found && {
        let target = load_target(x);
        let theirs = target.u8(x.b, f::FLIP);
        x.c.d[1] = x.c.d[1] & !0xff | theirs as u32;
        let same = theirs == x.u8(f::FLIP);
        (if same {
            lib::decision_b(x)
        } else {
            true
        }) && {
            lib::decision_a(x)
        }
    };
    x.c.d[0] = ok as u32;
    ok
}

/// 0x6a706 / 0x6a70c : le coup à jouer après l'approche est tiré dans la ligne de la difficulté.
pub(super) fn pick_attack(x: &mut Cx, table: u32) {
    x.c.d[2] = 0;
    let row = (var::RANK.get(x.b) >> 1) << 5;
    set_dw(x, 0, row);
    x.c.a[4] = table + row as u32;
    let n = rand_and(x, 0x1f);
    let mv = x.data8(table + row as u32 + n);
    x.set8(f::SUB, 4);
    x.set16(f::SUB2, 0);
    x.set8(NEXT_ATTACK, mv);
    x.c.d[0] = x.c.d[0] & !0xff | mv as u32;
}

/// 0x6a6de : la cible est de l'autre côté de l'orientation de l'elfe.
pub(super) fn needs_turn_around(x: &mut Cx) -> bool {
    if x.u16(f::TARGET) == 0 {
        x.c.d[0] = 0;
        return false;
    }
    let target = load_target(x);
    let dx = x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X));
    set_dw(x, 0, dx);
    let want = if (dx as i16) < 0 { 0x20 } else { 0 };
    x.c.d[1] = want;
    let turn = want != x.u8(f::FLIP) as u32;
    x.c.d[0] = turn as u32;
    turn
}

/// `+6 = 2` : demi-tour (animation 2, puis inversion de l'orientation).
fn turn(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        return body(x, 2);
    }
    if step(x) {
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        x.set8(f::SUB2, 0);
        body(x, 1);
    }
}

/// `+6 = 4` : attente en position de garde.
fn wait(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        return body(x, 0);
    }
    let n = x.u8(AI_WAIT).wrapping_sub(1);
    x.set8(AI_WAIT, n);
    if (n as i8) < 0 {
        x.set16(f::SUB2, 0);
    }
    step(x);
}

/// Sous-état 10 : repos jusqu'à ce qu'un joueur soit en vie.
pub(super) fn rest(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set8(VULNERABLE, 1);
        return body(x, 0);
    }
    if var::PLAYERS_ALIVE.get(x.b) != 0 {
        set_state_long(x, STANCE);
        pick_target(x);
    } else {
        step(x);
    }
}
