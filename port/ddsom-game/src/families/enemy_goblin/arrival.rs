//! Type 15, sous-état 0 : arrivée selon la variante `+3` (valeurs paires). 0 marche d'entrée, 2 chute depuis le haut,
//! 4 chute déjà amorcée, 6 course depuis le bord de l'écran suivie de séries de coups.

use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

pub(super) fn run(x: &mut Cx) {
    match x.u8(Obj::VARIANT) {
        0 => walk_in(x),
        2 => drop_in(x, true),
        4 => drop_in(x, false),
        6 => run_in(x),
        _ => {}
    }
}

/// Variante 0 : marche vers l'écran pendant `+0xa2` images.
fn walk_in(x: &mut Cx) {
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

/// Variantes 2 et 4 : saut depuis le haut. `gravity` : la variante 2 attend l'atterrissage (`+0xc`) avant de se
/// relever, la variante 4 se relève tout de suite.
fn drop_in(x: &mut Cx, gravity: bool) {
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

/// Variante 6 : le Goblin entre en courant par le bord (position posée d'après la caméra), puis frappe en série
/// jusqu'à être saisi.
fn run_in(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(TIMER, 0x28);
            x.set8(NO_ZONE, 1);
            let (angle, shift) = if x.u8(f::FLIP) != 0 { (8, -0x20i16) } else { (0x18, 0x1a0) };
            x.set8(Obj::SPEED_INDEX, angle);
            let cam = if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA_ALT_X.get(x.b) } else { var::SCROLL_X.get(x.b) };
            x.set16(Obj::X, cam.wrapping_add(shift as u16));
            set_speed(x, 0x12);
            mv(x, 1);
        }
        2 => {
            x.anim_step();
            if x.u8(f::KEY_FRAME) == 0 {
                x.move_by_table_xy();
            }
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if n != 0 {
                return;
            }
            x.add8(f::SUB2, 2);
            let t = row_byte(x, SERIES_TABLE);
            x.set8(TIMER, t);
            strike(x, 0);
        }
        4 => {
            if !x.anim_step() {
                if x.u8(f::KEY_FRAME) != 0 {
                    x.set8(f::KEY_FRAME, 0);
                    crate::families::eshl_creators::shot_0(&mut crate::families::effects_creators::restes::Pur, x.b, x.o);
                }
                return;
            }
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if n != 0 {
                return strike(x, 0);
            }
            set_state_long(x, HELD);
            grab(x);
        }
        _ => {}
    }
}
