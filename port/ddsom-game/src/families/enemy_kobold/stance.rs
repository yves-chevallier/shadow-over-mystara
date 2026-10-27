//! Type 0, sous-état 2 : debout. `+6` : 0 et 8 approche et choix d'un geste, 2 demi-tour, 4 attente, 6 course.

use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

pub(super) fn run(x: &mut Cx) {
    if x.a5_u8(0x35) == 0 {
        return set_state_long(x, IDLE);
    }
    if matches!(x.u8(Obj::VARIANT), 0xa | 0xb) || grabbed(x) {
        return set_state_long(x, HELD);
    }
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 | 8 => approach(x),
        2 => turn(x),
        4 => wait(x),
        6 => run_up(x),
        _ => {}
    }
    if x.a5_u8(0x35) == 0 {
        set_state_long(x, IDLE);
    }
}

// --- +6 = 0 et 8 : approche -----------------------------------------------------------------------------

fn approach(x: &mut Cx) {
    if target_far_in_line(x) {
        return set_state_long(x, 0x0202_0600);
    }
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        x.set8(LOOK, 0xc);
        mv(x, 0xc);
        set_speed(x, 0xb);
        return lib::draw_stance(x);
    }
    let n = x.u8(AI_CHARGE).wrapping_sub(1);
    x.set8(AI_CHARGE, n);
    if (n as i8) < 0 {
        lib::draw_stance(x);
        pick_target(x);
        let kind = if x.u8(f::SUB2) == 0 { 0xe } else { 0xf };
        x.set8(SHOT_KIND, kind);
        lib::approach_target(x);
        let w = x.u8(AI_WANDER).wrapping_sub(1);
        x.set8(AI_WANDER, w);
        if w == 0 {
            lib::draw_6b(x);
            if lib::maybe_draw_6e(x) {
                return x.set16(f::SUB2, 0x400);
            }
        }
    }
    reach(x);
}

/// 0x555d4 : en garde ; un joueur tout près ? Coup immédiat, sinon boîtes de portée, sinon pas de marche.
pub(super) fn reach(x: &mut Cx) {
    x.set8(GUARD, 2);
    reach_boxes(x);
}

/// 0x555da : boîtes de portée successives.
pub(super) fn reach_boxes(x: &mut Cx) {
    if lib::target_in_box(x, -0x28, 0x18, 4) {
        set_state_long(x, 0x0204_0200);
        return lib::pick_byte(x);
    }
    if front_attack(x) || far_attack(x) {
        return;
    }
    if lib::guard_in_front_strict(x, -0x30, 0x30, 0x20) {
        return lib::pick_byte(x);
    }
    x.anim_step();
    x.move_by_table_xy();
    if x.u16(f::TARGET) == 0 {
        return;
    }
    let target = x.linked(f::TARGET);
    let side = if (x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X)) as i16) < 0 { 0x20 } else { 0 };
    if side != x.u8(f::FLIP) {
        x.set32(RESUME_STATE, STANCE);
        x.set8(RESUME_SUB2, 0);
        set_state_long(x, 0x0202_0200);
    }
}

/// Décision d'attaque selon l'orientation relative de la cible (0xa4c2 de même sens, 0xa4b4 sinon).
fn gate(x: &mut Cx) -> bool {
    let target = x.linked(f::TARGET);
    x.c.a[1] = target.0 | 0xff00_0000;
    let theirs = target.u8(x.b, f::FLIP);
    x.c.d[0] = x.c.d[0] & !0xff | theirs as u32;
    let same = theirs == x.u8(f::FLIP);
    lib::decision(x, same)
}

/// 0x5566e : un joueur devant, tout près : coup de la série courte ou nouveau pas.
fn front_attack(x: &mut Cx) -> bool {
    if !lib::target_in_box(x, -0x40, 0x20, 2) || x.u16(f::TARGET) == 0 || !gate(x) {
        x.c.d[0] = 0;
        return false;
    }
    let r = random(x) & 0xf;
    x.c.d[0] = r;
    if (0xffc0u32 >> r) & 1 != 0 {
        set_state_long(x, 0x0202_0800);
    } else {
        set_state_long(x, 0x0204_0000);
        lib::pick_byte(x);
        pick_attack(x, 0);
        x.c.d[0] = 1;
    }
    true
}

/// 0x556e8 : un joueur plus loin devant : geste de la série lointaine.
fn far_attack(x: &mut Cx) -> bool {
    if !lib::target_in_box(x, -0xb0, 0x20, 2) || x.u16(f::TARGET) == 0 || !gate(x) {
        x.c.d[0] = 0;
        return false;
    }
    let r = rand_and(x, 2);
    if r != 0 {
        set_state_long(x, 0x0202_0800);
    } else {
        set_state_long(x, 0x0204_0000);
        lib::pick_byte(x);
        pick_attack(x, 0x14);
        x.c.d[0] = 1;
    }
    true
}

// --- +6 = 2 : demi-tour --------------------------------------------------------------------------------------

fn turn(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        look(x, 5);
        return mv(x, 5);
    }
    if !x.anim_step() {
        return;
    }
    let s = x.u32(RESUME_STATE);
    x.set32(f::STATE, s);
    let sub2 = x.u8(RESUME_SUB2);
    x.set8(f::SUB2, sub2);
    let flip = x.u8(f::FLIP) ^ 0x20;
    x.set8(f::FLIP, flip);
    mv(x, 0);
}

// --- +6 = 4 : attente -----------------------------------------------------------------------------------------

fn wait(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        x.set8(LOOK, 0);
        return mv(x, 0);
    }
    let n = x.u8(AI_WAIT).wrapping_sub(1);
    x.set8(AI_WAIT, n);
    if (n as i8) < 0 {
        return set_state_long(x, STANCE);
    }
    if must_turn(x) {
        x.set32(RESUME_STATE, STANCE);
        x.set8(RESUME_SUB2, 4);
        set_state_long(x, 0x0202_0200);
    }
    x.anim_step();
}

// --- +6 = 6 : course ------------------------------------------------------------------------------------------

fn run_up(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        4 => {
            x.add8(f::SUB3, 0xfe);
            run_begin(x);
        }
        0 => {
            x.add8(f::SUB3, 2);
            x.set16(TIMER, 0x28);
            lib::draw_stance(x);
            run_begin(x);
        }
        2 => {
            let n = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, n);
            if (n as i16) < 0 {
                return set_state_long(x, STANCE);
            }
            let n = x.u8(AI_CHARGE).wrapping_sub(1);
            x.set8(AI_CHARGE, n);
            if (n as i8) < 0 {
                lib::draw_stance(x);
                pick_target(x);
                x.set8(SHOT_KIND, 0xe);
                lib::approach_target(x);
            }
            if must_turn(x) {
                x.set32(RESUME_STATE, 0x0202_0004);
                let sub2 = x.u8(f::SUB2);
                x.set8(RESUME_SUB2, sub2);
                set_state_long(x, 0x0202_0200);
            }
            x.anim_step();
            x.move_by_table_xy();
        }
        _ => {}
    }
}

/// 0x5583c : début de la course.
fn run_begin(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    x.set8(LOOK, 0x16);
    mv(x, 0x16);
    set_speed(x, 0x17);
}
