use super::*;
use util::*;

const PROGRESS: u32 = 0x556;
const PROGRESS_LIMIT: i16 = 0x320;
const RESET_STATE: u32 = 0x80000;
const END_FLAGS: u32 = 0x18000000;
const P_STATE: u32 = 0x4;
const P_SUBSTATE: u32 = 0x6;
const P_ORDER: u32 = 0x7d;
const P_WEAPON: u32 = 0x8a;
const P_WEAPON_OBJ: u32 = 0x86;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            wait_players(x);
        }
        2 => wait_players(x),
        _ => x.free(&pools::SET),
    }
}

fn wait_players<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(IN_GAME) == 0 || (x.a5_u16(PROGRESS) as i16) <= PROGRESS_LIMIT {
        return;
    }
    x.set_a5_u8(0x43, 0);
    x.set_a5_u16(0x51e, 0);
    x.set_a5_u8(0x4ff, 4);
    for p in players() {
        if alive(x, p) && (p.u16(x.b, P_STATE) != 0x200 || p.u8(x.b, P_SUBSTATE) != 0) {
            return;
        }
    }
    let mut order = 0;
    for p in players() {
        if !alive(x, p) {
            continue;
        }
        p.set_u32(x.b, P_STATE, RESET_STATE);
        p.set_u8(x.b, 0x66, 0);
        p.set_u8(x.b, 0x15c, 0);
        if p.u8(x.b, P_WEAPON) != 0 {
            p.set_u8(x.b, P_WEAPON, 0);
            let weapon = Obj(p.u32(x.b, P_WEAPON_OBJ));
            weapon.set_u8(x.b, 5, 8);
            weapon.set_u16(x.b, 6, 0);
        }
        p.set_u8(x.b, P_ORDER, order);
        order += 1;
    }
    x.set_a5_u32(0x2c, END_FLAGS);
    x.add8(0x4, 2);
}
