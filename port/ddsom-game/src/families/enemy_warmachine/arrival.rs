//! Type 27, sous-état 0 : arrivée. `+6` = 0 la machine entre en roulant par le bord de l'écran, 2 et 4 elle attend
//! (boîte de portée du joueur) avant de se mettre debout.

use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

pub(super) fn run(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => roll_in(x),
        2 => wait_a(x),
        4 => wait_b(x),
        _ => {}
    }
}

/// 0x77e70 : roule depuis le bord, fait trembler le sol, puis passe au tir A.
fn roll_in(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB3) as u32;
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set_bit(1, 1, false);
            x.set8(FLAG, 0);
            x.call(call::IMMUNITIES);
            let cam = camera_x(x).wrapping_add(0xc0);
            x.c.d[2] = x.c.d[2] & !0xffff | cam as u32;
            x.set8(f::FLIP, 0);
            let mut angle = 0x18;
            if (cam as i16) > (x.u16(Obj::X) as i16) {
                x.set8(f::FLIP, 0x20);
                angle = 8;
            }
            x.set8(Obj::SPEED_INDEX, angle);
            set_speed(x, 0x21);
            x.call(call::DUST);
            look(x, 0x30);
            weapon(x, 3);
        }
        2 => {
            x.anim_step();
            x.move_x();
            if x.u8(f::KEY_FRAME) == 2 {
                x.set8(f::KEY_FRAME, 0);
                spark(x, 1);
                x.c.d[0] = 0x10;
                x.call(call::SHOCK);
                x.call(call::DUST);
            }
            let cam = camera_x(x);
            let reach = if (x.u16(Obj::VX) as i16) < 0 { 0xe0 } else { 0xa0 };
            let (mut d0, mut d1) = (cam.wrapping_add(reach), x.u16(Obj::X));
            if (x.u8(Obj::VX) as i8) < 0 {
                std::mem::swap(&mut d0, &mut d1);
            }
            if (d1 as i16) < (d0 as i16) {
                return;
            }
            leap_begin(x);
        }
        4 => hop_loop(x),
        6 => {
            if lib::step(x) {
                go::fire_a(x);
            }
        }
        _ => {}
    }
}

/// 0x77f48 : début du bond d'arrivée (poussée verticale ±0x20, animation 9).
pub(super) fn leap_begin(x: &mut Cx) {
    x.add8(f::SUB3, 2);
    x.set16(TIMER, 0x1e);
    x.set16(TIMER2, 0);
    let v: i16 = if x.u8(f::FLIP) != 0 { -0x20 } else { 0x20 };
    x.set16(Obj::AX, v as u16);
    x.c.d[0] = v as u16 as u32;
    x.call(call::DUST);
    x.call(call::FLASH);
    mv(x, 9);
}

/// 0x77f7e / 0x78470 / 0x78670 : bond : étincelle toutes les 4 images, minuterie `+0xa6`, puis animation 0xa.
pub(super) fn hop_loop(x: &mut Cx) {
    let n = x.u16(TIMER2).wrapping_sub(1);
    x.set16(TIMER2, n);
    if (n as i16) <= 0 {
        x.set16(TIMER2, 4);
        spark(x, 0);
    }
    let t = x.u16(TIMER).wrapping_sub(1);
    x.set16(TIMER, t);
    if (t as i16) > 0 {
        x.move_x();
        lib::step(x);
        return;
    }
    x.add8(f::SUB3, 2);
    mv(x, 0xa);
}

/// 0x77fc4 : attend qu'un joueur soit dans la boîte (−0x40, 0x40, 0x40).
fn wait_a(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set_bit(1, 1, false);
            x.set8(FLAG, 0);
            x.call(call::IMMUNITIES);
            look(x, 0x31);
            mv(x, 2);
        }
        2 => wait_for_player(x, -0x40, 0x40, 0x40),
        4 => {
            if lib::step(x) {
                go::fire_a(x);
            }
        }
        _ => {}
    }
}

/// 0x78040 : même attente avec la boîte (−0x20, 0x20, 0x40).
fn wait_b(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set_bit(1, 1, false);
            x.set8(FLAG, 0);
            x.call(call::IMMUNITIES);
            look(x, 0x32);
            mv(x, 0);
        }
        2 => wait_for_player(x, -0x20, 0x20, 0x40),
        4 => {
            if lib::step(x) {
                go::fire_a(x);
            }
        }
        _ => {}
    }
}

/// 0x78004 / 0x78080 : choisit une cible ; dès qu'elle est dans la boîte, étape suivante (animation 3).
fn wait_for_player(x: &mut Cx, d0: i16, d1: u16, d2: u16) {
    pick_target(x);
    set_dw(x, 0, d0 as u16);
    set_dw(x, 1, d1);
    set_dw(x, 2, d2);
    x.call(0xb5e0);
    if x.c.cond(7) {
        return;
    }
    x.add8(f::SUB3, 2);
    mv(x, 3);
}
