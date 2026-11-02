//! Type 18, substate 0: arrival. `+6` = 0 entrance fall from the right edge, 2 appearance on the ground (variant 0), 4
//! wait for a player in the box (variant 1), 6 appearance hidden until a player arrives (variant 2).

use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => fall_in(x),
        2 => walk_in(x),
        4 => wait_for_player(x),
        6 => hidden(x),
        _ => {}
    }
}

/// Common start of an arrival: no longer invulnerable, immunities reset.
fn begin<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    x.set_bit(Enemy::DISPLAY, 1, false);
    x.set8(Ogre::INVULNERABLE, 0);
    lib::set_immunities(x);
}

// --- +6 = 0: entrance fall ----------------------------------------------------------------------------

fn fall_in<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            begin(x);
            let px = var::CAMERA1_X.get(x.b).wrapping_add(0x1e0);
            let pz = var::CAMERA1_Y.get(x.b).wrapping_add(0x34);
            x.set16(Obj::X, px);
            x.set16(Obj::Y, pz);
            x.set16(Obj::VX, -0x300i16 as u16);
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, 0x400);
            x.set16(Obj::AZ, -0x40i16 as u16);
            x.move_xz();
            act(x, 7);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) != 0 {
                return;
            }
            x.move_xz();
            if (x.u16(Obj::VZ) as i16) <= 0x100 {
                x.add8(f::SUB3, 2);
                act(x, 8);
            }
        }
        4 => {
            x.move_xz();
            if (x.u16(Obj::VZ) as i16) <= -0x100 {
                x.add8(f::SUB3, 2);
                act(x, 9);
            }
        }
        6 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set16(Obj::Z, 0);
            x.set16(Ogre::TIMER, 0x12);
            x.set16(Ogre::TIMER2, 0);
            x.set16(Obj::AX, 0x30);
            lib::shake_58(x, 0x1e);
            lib::sound(x, crate::sound::CMD_328_428_B);
            act(x, 0xa);
        }
        8 => {
            if !count_down(x, Ogre::TIMER2) {
                x.set16(Ogre::TIMER2, 4);
                spark(x, 3);
            }
            if count_down(x, Ogre::TIMER) {
                x.move_x();
                lib::step(x);
            } else {
                x.add8(f::SUB3, 2);
                x.set16(Ogre::TIMER, 0x14);
            }
        }
        10 => {
            if count_down(x, Ogre::TIMER) {
                return;
            }
            x.add8(f::SUB3, 2);
            lib::sound(x, crate::sound::CMD_30C_40C);
            mv(x, 8);
        }
        12 => {
            if lib::step(x) {
                x.add8(f::SUB3, 2);
                mv(x, 0);
            }
        }
        14 => {
            if var::PHASE.get(x.b) == 0xc {
                go::wander(x);
            } else {
                lib::step(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 2: appearance on the ground --------------------------------------------------------------------

fn walk_in<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            begin(x);
            x.set16(Obj::AX, 0);
            let mut angle = 0x18;
            x.set8(f::FLIP, 0);
            if (x.u16(Obj::X) as i16) <= (var::CAMERA1_X.get(x.b) as i16) {
                x.set8(f::FLIP, 0x20);
                angle = 8;
            }
            x.set8(Obj::SPEED_INDEX, angle);
            set_speed(x, 0x35);
            look(x, 0x31);
            mv(x, 4);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) != 2 {
                x.move_x();
                let cam = var::CAMERA1_X.get(x.b);
                let leftward = (x.u16(Obj::VX) as i16) < 0;
                let (mut d0, mut d1) = (cam.wrapping_add(if leftward { 0xf0 } else { 0x90 }), x.u16(Obj::X));
                if leftward {
                    std::mem::swap(&mut d0, &mut d1);
                }
                if (d1 as i16) >= (d0 as i16) && x.u8(f::KEY_FRAME) != 0 {
                    return begin_dash(x);
                }
            }
            thump(x);
        }
        4 => dash(x),
        _ => {}
    }
}

/// 0x70770: key 3 of the animation shakes the ground.
fn thump<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::KEY_FRAME) != 3 {
        return;
    }
    x.set8(f::KEY_FRAME, 0);
    lib::sound(x, crate::sound::CMD_329_429);
    spark(x, 4);
    lib::shake_54(x, 0xa);
}

/// 0x70794: start of the run (animation 5).
fn begin_dash<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    x.set16(Ogre::TIMER, 0x14);
    x.set16(Ogre::TIMER2, 0);
    let vy = if x.u8(f::FLIP) != 0 { -0x40i16 } else { 0x40 };
    x.set16(Obj::AX, vy as u16);
    lib::shake_54(x, 0x14);
    lib::sound(x, crate::sound::CMD_328_428);
    lib::sound(x, crate::sound::CMD_329_429);
    mv(x, 5);
}

/// 0x707d2: the run, then the first move.
fn dash<R: Bench>(x: &mut Cx<'_, R>) {
    if !count_down(x, Ogre::TIMER2) {
        x.set16(Ogre::TIMER2, 4);
        spark(x, 3);
    }
    if count_down(x, Ogre::TIMER) {
        x.move_x();
        return;
    }
    if lib::players_alive(x) {
        go::wander(x);
    } else {
        go::retire(x);
    }
}

// --- +6 = 4: wait for a player --------------------------------------------------------------------------

fn wait_for_player<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            begin(x);
            look(x, 0x32);
            mv(x, 0);
        }
        2 | 4 => {
            if lib::target_in_box(x, -0x40, 0x40, 0x40) {
                x.add8(f::SUB3, 2);
                lib::sound(x, crate::sound::CMD_30C_40C);
                mv(x, 8);
            } else {
                lib::step(x);
            }
        }
        6 => waiting_out(x),
        _ => {}
    }
}

/// 0x75c36: end of wait, countdown of `+0xa0` then back to the previous substate (`+0xb1`).
fn waiting_out<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.u8(Ogre::LOOK).wrapping_sub(1);
    x.set8(Ogre::LOOK, n);
    if (n as i8) >= 0 {
        return;
    }
    set_state_long(x, 0x0202_0000);
    if rand_and(x, 1) != 0 {
        let m = x.u8(Ogre::WAIT_ROUNDS).wrapping_sub(1);
        x.set8(Ogre::WAIT_ROUNDS, m);
        if (m as i8) >= 0 {
            return;
        }
        x.set8(Ogre::WAIT_ROUNDS, 2);
    }
    set_state_long(x, 0x0200_0000);
    let back = x.u8(Ogre::RESUME_STEP);
    x.set8(f::SUB2, back);
}

// --- +6 = 6: hidden appearance -----------------------------------------------------------------------------

fn hidden<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set8(Enemy::DOWNED, 1);
            x.set_bit(Enemy::DISPLAY, 1, false);
            x.set8(Ogre::INVULNERABLE, 0);
            lib::set_immunities(x);
            look(x, 0x33);
            mv(x, 0xa);
        }
        2 => {
            if lib::target_in_box(x, -0x20, 0x60, 0x20) {
                x.add8(f::SUB3, 2);
                x.set8(Enemy::DOWNED, 0);
            }
        }
        4 => {
            if lib::step(x) {
                go::stand(x);
            }
        }
        _ => {}
    }
}
