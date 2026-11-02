use super::*;

const ANIMS: u32 = 0x170468;
const HOST: u32 = 0x42;
const SOUND: crate::sound::Sound = crate::sound::CMD_35B_45B;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set8(f::BOX_SET, 7);
            x.set8(f::ATTACK_POWER, 0x14);
            let host = x.linked(HOST);
            let px = host.u16(x.b, 0x8);
            x.set16(0x8, px);
            x.set16(0xc, 0x35);
            let py = host.u16(x.b, 0x10);
            x.set16(0x10, py);
            x.add32(0x10, 2);
            let flip = copy_flip(x, host);
            x.add16(0x8, if flip != 0 { 0x6e } else { neg(0x6e) });
            crate::sound::play(x.b, SOUND, x.o);
            ai_anim(x, ANIMS, 0);
        }
        2 => match x.u8(0x5) {
            0 => {
                let host = x.linked(HOST);
                if host.u8(x.b, 7) == 6 {
                    return finish(x);
                }
                if host.u8(x.b, 4) == 4 {
                    if x.u8(0x33) != 0 {
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
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

/// Goes to the final animation.
fn finish<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x5, 2);
    x.set8(0x6, 0);
    ai_anim(x, ANIMS, 2);
}
