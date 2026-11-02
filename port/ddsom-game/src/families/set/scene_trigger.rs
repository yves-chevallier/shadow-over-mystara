use super::*;
use util::*;

const CAMERA_X: u32 = 0x506;
const CAMERA_LIMIT: u16 = 0x1c80;
const PLAYER_MODE_DONE: u8 = 2;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
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
    x.add8(0x4, 2);
    x.set_a5_u8(0x42, 1);
    x.set_a5_u8(0x4a, 1);
    x.set_a5_u32(0x2c, 0x10000000);
}
