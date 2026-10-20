//! Type 14, sous-état 0 : arrivée selon la variante `+3` (valeurs paires). 0 marche d'entrée, 2 chute depuis le haut,
//! 4 chute déjà amorcée.

use super::*;

pub(super) fn run(x: &mut Cx) {
    x.c.d[0] = x.u8(VARIANT) as u32;
    match x.u8(VARIANT) {
        0 => walk_in(x),
        2 => drop_in(x, true),
        4 => drop_in(x, false),
        _ => {}
    }
}

/// Variante 0 : marche vers l'écran pendant `+0xa2` images.
fn walk_in(x: &mut Cx) {
    if x.u8(f::SUB2) != 0 {
        x.call(sub::MOVE_TABLE_XY);
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
    x.set8(ANGLE, angle);
    set_speed(x, 0x12);
    mv(x, 1);
}

/// Variantes 2 et 4 : saut depuis le haut. `gravity` : la variante 2 attend l'atterrissage (`+0xc` négatif) avant de
/// se relever, la variante 4 se relève tout de suite.
fn drop_in(x: &mut Cx, gravity: bool) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => {
            x.set16(SPEED_Z, -0x600i16 as u16);
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
                x.call(sub::MOVE_Z);
                if (x.u16(HEIGHT) as i16) >= 0 {
                    return;
                }
            }
            x.add8(f::SUB2, 2);
            x.set32(HEIGHT, 0);
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
