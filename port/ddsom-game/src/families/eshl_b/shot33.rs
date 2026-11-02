use super::*;

const ANIMS: u32 = 0x16f8ea;
const SOUND_FIRE: crate::sound::Sound = crate::sound::CMD_32E;
const SOUND_END: crate::sound::Sound = crate::sound::CMD_337;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    if host_dead(x) {
        crate::sound::play(x.b, SOUND_END, x.o);
        return x.free_eshl();
    }
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set8(f::BOX_SET, 7);
            x.set8(f::ATTACK_POWER, 0xf);
            ai_start(x, ANIMS, 0);
        }
        2 => match x.u8(0x5) {
            0 => {
                if x.u8(0x58) == 0 {
                    ai_step(x);
                } else {
                    x.add8(0x5, 2);
                    crate::sound::play(x.b, SOUND_FIRE, x.o);
                    ai_start(x, ANIMS, 1);
                }
            }
            2 => {
                if x.u8(0x59) == 0 {
                    ai_step(x);
                } else {
                    x.add8(0x5, 2);
                    x.set8(0x6, 0);
                    ai_start(x, ANIMS, 2);
                }
            }
            4 => match x.u8(0x6) {
                0 => {
                    ai_step(x);
                    if x.u16(0x32) != 0 {
                        x.add8(0x6, 2);
                        x.set8(0x5a, 1);
                    }
                }
                2 => {
                    if ai_step(x) {
                        x.add8(0x4, 2);
                    }
                }
                _ => {}
            },
            _ => {}
        },
        _ => {
            crate::sound::play(x.b, SOUND_END, x.o);
            return x.free_eshl();
        }
    }
    if x.u8(0x0) != 0 {
        x.reinsert();
    }
}
