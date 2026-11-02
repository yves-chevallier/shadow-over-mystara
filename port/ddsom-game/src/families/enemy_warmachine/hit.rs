//! Type 27, hit state (`+4 = 4`, 0x789d0). `+5`: 0 reaction by table (or start of death if the HP are exhausted), 2
//! recoil, 4 fall and get up (with HP threshold), 6 death in five stages, 8 flight off the screen, 10 knocked out. Zone
//! and shadow are tested except in flight (`+5` = 8).

use crate::families::cx::Bench;
use crate::data::RomAddr;
use super::*;
use crate::objects::pools;
use crate::objects::Current;
use crate::objects::Obj;

/// Table of reactions shared by the hits, indexed by code `+0x49`.
const REACT: RomAddr = RomAddr(0x78a26);

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u8(f::FREEZE) as i8) > 0 {
        let n = x.u8(f::FREEZE).wrapping_sub(1);
        x.set8(f::FREEZE, n);
    } else {
        match x.u8(f::SUB) {
            0 => first(x),
            2 => hop_back(x),
            4 => get_up(x),
            6 => death(x),
            8 => run_off(x),
            10 => stunned(x),
            _ => {}
        }
        if x.u8(f::SUB) == 8 {
            return;
        }
    }
    zone_and_terrain(x);
}

/// 0x792e0: HP exhausted: goes into death (`+5` = 6); true in that case.
fn dying<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if (x.u16(f::HP) as i16) > 0 {
        return false;
    }
    x.set8(f::SUB, 6);
    x.set8(Enemy::ACTIVE, 2);
    true
}

fn first<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        x.set8(WarMachine::FLAG, 0);
        if dying(x) {
            return death(x);
        }
    }
    react(x, REACT.0);
}

/// 0x78b4a: recoil backward.
fn hop_back<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.set16(WarMachine::TIMER, 0x14);
            let (angle, v) = if x.u8(f::FLIP) != 0 { (0x18, 0x20i16) } else { (8, -0x20i16) };
            x.set8(Obj::SPEED_INDEX, angle);
            x.set16(Obj::AX, v as u16);
            set_speed(x, 0x17);
            look(x, 0x20);
            x.add8(f::SUB2, 2);
            lib::sound(x, crate::sound::CMD_34F_44F);
            mv(x, 0xb);
        }
        2 => {
            let t = x.u16(WarMachine::TIMER).wrapping_sub(1);
            x.set16(WarMachine::TIMER, t);
            if (t as i16) <= 0 {
                x.add8(f::SUB2, 2);
                x.set16(WarMachine::TIMER, 6);
                x.set16(Obj::AX, 0);
            }
            x.move_x();
            lib::step(x);
        }
        4 => {
            let t = x.u16(WarMachine::TIMER).wrapping_sub(1);
            x.set16(WarMachine::TIMER, t);
            if (t as i16) > 0 {
                x.move_x();
                lib::step(x);
                return;
            }
            x.set32(Enemy::HIT_LOCK, 0);
            go::attack(x, 0xa, true);
        }
        _ => {}
    }
}

/// 0x78be2: fall then get up.
fn get_up<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(WarMachine::LOOK_PREV, 0xff);
            x.anim_start(SET_MOVE.0, 0x12);
        }
        2 => {
            if var::PAUSE.get_i8(x.b) > 0 {
                return;
            }
            let n = x.u16(Enemy::RECOVER).wrapping_sub(1);
            x.set16(Enemy::RECOVER, n);
            if (n as i16) > 0 {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(Enemy::DOWNED, 0);
            x.set8(Enemy::REACTION, 0);
        }
        4 => {
            if paused(x) {
                return;
            }
            if x.anim_step() {
                recover(x);
            }
        }
        _ => {}
    }
}

/// 0x78c3c: get up: chooses a target, crosses an HP threshold, then shot or attack leap.
pub(crate) fn recover<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::ACTIVE, 1);
    x.set8(WarMachine::LOOK_PREV, 0xff);
    if !pick_target(x) {
        return go::idle(x);
    }
    let table = if x.a5_u8(0x48) > 5 { 0x109e2 } else { 0x108e2 };
    lib::next_threshold(x, table);
    if lib::target_in_box(x, 0, 0x38, 0xc) {
        return go::attack(x, 6, true);
    }
    go::fire_a(x);
}

/// 0x78c84: death in five stages: ejection, flight, landing, shaking, disappearance.
fn death<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(Enemy::ACTIVE, 2);
            x.set32(f::IMMUNITY, 0xffff_ffff);
            lib::sound(x, crate::sound::CMD_30B_40B);
            lib::object_event(x);
            lib::drop_rewards(x);
            detach_from_host(x);
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, 0x500);
            x.set16(Obj::AZ, -0x38i16 as u16);
            let cam = camera_x(x).wrapping_add(0xc0);
            let (mut speed, mut flip) = (0x200i16, 0u8);
            if (cam as i16) < (x.u16(Obj::X) as i16) {
                speed = -speed;
                flip = 0x20;
            }
            x.set16(Obj::VX, speed as u16);
            x.set8(f::FLIP, flip);
            x.move_xz();
            x.anim_start(SET_MOVE.0, 0x13);
        }
        2 => {
            x.move_xz();
            if (x.u16(Obj::VZ) as i16) > 0 {
                lib::step(x);
                return;
            }
            x.add8(f::SUB2, 2);
            x.anim_start(SET_MOVE.0, 0x14);
        }
        4 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0 {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(Obj::Z, 0);
            spark(x, 5);
            lib::sound(x, crate::sound::CMD_327_427_B);
            lib::step(x);
        }
        6 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
                x.set16(WarMachine::TIMER, 0x3c);
            }
        }
        8 => {
            let t = x.u16(WarMachine::TIMER).wrapping_sub(1);
            x.set16(WarMachine::TIMER, t);
            if (t as i16) <= 0 {
                return x.free(&pools::ENEMY);
            }
            let flags = x.u8(Enemy::DISPLAY) ^ 2;
            x.set8(Enemy::DISPLAY, flags);
        }
        _ => {}
    }
}

/// 0x78d92: grab: the machine runs away off the screen then disappears.
fn run_off<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set16(Obj::AX, 0);
            let angle = heading(x);
            x.set8(Obj::SPEED_INDEX, angle);
            set_speed(x, 0x21);
            spark(x, 1);
            lib::sound(x, crate::sound::CMD_376_476);
            look(x, 0x40);
            mv(x, 8);
        }
        2 => {
            x.move_x();
            lib::step(x);
            if x.u8(f::KEY_FRAME) == 2 {
                x.set8(f::KEY_FRAME, 0);
                spark(x, 1);
                lib::shake_54(x, 0x10);
                lib::sound(x, crate::sound::CMD_376_476);
            }
            if lib::outside_band(x, -0x30, 0x1b0) {
                detach_from_host(x);
                x.free(&pools::ENEMY);
            }
        }
        _ => {}
    }
}

/// 0x78e34: knocked out: animation 0xc, then shot.
fn stunned<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB3, 2);
        return mv(x, 0xc);
    }
    if !lib::step(x) {
        return;
    }
    if grabbed(x) {
        return go::held(x);
    }
    if !lib::players_alive(x) {
        return go::idle(x);
    }
    recover(x);
}
