//! Boss 6, group `+5` = 0: arrival. The Manticore enters from the right by leaping, lands, roars then
//! waits for the camera to freeze before fighting.

use crate::families::cx::Bench;
use super::decide;
use super::*;
use crate::objects::Current;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => enter(x),
        2 => rise(x),
        4 => peak(x),
        6 => land(x),
        8 => roar_wait(x),
        10 => settle(x),
        12 => stand_up(x),
        14 => wait_camera(x),
        _ => {}
    }
}

/// +6 = 0: placed to the right of the camera, it sets off in a leap toward the left.
fn enter<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 2);
    x.set_bit(Enemy::DISPLAY, 1, false);
    let px = var::CAMERA1_X.get(x.b).wrapping_add(0x1e0);
    let py = var::CAMERA1_Y.get(x.b).wrapping_add(0x48);
    x.set16(Obj::X, px);
    x.set16(Obj::Y, py);
    x.set16(Enemy::VX, (-0x500i16) as u16);
    x.set16(Enemy::AX, 0);
    x.set16(Enemy::VZ, 0x400);
    x.set16(Enemy::GRAVITY, (-0x40i16) as u16);
    x.move_xz();
    fly(x, 7);
}

/// +6 = 2: rise of the leap.
fn rise<R: Bench>(x: &mut Cx<'_, R>) {
    lib::step(x);
    if x.u8(f::KEY_FRAME) != 0 {
        return;
    }
    x.move_xz();
    if cmp16(x, 0x18, 0x100).le() {
        x.add8(f::SUB2, 2);
        lib::sound(x, crate::sound::CMD_23);
        fly(x, 8);
    }
}

/// +6 = 4: top of the leap.
fn peak<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xz();
    if cmp16(x, 0x18, 0xff00).le() {
        x.add8(f::SUB2, 2);
        fly(x, 9);
    }
}

/// +6 = 6: fall; on landing, shake and dust.
fn land<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xz();
    if tst16(x, 0xc).gt() {
        return;
    }
    x.add8(f::SUB2, 2);
    x.set16(Obj::Z, 0);
    x.set16(Boss::TIMER, 0x12);
    x.set16(Boss6::COUNT, 0);
    x.set16(Enemy::AX, 0x48);
    lib::shake_58(x, 0x1e);
    lib::sound(x, crate::sound::CMD_328_428_B);
    fly(x, 0xa);
}

/// +6 = 8: landing slide, with dust every 4 frames.
fn roar_wait<R: Bench>(x: &mut Cx<'_, R>) {
    if !sub16(x, Boss6::COUNT, 1).gt() {
        x.set16(Boss6::COUNT, 4);
        lib::debris(x, 3);
    }
    if sub16(x, Boss::TIMER, 1).le() {
        x.add8(f::SUB2, 2);
        x.set16(Boss::TIMER, 0x14);
        return;
    }
    x.move_x();
    lib::step(x);
}

/// +6 = 10: pause before getting up.
fn settle<R: Bench>(x: &mut Cx<'_, R>) {
    if sub16(x, Boss::TIMER, 1).le() {
        x.add8(f::SUB2, 2);
        lib::sound(x, crate::sound::CMD_30C_40C);
        body(x, 8);
    }
}

/// +6 = 12: gets up.
fn stand_up<R: Bench>(x: &mut Cx<'_, R>) {
    if lib::step(x) {
        x.add8(f::SUB2, 2);
        body(x, 0);
    }
}

/// +6 = 14: waits for the camera to be ready (`A5+0x2c` = 0xc), then combat.
fn wait_camera<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PHASE.get(x.b) == 0xc {
        lib::create_set_14(x);
        decide::chase(x);
    } else {
        lib::step(x);
    }
}
