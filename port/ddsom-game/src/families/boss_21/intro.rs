//! Boss 21, groupe `+5` = 0 : arrivée en scène.
//!
//! | `+6` | Rôle |
//! |---|---|
//! | 0 | arrivée par la droite de l'écran : approche, bond, atterrissage (`+7` = 0 à 14) |
//! | 2 | entrée en combat : tombe du haut de l'écran (`+7` = 0 à 4) |

use super::decide;
use super::*;
use crate::objects::Courant;

pub(super) fn run(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => arrival(x),
        2 => entrance(x),
        _ => {}
    }
}

fn arrival(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(0, 2);
            x.set_bit(0x1, 1, false);
            x.set8(f::FLIP, 0);
            let cx = x.a5_u16(var::SCROLL_X).wrapping_add(0x140);
            x.set16(0x8, cx);
            let host = x.linked(f::HOST);
            let z = host.u16(x.b, 0xc).wrapping_sub(0x10);
            x.set16(0xc, z);
            let y = host.u16(x.b, 0x10).wrapping_sub(8);
            x.set16(0x10, y);
            x.set16(TIMER, 0x14);
            if effect(x, true, 0x31, Some(2), true).is_some() {
                lib::sound(x, 0xe4e5a);
            }
            body(x, 0x16);
        }
        2 => {
            if !sub16(x, TIMER, 1).gt() {
                x.add8(0x7, 2);
            }
            lib::step(x);
        }
        4 => {
            sub16(x, 0xc, 1);
            if x.a5_u8(0x500) != 0 && cmp16(x, 0xc, 0x58).le() {
                x.add8(0x7, 2);
                x.set16(TIMER, 0x1e);
            }
            lib::step(x);
        }
        6 => {
            if sub16(x, TIMER, 1).gt() {
                lib::step(x);
                return;
            }
            x.add8(0x7, 2);
            x.set16(0x14, 0x40);
            x.set16(0x16, 0);
            x.set16(0x18, 0x300);
            x.set16(0x1a, (-0x20i16) as u16);
            x.set16(0x1c, (-0x80i16) as u16);
            x.set16(0x1e, 0);
            x.move_xyz();
            body(x, 4);
        }
        8 => {
            x.move_xyz();
            if tst16(x, 0x18).gt() {
                lib::step(x);
                return;
            }
            x.add8(0x7, 2);
            body(x, 7);
        }
        10 => {
            x.move_xyz();
            if tst16(x, 0xc).gt() {
                lib::step(x);
                return;
            }
            x.add8(0x7, 2);
            x.set16(0xc, 0);
            body(x, 9);
        }
        12 => {
            if lib::step(x) {
                x.add8(0x7, 2);
                body(x, 0);
            }
        }
        14 => {
            if x.a5_u8(0x2c) != 0xc {
                lib::step(x);
                return;
            }
            x.set8(0, 1);
            lib::set_immunities(x);
            decide::stand(x);
        }
        _ => {}
    }
}

fn entrance(x: &mut Cx) {
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            body(x, 0);
        }
        2 => {
            if x.data8(0xff2a38) != 0x10 {
                lib::step(x);
                return;
            }
            x.add8(0x7, 2);
            x.set16(0x18, 0x700);
            x.set16(0x1a, 0);
            x.set32(0x14, 0);
            lib::sound(x, 0xe580a);
            body(x, 6);
        }
        4 => {
            if !sub16(x, TIMER, 1).gt() {
                x.set16(TIMER, 5);
                dust(x);
            }
            x.move_xz();
            let floor = x.a5_u16(var::CAMERA1_Y).wrapping_add(0x100);
            let here = x.u16(0x10).wrapping_add(x.u16(0xc));
            if Fl::sub16(here, floor).lt() {
                return;
            }
            x.add8(f::SUB, 2);
            x.set_bit(0x1, 1, true);
        }
        _ => {}
    }
}
