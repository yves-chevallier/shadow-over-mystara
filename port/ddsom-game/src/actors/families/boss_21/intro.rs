//! Boss 21, group `+5` = 0: arrival on stage.
//!
//! | `+6` | Role |
//! |---|---|
//! | 0 | arrival from the right of the screen: approach, leap, landing (`+7` = 0 to 14) |
//! | 2 | entry into combat: falls from the top of the screen (`+7` = 0 to 4) |

use super::decide;
use crate::actors::families::cx::Bench;
use super::*;
use crate::world::objects::Current;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => arrival(x),
        2 => entrance(x),
        _ => {}
    }
}

fn arrival<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Enemy::ACTIVE, 2);
            x.set_bit(Enemy::DISPLAY, 1, false);
            x.set8(f::FLIP, 0);
            let cx = var::SCROLL_X.get(x.b).wrapping_add(0x140);
            x.set16(Obj::X, cx);
            let host = x.linked(f::HOST);
            let z = host.u16(x.b, Obj::Z).wrapping_sub(0x10);
            x.set16(Obj::Z, z);
            let y = host.u16(x.b, Obj::Y).wrapping_sub(8);
            x.set16(Obj::Y, y);
            x.set16(Boss::TIMER, 0x14);
            if effect(x, true, 0x31, Some(2), true).is_some() {
                lib::sound(x, crate::hw::sound::CMD_311_411);
            }
            body(x, 0x16);
        }
        2 => {
            if !sub16(x, Boss::TIMER, 1).gt() {
                x.add8(Enemy::SUB_STEP, 2);
            }
            lib::step(x);
        }
        4 => {
            sub16(x, Obj::Z, 1);
            if var::LANDING_CUE.get(x.b) != 0 && cmp16(x, Obj::Z, 0x58).le() {
                x.add8(Enemy::SUB_STEP, 2);
                x.set16(Boss::TIMER, 0x1e);
            }
            lib::step(x);
        }
        6 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                lib::step(x);
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Enemy::VX, 0x40);
            x.set16(Enemy::AX, 0);
            x.set16(Enemy::VZ, 0x300);
            x.set16(Enemy::GRAVITY, (-0x20i16) as u16);
            x.set16(Enemy::VY, (-0x80i16) as u16);
            x.set16(Enemy::AY, 0);
            x.move_xyz();
            body(x, 4);
        }
        8 => {
            x.move_xyz();
            if tst16(x, Enemy::VZ).gt() {
                lib::step(x);
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            body(x, 7);
        }
        10 => {
            x.move_xyz();
            if tst16(x, Obj::Z).gt() {
                lib::step(x);
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Obj::Z, 0);
            body(x, 9);
        }
        12 => {
            if lib::step(x) {
                x.add8(Enemy::SUB_STEP, 2);
                body(x, 0);
            }
        }
        14 => {
            if var::PHASE.get(x.b) != 0xc {
                lib::step(x);
                return;
            }
            x.set8(Enemy::ACTIVE, 1);
            lib::set_immunities(x);
            decide::stand(x);
        }
        _ => {}
    }
}

fn entrance<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            body(x, 0);
        }
        2 => {
            if var::SCENE_SEQUENCE.get(x.b) != 0x10 {
                lib::step(x);
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Enemy::VZ, 0x700);
            x.set16(Enemy::GRAVITY, 0);
            x.set32(Enemy::VX_LONG, 0);
            lib::sound(x, crate::hw::sound::CMD_80_80);
            body(x, 6);
        }
        4 => {
            if !sub16(x, Boss::TIMER, 1).gt() {
                x.set16(Boss::TIMER, 5);
                dust(x);
            }
            x.move_xz();
            let floor = var::CAMERA1_Y.get(x.b).wrapping_add(0x100);
            let here = x.u16(Obj::Y).wrapping_add(x.u16(Obj::Z));
            if Fl::sub16(here, floor).lt() {
                return;
            }
            x.add8(f::SUB, 2);
            x.set_bit(Enemy::DISPLAY, 1, true);
        }
        _ => {}
    }
}
