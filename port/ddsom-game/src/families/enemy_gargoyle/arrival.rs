//! Type 8, sous-état 0 : arrivée. `+6` = 0 : vol d'entrée par un bord, atterrissage, premier combat ;
//! `+6` = 2 : arrivée posée (crée une ombre, attend un joueur à portée).

use super::*;
use crate::objects::pools;
use crate::objects::Courant;
use crate::objects::Obj;

pub(super) fn run(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => flight(x),
        2 => landed(x),
        _ => {}
    }
}

/// Signe de l'orientation appliqué à `v` : négatif quand l'ennemi regarde à gauche.
fn signed_by_facing<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>, v: i16) -> u16 {
    if x.u8(f::FLIP) != 0 { v.wrapping_neg() as u16 } else { v as u16 }
}

fn flight(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set_bit(1, 1, false);
            let right = camera_x(x).wrapping_add(0xc0);
            x.set8(f::FLIP, 0);
            let mut angle = 0x18;
            if (right as i16) > (x.u16(Obj::X) as i16) {
                x.set8(f::FLIP, 0x20);
                angle = 8;
            }
            x.set8(Obj::SPEED_INDEX, angle);
            set_speed(x, 0x21);
            x.set16(Obj::Z, 0x28);
            look(x, 0x30);
            x.set8(INVULNERABLE, 0);
            lib::set_immunities(x);
            lib::sound(x, 0xe3776);
            fly(x, 5);
        }
        2 => {
            x.move_x();
            let cam = camera_x(x);
            let rightward = (x.u16(Obj::VX) as i16) >= 0;
            let mut d0 = cam.wrapping_add(if rightward { 0x90 } else { 0xf0 });
            let mut d1 = x.u16(Obj::X);
            if !rightward {
                std::mem::swap(&mut d0, &mut d1);
            }
            if (d1 as i16) < (d0 as i16) {
                return;
            }
            x.add8(f::SUB3, 2);
            let vy = signed_by_facing(x, 0x10);
            x.set16(Obj::AX, vy);
            x.set16(Obj::VZ, 0);
            x.set16(Obj::AZ, -0x40i16 as u16);
        }
        4 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set16(TIMER, 0x14);
            x.set16(TIMER2, 0);
            x.set16(Obj::Z, 0);
            let vy = signed_by_facing(x, 0x10);
            x.set16(Obj::AX, vy);
            fly(x, 6);
        }
        6 => {
            if !count_down(x, TIMER2) {
                x.set16(TIMER2, 4);
                spark(x, 8);
            }
            x.move_x();
            if count_down(x, TIMER) {
                lib::step(x);
            } else if lib::players_alive(x) {
                go::wander(x);
            } else {
                go::retire(x);
            }
        }
        _ => {}
    }
}

/// `+6` = 2 : l'ombre est créée (EFFCT 0x32), puis la créature attend un joueur dans sa boîte.
fn landed(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set_bit(1, 1, false);
            x.set8(NO_GRAB, 1);
            if let Some(shadow) = x.alloc(&pools::EFFCT) {
                shadow.set_u8(x.b, 0, 1);
                shadow.set_u8(x.b, 2, 0x32);
                shadow.set_u8(x.b, 3, 0xe);
                let px = x.u16(Obj::X);
                shadow.set_u16(x.b, Obj::X, px);
                let depth = x.u32(Obj::Y);
                shadow.set_u32(x.b, Obj::Y, depth);
                let n = shadow.u16(x.b, 0x12).wrapping_add(1);
                shadow.set_u16(x.b, 0x12, n);
                shadow.set_u16(x.b, 0x42, x.o.0 as u16);
                x.set16(Obj::Z, 0x28);
            }
            look(x, 0x31);
            fly(x, 0x13);
        }
        2 => {
            pick_target(x);
            if lib::target_in_box(x, 0, 0x20, 0x40) {
                x.add8(f::SUB3, 2);
                x.set16(TIMER, 0x18);
            }
        }
        4 => {
            if count_down(x, TIMER) {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set16(TIMER, 0x32);
            let mut flip = 0;
            if (x.u16(f::TARGET) as i16) < 0 {
                let target = x.linked(f::TARGET);
                if (target.u16(x.b, Obj::X) as i16) > (x.u16(Obj::X) as i16) {
                    flip = 0x20;
                }
            }
            x.set8(f::FLIP, flip);
            lib::step(x);
        }
        6 => {
            if count_down(x, TIMER) {
                lib::step(x);
                return;
            }
            x.set8(NO_GRAB, 0);
            x.set8(INVULNERABLE, 0);
            lib::set_immunities(x);
            spark(x, 0xa);
            if (x.u16(f::TARGET) as i16) >= 0 {
                go::dive(x);
            } else {
                go::strike_12(x);
            }
        }
        _ => {}
    }
}
