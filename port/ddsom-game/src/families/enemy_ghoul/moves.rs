//! Type 5, déplacements de repli (`+5 = 6`, `+6` : 0 et 2 pas de côté, 4 et 6 bond, 8 coups en deux temps,
//! 10 sortie d'écran) et demi-tours enchaînés sur une attaque (`+5 = 8`, `+6` : 0 à 6).

use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

// --- sous-état 6 --------------------------------------------------------------------------------

pub(super) fn run_b(x: &mut Cx) {
    if special_hit(x, 0x605d8) {
        return;
    }
    match x.u8(f::SUB2) {
        0 => side_step(x, false),
        2 => side_step(x, true),
        4 | 6 => hop(x),
        8 => two_strikes(x),
        10 => exit_screen(x),
        _ => {}
    }
}

/// 0 : pas en avant (animation 0 ou coup 3) ; 2 : pas en profondeur vers un côté tiré au hasard.
fn side_step(x: &mut Cx, depth: bool) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        look(x, 6);
        x.set8(VULNERABLE, 1);
        if !depth {
            x.set8(TIMER, 0x10);
            if rand_and(x, 1) != 0 {
                return body(x, 0);
            }
            return strike(x, 3);
        }
        x.set8(TIMER, 0xc);
        load_target(x);
        x.set8(Obj::SPEED_INDEX, 0);
        if rand_and(x, 1) != 0 {
            x.set8(Obj::SPEED_INDEX, 0x10);
        }
        set_speed(x, 0xd);
        return body(x, 2);
    }
    let t = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, t);
    if t == 0 {
        set_state_long(x, STANCE);
    }
    if depth {
        move_xy(x);
    }
    step(x);
}

/// 4 et 6 : petit bond en arrière puis retombée (animation 3, 4, 5).
fn hop(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            look(x, 6);
            x.set8(TIMER, 4);
            x.set8(VULNERABLE, 1);
            body(x, 3);
        }
        2 => {
            let t = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, t);
            if t != 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set16(Obj::VX, -0x200i16 as u16);
            if x.u8(f::FLIP) != 0 {
                negate(x, Obj::VX);
            }
            x.set16(Obj::VZ, 0x280);
            x.set16(Obj::AZ, -0x48i16 as u16);
            x.set16(Obj::AX, 0);
            body(x, 4);
        }
        4 => {
            if x.move_xz().gt() {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set32(Obj::Z, 0);
            body(x, 5);
        }
        6 => {
            if step(x) {
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}

/// 8 : approche, puis deux coups de griffe de suite.
fn two_strikes(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            look(x, 6);
            x.set8(VULNERABLE, 1);
            x.set8(TIMER, 0xc);
            load_target(x);
            x.set8(Obj::SPEED_INDEX, 0);
            if rand_and(x, 1) != 0 {
                x.set8(Obj::SPEED_INDEX, 0x10);
            }
            set_speed(x, 0xd);
            body(x, 2);
        }
        2 => {
            let t = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, t);
            if t == 0 {
                x.add8(f::SUB3, 2);
                x.set8(STRIKE_KIND, 0x11);
                return strike(x, 0);
            }
            move_xy(x);
            step(x);
        }
        4 => {
            if step(x) {
                x.add8(f::SUB3, 2);
                x.set8(STRIKE_KIND, 0x12);
                strike(x, 1);
            }
        }
        6 => {
            if step(x) {
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}

/// 10 : le ghoul quitte l'écran en courant vers le bord le plus proche.
fn exit_screen(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.set16(Obj::VX, -0x230i16 as u16);
        x.set8(f::FLIP, 0);
        let d = x.a5_u16(0x506).wrapping_sub(x.u16(Obj::X));
        if (d as i16) >= 0xc0 {
            negate(x, Obj::VX);
            x.set8(f::FLIP, 0x20);
        }
        x.add8(f::SUB3, 2);
        return body(x, 2);
    }
    if (x.u8(f::KEY_FRAME) as i8) >= 0 {
        x.move_x();
    }
    step(x);
    let d = x.u16(Obj::X).wrapping_sub(x.a5_u16(0x506)).wrapping_add(0x10);
    if d > 0x1a0 {
        detach_from_host(x);
        set_state_long(x, st::DEAD_LONG);
    }
}

// --- sous-état 8 --------------------------------------------------------------------------------

pub(super) fn run_c(x: &mut Cx) {
    if special_hit(x, 0x60384) {
        return;
    }
    match x.u8(f::SUB2) {
        0 => turn_then(x, Axis::None),
        2 => turn_then(x, Axis::Horizontal),
        4 => turn_then(x, Axis::Depth),
        6 => wake(x),
        _ => {}
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Axis {
    None,
    Horizontal,
    Depth,
}

/// Se retourne vers la cible si besoin, puis marche selon `axis` (rien, vers elle en X, en profondeur).
fn turn_then(x: &mut Cx, axis: Axis) {
    match (x.u8(f::SUB3), axis) {
        (0, Axis::None) => {
            x.add8(f::SUB3, 2);
            look(x, 8);
            x.set8(VULNERABLE, 1);
            x.set8(TIMER, 0x10);
            if needs_turn_around(x) {
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                return body(x, 1);
            }
            x.add8(f::SUB3, 2);
            body(x, 0);
        }
        (2, Axis::None) => {
            if step(x) {
                x.add8(f::SUB3, 2);
                body(x, 0);
            }
        }
        (4, Axis::None) => {
            let t = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, t);
            if t == 0 {
                set_state_long(x, STANCE);
            }
            step(x);
        }
        (0, _) => {
            x.add8(f::SUB3, 2);
            look(x, 8);
            x.set8(VULNERABLE, 1);
            x.set8(TIMER, if axis == Axis::Horizontal { 0x18 } else { 0x10 });
            if needs_turn_around(x) {
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                return body(x, 1);
            }
            walk_toward(x, axis);
        }
        (2, _) => {
            if step(x) {
                walk_toward(x, axis);
            }
        }
        (4, _) => {
            let t = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, t);
            if t == 0 {
                set_state_long(x, STANCE);
            }
            move_xy(x);
            step(x);
        }
        _ => {}
    }
}

/// 0x60472 / 0x60512 : marche vers la cible, en X ou en profondeur.
fn walk_toward(x: &mut Cx, axis: Axis) {
    x.add8(f::SUB3, 2);
    let target = load_target(x);
    if axis == Axis::Horizontal {
        x.set8(Obj::SPEED_INDEX, 8);
        let dx = x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X));
        set_dw(x, 0, dx);
        if (dx as i16) < 0 {
            x.set8(Obj::SPEED_INDEX, 0x18);
        }
    } else {
        x.set8(Obj::SPEED_INDEX, 0);
        let dy = x.u16(Obj::Y).wrapping_sub(target.u16(x.b, Obj::Y));
        set_dw(x, 0, dy);
        if (dy as i16) < 0 {
            x.set8(Obj::SPEED_INDEX, 0x10);
        }
    }
    set_speed(x, 0xd);
    body(x, 2);
}

/// 6 : se retourne si besoin puis regarde si un joueur est à portée de coup.
fn wake(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        if needs_turn_around(x) {
            let flip = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
            return body(x, 1);
        }
    } else if !step(x) {
        return;
    }
    look(x, 8);
    x.set8(VULNERABLE, 1);
    body(x, 0);
    if box_attack(x, 0, 0x50, 8, CLOSE_ATTACKS, 0) {
        return;
    }
    if !box_attack(x, -0x80, 0x30, 8, FAR_ATTACKS, 0xa) {
        set_state_long(x, STANCE);
    }
}
