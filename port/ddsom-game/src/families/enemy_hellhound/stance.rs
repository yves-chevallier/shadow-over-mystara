//! Type 6, sous-état 2 : debout. `+6` : 0 et 8 approche et choix d'un coup, 2 demi-tour, 4 attente, 6 crachat
//! de feu (le chien recule en soufflant), 10 saut de côté.

use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

pub(super) fn run(x: &mut Cx) {
    if x.u8(f::SUB2) != 10 && grabbed(x) {
        return set_state_long(x, HELD);
    }
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 | 8 => approach(x),
        2 => turn(x),
        4 => wait(x),
        6 => spit(x),
        10 => hop(x),
        _ => {}
    }
    if x.u16(Obj::Z) == 0 && var::PLAYERS_ALIVE.get(x.b) == 0 {
        set_state_long(x, IDLE);
    }
}

// --- +6 = 0 et 8 : approche -----------------------------------------------------------------------------

fn approach(x: &mut Cx) {
    let dx = x.u16(Obj::X).wrapping_sub(var::SCROLL_X.get(x.b));
    if dx >= 0x180 {
        // loin de la caméra : se tourne vers la cible puis tire son geste au hasard
        let mut flip = 0;
        if x.u16(f::TARGET) != 0 {
            let target = x.linked(f::TARGET);
            if (x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X)) as i16) < 0 {
                flip = 0x20;
            }
        }
        x.set8(f::FLIP, flip);
        let mask = x.rom32(0x61a00);
        let r = rand_and(x, 0x1f);
        let next = if mask >> r & 1 != 0 { 0x0202_0600 } else { 0x0202_0a00 };
        return set_state_long(x, next);
    }
    if target_far_in_line(x) {
        return set_state_long(x, 0x0202_0600);
    }
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        x.set8(LOOK, 0xc);
        mv(x, 2);
        set_speed(x, 0x11);
        return lib::draw_stance(x);
    }
    let n = x.u8(AI_CHARGE).wrapping_sub(1);
    x.set8(AI_CHARGE, n);
    if (n as i8) < 0 {
        lib::draw_stance(x);
        pick_target(x);
        let kind = if x.u8(f::SUB2) == 0 { 0x11 } else { 0x12 };
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

/// 0x61a9e : le chien est en garde ; un joueur à portée ? Coup immédiat, sinon pas de marche.
pub(super) fn reach(x: &mut Cx) {
    x.set8(GUARD, 2);
    reach_boxes(x);
}

/// 0x61aa4 : boîtes de portée successives (coup immédiat, attaque de face, attaque lointaine, annonce, marche).
pub(super) fn reach_boxes(x: &mut Cx) {
    if lib::target_in_box(x, -0x28, 0x28, 0xc) {
        x.set16(f::STATE, 0x0204);
        lib::pick_byte(x);
        return pick_attack(x, 0);
    }
    if front_attack(x) || far_attack(x) {
        return;
    }
    if lib::guard_in_front_strict(x, -0x40, 0x40, 0x10) {
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
    let theirs = target.u8(x.b, f::FLIP);
    x.c.d[0] = x.c.d[0] & !0xff | theirs as u32;
    let same = theirs == x.u8(f::FLIP);
    lib::decision(x, same)
}

/// 0x61b3a : un joueur devant, assez près : coup de queue ou geste.
fn front_attack(x: &mut Cx) -> bool {
    if !lib::target_in_box(x, -0x40, 0x40, 0x10) || x.u16(f::TARGET) == 0 || !gate(x) {
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

/// 0x61bb4 : un joueur loin devant : geste de la série lointaine.
fn far_attack(x: &mut Cx) -> bool {
    if !lib::target_in_box(x, -0xc0, 0x40, 0x10) || x.u16(f::TARGET) == 0 || !gate(x) {
        x.c.d[0] = 0;
        return false;
    }
    let r = rand_and(x, 2);
    if r != 0 {
        set_state_long(x, 0x0202_0800);
    } else {
        set_state_long(x, 0x0204_0000);
        lib::pick_byte(x);
        pick_attack(x, 8);
        x.c.d[0] = 1;
    }
    true
}

// --- +6 = 2 : demi-tour ------------------------------------------------------------------------------------

fn turn<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        look(x, 4);
        return mv(x, 4);
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

fn wait<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        look(x, 0);
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

// --- +6 = 6 : crachat ---------------------------------------------------------------------------------------

fn spit(x: &mut Cx) {
    match x.u8(f::SUB3) {
        4 => {
            x.add8(f::SUB3, 0xfe);
            spit_begin(x);
        }
        0 => {
            x.add8(f::SUB3, 2);
            x.set8(TIMER, 0x34);
            spit_begin(x);
        }
        2 => {
            puff(x, 7, 0, 0x101, 0x20, 4);
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if (n as i8) < 0 {
                set_state_long(x, STANCE);
            } else {
                x.anim_step();
                x.move_by_table_xy();
            }
        }
        _ => {}
    }
}

/// 0x61d08 : début du crachat : le chien recule en soufflant.
fn spit_begin<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    x.set8(VULNERABLE, 1);
    x.set8(LOOK, 3);
    x.set8(STRIKE_KIND, 0x2c);
    arrival::set_angle_facing(x, 0x18, 8);
    mv(x, 3);
    set_speed(x, 0x25);
}

// --- +6 = 10 : saut ---------------------------------------------------------------------------------------------

fn hop(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set8(VULNERABLE, 1);
            look(x, 6);
            x.set8(STRIKE_KIND, 0x2b);
            x.set16(Obj::VX, -0x380i16 as u16);
            if x.u8(f::FLIP) != 0 {
                negate(x, Obj::VX);
            }
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, 0x4c0);
            x.set16(Obj::AZ, -0x38i16 as u16);
            mv(x, 5);
        }
        2 => wait_key(x),
        4 => land(x),
        6 => {
            if x.anim_step() {
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}
