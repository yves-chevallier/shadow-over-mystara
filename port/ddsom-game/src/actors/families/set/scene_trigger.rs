use super::*;
use util::*;
use crate::actors::families::set::vars;
use crate::world::objects::Obj;

const CAMERA_X: u32 = 0x506;
const CAMERA_LIMIT: u16 = 0x1c80;
const PLAYER_MODE_DONE: u8 = 2;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            trigger(x);
        }
        2 => trigger(x),
        _ => x.free(&pools::SET),
    }
}

fn trigger<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(IN_GAME) == 0 || x.a5_u16(CAMERA_X) < CAMERA_LIMIT {
        return;
    }
    let mut count = 0;
    for p in players() {
        if alive(x, p) {
            p.set_u8(x.b, P_ACTIVE, PLAYER_MODE_DONE);
            count += 1;
        }
    }
    if count == 0 {
        return;
    }
    x.add8(Obj::STATE, 2);
    x.set_a5_u8(vars::FROZEN, 1);
    x.set_a5_u8(vars::PAUSE, 1);
    x.set_a5_u32(vars::PHASE, 0x10000000);
}
