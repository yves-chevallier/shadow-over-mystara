use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::records::view;

const ANIMS: RomAddr = RomAddr(0x16f8ea);
const SOUND_FIRE: crate::hw::sound::Sound = crate::hw::sound::CMD_32E;
const SOUND_END: crate::hw::sound::Sound = crate::hw::sound::CMD_337;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    if host_dead(x) {
        crate::hw::sound::play(x.b, SOUND_END, x.o);
        return x.free_eshl();
    }
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set8(Base::BOX_SET, 7);
            x.set8(Shot::ATTACK_POWER, 0xf);
            ai_start(x, ANIMS.0, 0);
        }
        2 => match x.u8(Base::SUB_STATE) {
            0 => {
                if x.u8(Shot33::FIRST) == 0 {
                    ai_step(x);
                } else {
                    x.add8(Base::SUB_STATE, 2);
                    crate::hw::sound::play(x.b, SOUND_FIRE, x.o);
                    ai_start(x, ANIMS.0, 1);
                }
            }
            2 => {
                if x.u8(Shot33::SECOND) == 0 {
                    ai_step(x);
                } else {
                    x.add8(Base::SUB_STATE, 2);
                    x.set8(Base::STEP, 0);
                    ai_start(x, ANIMS.0, 2);
                }
            }
            4 => match x.u8(Base::STEP) {
                0 => {
                    ai_step(x);
                    if x.u16(Obj::ANIM_FLAGS) != 0 {
                        x.add8(Base::STEP, 2);
                        x.set8(Shot33::DONE, 1);
                    }
                }
                2 => {
                    if ai_step(x) {
                        x.add8(Obj::STATE, 2);
                    }
                }
                _ => {}
            },
            _ => {}
        },
        _ => {
            crate::hw::sound::play(x.b, SOUND_END, x.o);
            return x.free_eshl();
        }
    }
    if x.u8(Obj::ACTIVE) != 0 {
        x.reinsert();
    }
}

view! {
    /// Fields of shot 33.
    Shot33 {
        /// First flag (h).
        first, set_first, FIRST: u8 @ 0x58;
        /// Second flag (h).
        second, set_second, SECOND: u8 @ 0x59;
        /// Set to 1 at its end (h).
        done, set_done, DONE: u8 @ 0x5a;
    }
}
