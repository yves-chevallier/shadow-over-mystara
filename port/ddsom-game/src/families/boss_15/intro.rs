//! Boss 15 / 20, groupe `+5` = 0 : arrivée. Le dragon tombe du haut de l'écran, à 0xc0 pixels à droite de la caméra,
//! s'immobilise à l'altitude 0x40, puis le combat commence.
//!
//! | `+6` | Rôle |
//! |---|---|
//! | 0 | placement au-dessus de l'écran, délai (0xa0 images ; 0x10 pour le petit dragon) |
//! | 2 | attend, puis commence à descendre |
//! | 4 | descend jusqu'à l'altitude 0x40 |

use crate::families::cx::Banc;
use super::*;
use crate::objects::Courant;

pub(super) fn run(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => place(x),
        2 => wait_then_fall(x),
        4 => fall(x),
        _ => {}
    }
}

fn at_camera<R: Banc>(x: &mut Cx<'_, R>) {
    let cx = var::SCROLL_X.get(x.b).wrapping_add(0xc0);
    x.set16(0x8, cx);
}

fn place(x: &mut Cx) {
    x.add8(f::SUB2, 2);
    at_camera(x);
    x.set16(0x10, 0x1a0);
    x.set16(0xc, 0xc0);
    x.set32(0x14, 0);
    x.set32(0x1c, 0);
    x.set16(0x18, (-0x400i16) as u16);
    x.set16(0x1a, 0);
    let wait = if x.u8(TYPE) == TYPE_SMALL { 0x10 } else { 0xa0 };
    x.set16(TIMER, wait);
    rest(x);
}

fn wait_then_fall(x: &mut Cx) {
    if (x.u16(TIMER) as i16) > 0 {
        let t = x.u16(TIMER) - 1;
        x.set16(TIMER, t);
        return;
    }
    at_camera(x);
    x.move_z();
    if (x.u16(0xc) as i16) > 0x40 {
        return;
    }
    x.add8(f::SUB2, 2);
    x.set16(0x1a, 0x80);
    fall(x);
}

fn fall(x: &mut Cx) {
    at_camera(x);
    x.move_z();
    if (x.u16(0xc) as i16) < 0x40 {
        return;
    }
    x.set16(0xc, 0x40);
    x.add8(f::SUB, 2);
    x.set16(f::SUB2, 0);
    if x.u8(TYPE) != TYPE_BIG {
        lib::sound(x, 0xe13f0);
    }
    x.set8(0, 1);
    lib::set_immunities(x);
    rest(x);
    x.call(0xb3fb0);
    // l'arrivée finie, le combat commence dans la même image
    super::fight::run(x);
}
