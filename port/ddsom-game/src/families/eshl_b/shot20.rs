use super::*;
use crate::families::shell::tables::{tables, Read};

const ANIMS: u32 = 0x17d7d2;
const ANIMS_STRIKE: u32 = 0x17d7e0;
const FROZEN: u32 = 0x4e;
const ARMED: u32 = 0xa5;
/// Substates (`+5`).
const IDLE: u8 = 0;
const STRIKE: u8 = 2;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => init(x),
        2 => active(x),
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(GAME_PAUSED) != 0 {
        return;
    }
    x.add8(0x4, 2);
    x.set8(0x1d, 0);
    attach(x, &tables().shot20_attach);
    let mut anim = 3;
    if x.u8(0x3) != 0 {
        x.set16(f::TIMER, 0x1e);
        anim = 2;
    }
    x.anim_start(ANIMS, anim);
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(GAME_PAUSED) != 0 {
        return;
    }
    if x.a5_u8(FROZEN) != 0 || suspended(x) {
        return freeze(x);
    }
    match x.u8(0x5) {
        IDLE => idle(x),
        STRIKE => strike(x),
        _ => {}
    }
    x.reinsert();
}

/// Frozen object: variant 0 goes back once to state 2 and gives the weapon back to the owner.
fn freeze<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x3) != 0 || x.u8(0x1d) != 0 {
        return;
    }
    x.set8(0x1d, 1);
    x.set32(0x4, 0x0200_0000);
    let owner = x.linked(f::OWNER);
    let armed = owner.u8(x.b, ARMED) & !1;
    owner.set_u8(x.b, ARMED, armed);
    crate::sound::play(x.b, crate::sound::CMD_37A, x.o);
    x.anim_start(ANIMS, 3);
}

fn idle<R: Bench>(x: &mut Cx<'_, R>) {
    match (x.u8(0x6), x.u8(0x3)) {
        (0, 0) => {
            x.set8(0x1d, 0);
            x.add8(0x6, 2);
            let rng = x.random();
            let anim = tables().shot20_idle_anims.at8(x.b, rng as u32 & 7) as u32;
            x.anim_start(ANIMS, anim as u16);
            attach(x, &tables().shot20_attach);
            x.anim_step();
        }
        (0, _) => {
            x.add16(f::TIMER, 0xffff);
            if (x.u16(f::TIMER) as i16) < 0 {
                x.add8(0x6, 2);
                x.set8(f::BOX_SET, 7);
                x.set8(f::ATTACK_ID, 0x28);
                x.anim_start(ANIMS_STRIKE, 0);
            }
            attach(x, &tables().shot20_attach);
            x.anim_step();
        }
        (2, 0) => {
            let owner = x.linked(f::OWNER);
            if (owner.u32(x.b, 0x4) as i32) >= 0x0200_0600 && x.u8(0x33) != 0 {
                x.set8(0x33, 0);
                crate::sound::play(x.b, crate::sound::CMD_311_411, x.o);
            }
            if x.anim_step() {
                if x.u16(f::OWNER) == 0 {
                    return kill(x);
                }
                let owner = x.linked(f::OWNER);
                if owner.u8(x.b, ARMED) & 1 != 0 {
                    return x.set32(0x4, 0x0202_0000);
                }
                x.add8(0x6, 0xfe);
            }
            attach(x, &tables().shot20_attach);
            x.anim_step();
        }
        (2, _) => {
            if x.anim_step() {
                x.add8(0x6, 0xfe);
                let rng = x.random();
                let delay = tables().shot20_delays.at16(x.b, 2 * (rng as u32 & 7));
                x.set16(f::TIMER, delay);
                x.anim_start(ANIMS, 2);
            } else {
                attach(x, &tables().shot20_attach);
            }
        }
        _ => {}
    }
}

fn strike<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x6) {
        0 => {
            x.add8(0x6, 2);
            x.anim_start(ANIMS_STRIKE, 1);
            attach(x, &tables().shot20_attach);
            x.anim_step();
        }
        2 => {
            if !x.anim_step() {
                return attach(x, &tables().shot20_attach);
            }
            crate::sound::play(x.b, crate::sound::CMD_311_411, x.o);
            crate::sound::play(x.b, crate::sound::CMD_379, x.o);
            x.add8(0x6, 2);
            // Original glitch: the allocation failure is not tested (see `Pool::alloc_unchecked`).
            let blow = x.alloc_unchecked(&pools::ESHL);
            let active = x.u8(0x0);
            blow.set_u8(x.b, 0, active);
            blow.set_u8(x.b, 3, 0);
            blow.set_u8(x.b, 2, 0x15);
            blow.set_u16(x.b, f::OWNER, low16(x.o));
            x.set16(0x42, low16(blow));
            x.anim_start(ANIMS_STRIKE, 2);
        }
        4 => {
            let blow_gone = x.u16(0x42) == 0 || {
                let blow = x.linked(0x42);
                blow.u8(x.b, 0x1e) == 1
            };
            if blow_gone {
                x.add8(0x6, 2);
                x.anim_start(ANIMS_STRIKE, 3);
            } else {
                attach(x, &tables().shot20_attach);
                x.anim_step();
            }
        }
        6 => {
            if x.anim_step() {
                x.set32(0x4, 0x0200_0000);
                let owner = x.linked(f::OWNER);
                let armed = owner.u8(x.b, ARMED) & !1;
                owner.set_u8(x.b, ARMED, armed);
                crate::sound::play(x.b, crate::sound::CMD_37A, x.o);
                x.anim_start(ANIMS, 3);
            } else {
                attach(x, &tables().shot20_attach);
            }
        }
        _ => {}
    }
}
