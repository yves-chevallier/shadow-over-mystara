//! Type 11, sous-états 0 et 2 : le Golem debout, piloté par les touches du quatrième joueur (`A5+3`).
//! `+6` = 0 marche (croix : X, profondeur), bouton 4 : coup, bouton 5 : saut ; `+6` = 2 : saut en arc.

use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => walk(x),
        2 => arc(x),
        _ => {}
    }
}

fn walk<R: Bench>(x: &mut Cx<'_, R>) {
    let keys = x.a5_u8(3);
    let mut moved = false;
    if keys & 1 != 0 {
        let at = x.u16(Obj::X).wrapping_add(3);
        x.set16(Obj::X, at);
        x.set8(f::FLIP, 0x20);
        moved = true;
    }
    if keys & 2 != 0 {
        let at = x.u16(Obj::X).wrapping_sub(3);
        x.set16(Obj::X, at);
        x.set8(f::FLIP, 0);
        moved = true;
    }
    if keys & 8 != 0 {
        let at = x.u16(Obj::Y).wrapping_add(3);
        x.set16(Obj::Y, at);
        moved = true;
    }
    if keys & 4 != 0 {
        let at = x.u16(Obj::Y).wrapping_sub(3);
        x.set16(Obj::Y, at);
        moved = true;
    }
    // Glitch d'origine : les boutons sont relus dans D0, que le pas d'animation (0x1a56) écrase par l'octet `+0x32`
    // quand l'image en cours se termine.
    let mut buttons = keys;
    // Glitch d'origine : le drapeau « a bougé » reste dans D2 (`Leftovers`).
    *x.c.d2() = moved as u32;
    if moved {
        let (ends, state) = (x.u16(ANIM_TIME) == 1, x.u8(ANIM_END));
        x.anim_step();
        if ends {
            buttons = state;
        }
    }
    if buttons & 0x10 != 0 {
        return set_state_long(x, 0x0204_0000);
    }
    if buttons & 0x20 != 0 {
        set_state_long(x, 0x0202_0200);
    }
}

/// +6 = 2 : saut en arc (vitesse verticale 0x800).
fn arc<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.set32(Obj::VX, 0);
        x.add8(f::SUB3, 2);
        x.set16(Obj::VZ, 0x800);
        x.set16(Obj::AZ, -0x40i16 as u16);
        return;
    }
    x.move_xz();
    if (x.u16(Obj::Z) as i16) > 0 {
        return;
    }
    x.set32(Obj::Z, 0);
    set_state_long(x, STANCE);
}
