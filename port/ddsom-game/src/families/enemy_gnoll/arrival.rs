//! Type 13, sous-état 0 : arrivée selon la variante `+3` (valeurs paires). 0 marche d'entrée, 2 et 6 chute depuis le
//! haut, 4 saut de côté, 8 arrivée armée (pose la hache), 10 course d'entrée armée suivie de la saisie.

use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

pub(super) fn run(x: &mut Cx) {
    match x.u8(Obj::VARIANT) {
        0 => walk_in(x),
        2 => drop_in(x, true),
        4 => side_jump(x),
        6 => drop_in(x, false),
        8 => armed_in(x),
        10 => armed_run_in(x),
        _ => {}
    }
}

/// Variante 0 : marche vers l'écran pendant `+0xa2` images.
fn walk_in<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) != 0 {
        x.move_by_table_xy();
        x.anim_step();
        let n = x.u8(TIMER).wrapping_sub(1);
        x.set8(TIMER, n);
        if (n as i8) < 0 {
            set_state_long(x, STANCE);
        }
        return;
    }
    x.add8(f::SUB2, 2);
    x.set8(TIMER, 0x14);
    let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
    x.set8(Obj::SPEED_INDEX, angle);
    set_speed(x, 0x12);
    mv(x, 1);
}

/// Variantes 2 et 6 : saut depuis le haut. `gravity` : la variante 2 attend l'atterrissage (`+0xc`) avant de se
/// relever, la variante 6 se relève tout de suite.
fn drop_in<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>, gravity: bool) {
    match x.u8(f::SUB2) {
        0 => {
            x.set16(Obj::VZ, 0);
            x.set16(Obj::AZ, -0x40i16 as u16);
            x.add8(f::SUB2, 2);
            mv(x, 3);
        }
        2 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
            }
        }
        4 => {
            if gravity {
                x.move_z();
                if (x.u16(Obj::Z) as i16) > 0 {
                    return;
                }
            }
            x.add8(f::SUB2, 2);
            x.set32(Obj::Z, 0);
            mv(x, 5);
        }
        6 => {
            if x.anim_step() {
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}

/// Variante 4 : bond par le côté (vitesse horizontale −0x280), coup en l'air, retombée.
fn side_jump(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set16(Obj::VZ, 0);
            x.set16(Obj::AZ, -0x38i16 as u16);
            x.set16(Obj::VX, -0x280i16 as u16);
            x.set16(Obj::AX, 0);
            if x.u8(f::FLIP) != 0 {
                negate(x, Obj::VX);
            }
            mv(x, 3);
        }
        2 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
            }
        }
        4 => {
            x.add8(f::SUB2, 2);
            strike(x, 1);
        }
        6 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) <= 0 {
                x.add8(f::SUB2, 2);
                x.set32(Obj::Z, 0);
                return mv(x, 5);
            }
            if (x.u16(Obj::Z) as i16) > 0x30 {
                return;
            }
            x.anim_step();
        }
        8 => {
            if x.anim_step() {
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}

/// L'arme se pose (clé 9) ou se reprend (clé 0xb) pendant les animations d'arrivée armée.
fn weapon_key_drop<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    if x.u8(f::KEY_FRAME) == 9 {
        x.set8(f::KEY_FRAME, 0);
        x.set8(WEAPON, 0);
        lib::eshl_12(x);
    }
}

/// Variante 8 : le Gnoll arrive en courant avec sa hache, la lâche (animation 9) puis la reprend.
fn armed_in(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(WEAPON, 1);
            x.set8(ARMED, 1);
            x.set8(VULNERABLE, 0);
            x.set8(TIMER, 0x1a);
            set_speed(x, 0x17);
            let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            x.set8(Obj::SPEED_INDEX, angle);
            strike(x, 4);
        }
        2 => armed_run(x),
        4 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
                return mv(x, 9);
            }
            weapon_key_drop(x);
        }
        6 => {
            if x.anim_step() {
                return set_state_long(x, STANCE);
            }
            if x.u8(f::KEY_FRAME) == 0xb {
                lib::eshl_16(x, 1);
                x.set8(f::KEY_FRAME, 0);
                x.set8(ARMED, 0);
            }
        }
        _ => {}
    }
}

/// 0x6ae42 : course armée pendant `+0xa2` images, puis animation 5.
fn armed_run(x: &mut Cx) {
    x.move_by_table_xy();
    x.anim_step();
    let n = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, n);
    if (n as i8) >= 0 {
        return;
    }
    x.add8(f::SUB2, 2);
    strike(x, 5);
}

/// Variante 10 : le Gnoll entre par le bord de l'écran avec sa hache ; saisi aussitôt la course finie.
fn armed_run_in(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(WEAPON, 1);
            x.set8(ARMED, 1);
            x.set8(VULNERABLE, 0);
            x.set8(TIMER, 0x1a);
            set_speed(x, 0x17);
            let (angle, shift) = if x.u8(f::FLIP) != 0 { (8, -0x20i16) } else { (0x18, 0x1a0) };
            x.set8(Obj::SPEED_INDEX, angle);
            let cam = if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA_ALT_X.get(x.b) } else { var::SCROLL_X.get(x.b) };
            x.set16(Obj::X, cam.wrapping_add(shift as u16));
            strike(x, 4);
        }
        2 => armed_run(x),
        4 => {
            if x.anim_step() {
                x.set16(f::STATE, 0x0408);
                x.set16(f::SUB2, 0);
                return grab(x);
            }
            weapon_key_drop(x);
        }
        _ => {}
    }
}
