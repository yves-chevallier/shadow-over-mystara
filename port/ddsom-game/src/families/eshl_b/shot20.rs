use super::*;
use crate::data::RomAddr;
use crate::families::shell::vars;
use crate::families::shell::records::view;
use crate::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x17d7d2);
const ANIMS_STRIKE: RomAddr = RomAddr(0x17d7e0);
const ARMED: u32 = 0xa5;
/// Substates (`+5`).
const IDLE: u8 = 0;
const STRIKE: u8 = 2;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => init(x),
        2 => active(x),
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if vars::GAME_PAUSED.get(x.b) != 0 {
        return;
    }
    x.add8(Obj::STATE, 2);
    x.set8(Shot20::BUSY, 0);
    attach(x, &tables().shot20_attach);
    let mut anim = 3;
    if x.u8(Obj::VARIANT) != 0 {
        x.set16(Shot::TIMER, 0x1e);
        anim = 2;
    }
    x.anim_start(ANIMS.0, anim);
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if vars::GAME_PAUSED.get(x.b) != 0 {
        return;
    }
    if vars::SCRIPTED.get(x.b) != 0 || suspended(x) {
        return freeze(x);
    }
    match x.u8(Base::SUB_STATE) {
        IDLE => idle(x),
        STRIKE => strike(x),
        _ => {}
    }
    x.reinsert();
}

/// Frozen object: variant 0 goes back once to state 2 and gives the weapon back to the owner.
fn freeze<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Obj::VARIANT) != 0 || x.u8(Shot20::BUSY) != 0 {
        return;
    }
    x.set8(Shot20::BUSY, 1);
    x.set32(Obj::STATE, 0x0200_0000);
    let owner = x.linked(Shot::OWNER);
    let armed = owner.u8(x.b, ARMED) & !1;
    owner.set_u8(x.b, ARMED, armed);
    crate::sound::play(x.b, crate::sound::CMD_37A, x.o);
    x.anim_start(ANIMS.0, 3);
}

fn idle<R: Bench>(x: &mut Cx<'_, R>) {
    match (x.u8(Base::STEP), x.u8(Obj::VARIANT)) {
        (0, 0) => {
            x.set8(Shot20::BUSY, 0);
            x.add8(Base::STEP, 2);
            let rng = x.random();
            let anim = tables().shot20_idle_anims.at8(x.b, rng as u32 & 7) as u32;
            x.anim_start(ANIMS.0, anim as u16);
            attach(x, &tables().shot20_attach);
            x.anim_step();
        }
        (0, _) => {
            x.add16(Shot::TIMER, 0xffff);
            if (x.u16(Shot::TIMER) as i16) < 0 {
                x.add8(Base::STEP, 2);
                x.set8(Base::BOX_SET, 7);
                x.set8(Shot::ATTACK_ID, 0x28);
                x.anim_start(ANIMS_STRIKE.0, 0);
            }
            attach(x, &tables().shot20_attach);
            x.anim_step();
        }
        (2, 0) => {
            let owner = x.linked(Shot::OWNER);
            if (owner.u32(x.b, Obj::STATE) as i32) >= 0x0200_0600 && x.u8(Base::ANIM_EVENT) != 0 {
                x.set8(Base::ANIM_EVENT, 0);
                crate::sound::play(x.b, crate::sound::CMD_311_411, x.o);
            }
            if x.anim_step() {
                if x.u16(Shot::OWNER) == 0 {
                    return kill(x);
                }
                let owner = x.linked(Shot::OWNER);
                if owner.u8(x.b, ARMED) & 1 != 0 {
                    return x.set32(Obj::STATE, 0x0202_0000);
                }
                x.add8(Base::STEP, 0xfe);
            }
            attach(x, &tables().shot20_attach);
            x.anim_step();
        }
        (2, _) => {
            if x.anim_step() {
                x.add8(Base::STEP, 0xfe);
                let rng = x.random();
                let delay = tables().shot20_delays.at16(x.b, 2 * (rng as u32 & 7));
                x.set16(Shot::TIMER, delay);
                x.anim_start(ANIMS.0, 2);
            } else {
                attach(x, &tables().shot20_attach);
            }
        }
        _ => {}
    }
}

fn strike<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Base::STEP) {
        0 => {
            x.add8(Base::STEP, 2);
            x.anim_start(ANIMS_STRIKE.0, 1);
            attach(x, &tables().shot20_attach);
            x.anim_step();
        }
        2 => {
            if !x.anim_step() {
                return attach(x, &tables().shot20_attach);
            }
            crate::sound::play(x.b, crate::sound::CMD_311_411, x.o);
            crate::sound::play(x.b, crate::sound::CMD_379, x.o);
            x.add8(Base::STEP, 2);
            // Original glitch: the allocation failure is not tested (see `Pool::alloc_unchecked`).
            let blow = x.alloc_unchecked(&pools::ESHL);
            let active = x.u8(Obj::ACTIVE);
            blow.set_u8(x.b, Obj::ACTIVE, active);
            blow.set_u8(x.b, Obj::VARIANT, 0);
            blow.set_u8(x.b, Obj::KIND, 0x15);
            blow.set_u16(x.b, Shot::OWNER, low16(x.o));
            x.set16(Shot20::BLOW, low16(blow));
            x.anim_start(ANIMS_STRIKE.0, 2);
        }
        4 => {
            let blow_gone = x.u16(Shot20::BLOW) == 0 || {
                let blow = x.linked(Shot20::BLOW);
                blow.u8(x.b, super::shot21::Shot21::ENDED) == 1
            };
            if blow_gone {
                x.add8(Base::STEP, 2);
                x.anim_start(ANIMS_STRIKE.0, 3);
            } else {
                attach(x, &tables().shot20_attach);
                x.anim_step();
            }
        }
        6 => {
            if x.anim_step() {
                x.set32(Obj::STATE, 0x0200_0000);
                let owner = x.linked(Shot::OWNER);
                let armed = owner.u8(x.b, ARMED) & !1;
                owner.set_u8(x.b, ARMED, armed);
                crate::sound::play(x.b, crate::sound::CMD_37A, x.o);
                x.anim_start(ANIMS.0, 3);
            } else {
                attach(x, &tables().shot20_attach);
            }
        }
        _ => {}
    }
}

view! {
    /// Fields of shot 20.
    Shot20 {
        /// Non-zero while waiting (h).
        busy, set_busy, BUSY: u8 @ 0x1d;
        /// Blow object (16-bit pointer).
        blow, set_blow, BLOW: u16 @ 0x42;
    }
}
