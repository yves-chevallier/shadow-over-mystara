//! Type 3, sous-états 2 (debout : approche, marche), 8 (repos), 10 (attente) et 12 (demi-tour).

use super::shoot::start_shot;
use super::*;
use crate::objects::Obj;
use crate::objects::Courant;

pub(super) fn run(x: &mut Cx) {
    if x.u8(EXPIRED) != 0 || grabbed(x) {
        set_state_long(x, HELD);
        return hit::grab(x);
    }
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => stance_begin(x),
        2 => advance(x),
        4 => strafe(x),
        _ => {}
    }
}

/// 0x5c17e : premier pas debout : choisit une cible et s'oriente vers elle.
pub(super) fn stance_begin(x: &mut Cx) {
    x.add8(f::SUB2, 2);
    if !pick_target(x) {
        set_state_long(x, IDLE);
        return idle_begin(x);
    }
    lib::draw_6b(x);
    x.set8(SHOT_KIND, 3);
    lib::approach_target(x);
    set_speed(x, 0xd);
    mv(x, 2);
}

/// 0x5c2de : début de l'attente.
fn idle_begin(x: &mut Cx) {
    x.add8(f::SUB2, 2);
    mv(x, 0);
}

/// +6 = 2 : marche vers la cible ; tire ou esquive quand elle est à portée.
fn advance(x: &mut Cx) {
    if !x.in_arena_default() {
        x.set8(SHOT_KIND, 5);
        x.add8(f::SUB2, 2);
        x.set8(TIMER, 0x20);
        return strafe(x);
    }
    let n = x.u8(AI_CHARGE).wrapping_sub(1);
    x.set8(AI_CHARGE, n);
    if (n as i8) < 0 {
        let mut waiting = true;
        if pick_target(x) {
            lib::draw_stance(x);
            let m = x.u8(AI_WANDER).wrapping_sub(1);
            x.set8(AI_WANDER, m);
            if (m as i8) >= 0 {
                waiting = false;
            }
        }
        if waiting {
            if lib::maybe_draw_6e(x) {
                set_state_long(x, IDLE);
                return check_facing(x);
            }
        }
    }
    lib::approach_target(x);
    x.move_by_table_xy();
    x.anim_step();
    if point_blank(x) {
        set_state_long(x, 0x0204_0202);
        return start_shot(x);
    }
    decide(x)
}

/// 0x5c22c : un joueur à portée de tir ? Alors tir ou esquive selon la boîte qui l'a vu.
pub(super) fn decide(x: &mut Cx) {
    if in_front(x) || on_line(x) {
        // la cible trouvée par la boîte est dans A1 ; D5 dit laquelle (0 devant, 2 sur la ligne)
        let found = Obj(x.c.a[1]);
        let facing = x.u8(f::FLIP);
        x.c.d[0] = x.c.d[0] & !0xff | facing as u32;
        let same_side = facing == found.u8(x.b, f::FLIP);
        if lib::decision(x, same_side) {
            set_state_long(x, 0x0204_0000);
            let d5 = x.c.d[5] as u8;
            x.set8(f::SUB2, d5);
            return check_facing(x);
        }
        if lib::guard_in_front_strict(x, -0x38, 0x38, 0x10) {
            x.set8(f::SUB, 0xa);
            x.set16(f::SUB2, 0);
        }
    }
    check_facing(x)
}

/// 0x5c27a : si le Bow Gnoll tourne le dos à sa cible, demi-tour (état à reprendre mémorisé en `+0xa0`).
pub(super) fn check_facing(x: &mut Cx) {
    if x.u16(f::TARGET) == 0 {
        return;
    }
    let target = load_target(x);
    let dx = x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X));
    let want = if (dx as i16) < 0 { 0x20 } else { 0 };
    x.c.d[1] = want;
    if want as u8 == x.u8(f::FLIP) {
        return;
    }
    let state = x.u32(f::STATE);
    x.set32(RESUME_STATE, state);
    x.set8(RESUME_SUB2, 0);
    x.set8(f::SUB, 0xc);
    x.set16(f::SUB2, 0);
}

/// +6 = 4 : marche latérale pendant `+0xa4` images, puis retour à l'approche.
fn strafe(x: &mut Cx) {
    let n = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, n);
    if (n as i8) < 0 {
        x.set8(f::SUB2, 2);
        x.set8(SHOT_KIND, 3);
        return advance(x);
    }
    lib::approach_target(x);
    x.move_by_table_xy();
    x.anim_step();
}

// --- sous-état 8 : repos ------------------------------------------------------------------------------

pub(super) fn rest(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => {
            set_state_long(x, IDLE);
            x.set8(AI_WAIT, 0x1e);
            idle_begin(x);
        }
        2 => side_step(x, true),
        4 => side_step(x, false),
        6 => {
            set_state_long(x, STANCE);
            decide(x);
        }
        _ => {}
    }
}

/// +6 = 2 (en X, regarde vers la cible) et 4 (en profondeur) : pas de côté pendant `+0x6b` images.
fn side_step(x: &mut Cx, along_x: bool) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(AI_WANDER, 0x18);
        let target = load_target(x);
        if along_x {
            x.set8(Obj::SPEED_INDEX, 0x18);
            let d = x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X));
            x.c.d[1] = x.c.d[1] & 0xffff_0000 | d as u32;
            if (d as i16) >= 0 {
                x.set8(Obj::SPEED_INDEX, 8);
            }
        } else {
            x.set8(Obj::SPEED_INDEX, 0xf);
            let d = x.u16(Obj::Y).wrapping_sub(target.u16(x.b, Obj::Y));
            x.c.d[1] = x.c.d[1] & 0xffff_0000 | d as u32;
            if (d as i16) >= 0 {
                x.set8(Obj::SPEED_INDEX, 0);
            }
        }
        return mv(x, 2);
    }
    let n = x.u8(AI_WANDER).wrapping_sub(1);
    x.set8(AI_WANDER, n);
    if (n as i8) < 0 {
        set_state_long(x, STANCE);
    }
    x.move_by_table_xy();
    x.anim_step();
}

// --- sous-état 10 : attente ---------------------------------------------------------------------------

pub(super) fn idle(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        return idle_begin(x);
    }
    let n = x.u8(AI_WAIT).wrapping_sub(1);
    x.set8(AI_WAIT, n);
    if (n as i8) < 0 {
        x.set8(f::SUB, 2);
        x.set16(f::SUB2, 0);
    }
    x.anim_step();
}

// --- sous-état 12 : demi-tour -------------------------------------------------------------------------

pub(super) fn turn_around(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            mv(x, 3);
        }
        2 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                let state = x.u32(RESUME_STATE);
                x.set32(f::STATE, state);
                mv(x, 0);
            }
        }
        _ => {}
    }
}
