//! Type 27, substate 0: arrival. `+6` = 0 the machine rolls in from the edge of the screen, 2 and 4 it waits (player
//! range box) before standing up.

use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => roll_in(x),
        2 => wait_a(x),
        4 => wait_b(x),
        _ => {}
    }
}

/// 0x77e70: rolls in from the edge, shakes the ground, then goes on to shot A.
fn roll_in<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set_bit(Enemy::DISPLAY, 1, false);
            x.set8(FLAG, 0);
            lib::set_immunities(x);
            let cam = camera_x(x).wrapping_add(0xc0);
            x.set8(f::FLIP, 0);
            let mut angle = 0x18;
            if (cam as i16) > (x.u16(Obj::X) as i16) {
                x.set8(f::FLIP, 0x20);
                angle = 8;
            }
            x.set8(Obj::SPEED_INDEX, angle);
            set_speed(x, 0x21);
            lib::sound(x, crate::sound::CMD_376_476);
            look(x, 0x30);
            weapon(x, 3);
        }
        2 => {
            x.anim_step();
            x.move_x();
            if x.u8(f::KEY_FRAME) == 2 {
                x.set8(f::KEY_FRAME, 0);
                spark(x, 1);
                lib::shake_54(x, 0x10);
                lib::sound(x, crate::sound::CMD_376_476);
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

/// 0x77f48: start of the arrival leap (vertical thrust ±0x20, animation 9).
pub(super) fn leap_begin<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    x.set16(TIMER, 0x1e);
    x.set16(TIMER2, 0);
    let v: i16 = if x.u8(f::FLIP) != 0 { -0x20 } else { 0x20 };
    x.set16(Obj::AX, v as u16);
    lib::sound(x, crate::sound::CMD_376_476);
    lib::sound(x, crate::sound::CMD_329_429);
    mv(x, 9);
}

/// 0x77f7e / 0x78470 / 0x78670: leap: spark every 4 frames, timer `+0xa6`, then animation 0xa.
pub(super) fn hop_loop<R: Bench>(x: &mut Cx<'_, R>) {
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

/// 0x77fc4: waits for a player to be in the box (−0x40, 0x40, 0x40).
fn wait_a<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set_bit(Enemy::DISPLAY, 1, false);
            x.set8(FLAG, 0);
            lib::set_immunities(x);
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

/// 0x78040: same wait with box (−0x20, 0x20, 0x40).
fn wait_b<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set_bit(Enemy::DISPLAY, 1, false);
            x.set8(FLAG, 0);
            lib::set_immunities(x);
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

/// 0x78004 / 0x78080: chooses a target; as soon as it is in the box, next step (animation 3).
fn wait_for_player<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, d2: u16) {
    pick_target(x);
    if !lib::target_in_box(x, d0, d1, d2) {
        return;
    }
    x.add8(f::SUB3, 2);
    mv(x, 3);
}
