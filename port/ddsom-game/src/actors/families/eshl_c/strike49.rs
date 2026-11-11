use super::*;
use crate::world::data::RomAddr;

const ANIMS: RomAddr = RomAddr(0x170468);
const HOST: u32 = 0x42;
const SOUND: crate::hw::sound::Sound = crate::hw::sound::CMD_35B_45B;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set8(Base::BOX_SET, 7);
            x.set8(Shot::ATTACK_POWER, 0x14);
            let host = x.linked(HOST);
            let px = host.u16(x.b, Obj::X);
            x.set16(Obj::X, px);
            x.set16(Obj::Z, 0x35);
            let py = host.u16(x.b, Obj::Y);
            x.set16(Obj::Y, py);
            x.add32(Obj::Y, 2);
            let flip = copy_flip(x, host);
            x.add16(Obj::X, if flip != 0 { 0x6e } else { neg(0x6e) });
            crate::hw::sound::play(x.b, SOUND, x.o);
            ai_anim(x, ANIMS.0, 0);
        }
        2 => match x.u8(Base::SUB_STATE) {
            0 => {
                let host = x.linked(HOST);
                if host.u8(x.b, Base::SUB_STEP) == 6 {
                    return finish(x);
                }
                if host.u8(x.b, Obj::STATE) == 4 {
                    if x.u8(Base::ANIM_EVENT) != 0 {
                        finish(x);
                    } else {
                        x.free_eshl();
                    }
                } else {
                    ai_step(x);
                }
            }
            2 => {
                if ai_step(x) {
                    x.free_eshl();
                }
            }
            _ => {}
        },
        _ => x.free_eshl(),
    }
    if x.u8(Obj::ACTIVE) != 0 {
        x.reinsert();
    }
}

/// Goes to the final animation.
fn finish<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Base::SUB_STATE, 2);
    x.set8(Base::STEP, 0);
    ai_anim(x, ANIMS.0, 2);
}
