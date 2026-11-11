use super::*;
use crate::actors::families::set::vars;
use crate::world::objects::Obj;
use crate::actors::families::set::fields::SetObj;
use crate::actors::families::set_b::fields::LevelClockObj;

const ON: crate::services::sound::Sound = crate::services::sound::CMD_9A_B;
const OFF: crate::services::sound::Sound = crate::services::sound::CMD_9B_B;
const TIME: u32 = 0x42;
const CUE: u32 = 0x1a;
const TICK: u32 = 0x1c;
const FAR: i16 = 0xa00;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set16(TIME, 0);
            let v = x.a5_u16(ram::CAMERA_X);
            x.set16(LevelClockObj::START_X, v);
        }
        2 => step(x),
        _ => x.free(&pools::SET),
    }
}

/// 0xa66c0, which reads its table row in D2 (left to the following handlers, `dispatch::Leftovers`).
fn speech<R: Bench>(x: &mut Cx<'_, R>, kind: u32) {
    // the row stays in D2 (`dispatch::Leftovers`)
    *x.c.d2() = kind;
    crate::actors::families::scenery::eshl_volley(x, kind);
}

fn countdown<R: Bench>(x: &mut Cx<'_, R>, reset: u16, kind: u32) {
    let n = x.u16(TICK).wrapping_sub(1);
    x.set16(TICK, n);
    if (n as i16) < 0 {
        x.set16(TICK, reset);
        speech(x, kind);
    }
    let n = x.u16(CUE).wrapping_sub(1);
    x.set16(CUE, n);
    if n == 0 {
        crate::services::sound::play(x.b, OFF, x.o);
    }
}

fn step<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(vars::PLAYERS_ALIVE) != 0 {
        x.add16(TIME, 1);
    }
    if x.u8(SetObj::SUBSTATE) != 6 {
        let far = x.a5_u16(ram::CAMERA_X).wrapping_sub(x.u16(LevelClockObj::START_X)) as i16;
        if far >= FAR {
            x.set16(CUE, 0x78);
            x.set8(SetObj::SUBSTATE, 6);
        }
    }
    let time = x.u16(TIME) as i16;
    match x.u8(SetObj::SUBSTATE) {
        0 => {
            if time < 0x258 {
                return;
            }
            x.add8(SetObj::SUBSTATE, 2);
            show_message(x, 2);
            x.set16(TICK, 0x50);
            speech(x, 0);
            x.set16(CUE, 0x28);
            crate::actors::camera_modes::start_shake(x.b, crate::actors::camera_modes::shake::A, 0x28);
            crate::services::sound::play(x.b, ON, x.o);
        }
        2 => {
            countdown(x, 0x50, 0);
            if time < 0x4b0 {
                return;
            }
            x.add8(SetObj::SUBSTATE, 2);
            show_message(x, 2);
            x.set16(TICK, 0x3c);
            speech(x, 1);
            x.set16(CUE, 0x28);
            crate::actors::camera_modes::start_shake(x.b, crate::actors::camera_modes::shake::B, 0x28);
        }
        4 => {
            countdown(x, 0x3c, 1);
            if time < 0x708 {
                return;
            }
            x.add8(SetObj::SUBSTATE, 2);
            speech(x, 2);
            x.set16(CUE, 0x78);
            crate::actors::camera_modes::start_shake(x.b, crate::actors::camera_modes::shake::C, 0x78);
            crate::services::sound::play(x.b, ON, x.o);
        }
        6 => {
            let n = x.u16(CUE).wrapping_sub(1);
            x.set16(CUE, n);
            if n != 0 {
                return;
            }
            x.add8(Obj::STATE, 2);
            x.set_a5_u8(vars::FROZEN, 1);
            crate::services::sound::play(x.b, OFF, x.o);
        }
        _ => {}
    }
}
