//! Type 11, sous-états 0 et 2 : le Golem debout, piloté par les touches du quatrième joueur (`A5+3`).
//! `+6` = 0 marche (croix : X, profondeur), bouton 4 : coup, bouton 5 : saut ; `+6` = 2 : saut en arc.

use super::*;
use crate::objects::Courant;

pub(super) fn run(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => walk(x),
        2 => arc(x),
        _ => {}
    }
}

fn walk(x: &mut Cx) {
    let keys = x.a5_u8(3);
    x.c.d[0] = x.c.d[0] & !0xff | keys as u32;
    let held = x.a5_u8(0xf);
    x.c.d[1] = x.c.d[1] & !0xff | held as u32;
    let mut moved = false;
    if keys & 1 != 0 {
        let at = x.u16(X).wrapping_add(3);
        x.set16(X, at);
        x.set8(f::FLIP, 0x20);
        moved = true;
    }
    if keys & 2 != 0 {
        let at = x.u16(X).wrapping_sub(3);
        x.set16(X, at);
        x.set8(f::FLIP, 0);
        moved = true;
    }
    if keys & 8 != 0 {
        let at = x.u16(DEPTH).wrapping_add(3);
        x.set16(DEPTH, at);
        moved = true;
    }
    if keys & 4 != 0 {
        let at = x.u16(DEPTH).wrapping_sub(3);
        x.set16(DEPTH, at);
        moved = true;
    }
    x.c.d[2] = moved as u32;
    if moved {
        x.anim_step();
    }
    // les boutons sont lus dans D0, que l'animation a pu écraser
    let buttons = x.c.d[0] as u8;
    if buttons & 0x10 != 0 {
        return set_state_long(x, 0x0204_0000);
    }
    if buttons & 0x20 != 0 {
        set_state_long(x, 0x0202_0200);
    }
}

/// +6 = 2 : saut en arc (vitesse verticale 0x800).
fn arc(x: &mut Cx) {
    if x.u8(STEP) == 0 {
        x.set32(SPEED_X, 0);
        x.add8(STEP, 2);
        x.set16(SPEED_Z, 0x800);
        x.set16(GRAVITY, -0x40i16 as u16);
        return;
    }
    x.move_xz();
    if (x.u16(HEIGHT) as i16) > 0 {
        return;
    }
    x.set32(HEIGHT, 0);
    set_state_long(x, STANCE);
}
