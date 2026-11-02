use super::*;

const ANIMS: u32 = 0x16e752;
const ATTACH: u32 = 0xa113a;
const LOOP: u8 = 0;
const END: u8 = 2;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => init(x),
        2 => active(x),
        _ => {
            crate::sound::play(x.b, crate::sound::CMD_37A, x.o);
            x.free_eshl()
        }
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(GAME_PAUSED) != 0 {
        return;
    }
    x.add8(0x4, 2);
    attach(x, ATTACH);
    x.set8(f::BOX_SET, 7);
    x.set8(f::ATTACK_ID, 0xb);
    x.set8(0x42, 0x4f);
    x.set8(0x1e, 0);
    x.anim_start(ANIMS, 2);
    crate::sound::play(x.b, crate::sound::CMD_379, x.o);
}

fn burn<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0x1e, 1);
    kill(x);
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(GAME_PAUSED) != 0 {
        return;
    }
    if x.a5_u8(0x4e) != 0 {
        return burn(x);
    }
    let weapon = x.linked(f::OWNER);
    let host = link_of(x, weapon, f::OWNER);
    if host.u8(x.b, 0xb2) != 0 || suspended(x) {
        return burn(x);
    }
    match x.u8(0x5) {
        LOOP => {
            attach(x, ATTACH);
            if !x.anim_step() {
                return x.reinsert();
            }
            x.add8(0x42, 0xff);
            if (x.u8(0x42) as i8) < 0 {
                let effect = x.linked(0x46);
                effect.set_u8(x.b, 0x43, 1);
                x.add8(0x5, 2);
                x.anim_start(ANIMS, 1);
            } else {
                if x.u8(0x46) == 0 {
                    if let Some(e) = x.alloc(&pools::EFFCT) {
                        e.set_u8(x.b, 0, 1);
                        e.set_u8(x.b, 3, 0);
                        e.set_u8(x.b, 2, 0x18);
                        e.set_u16(x.b, f::OWNER, low16(x.o));
                        x.set16(0x46, low16(e));
                    }
                }
                x.anim_start(ANIMS, 0);
            }
        }
        END => {
            attach(x, ATTACH);
            if x.anim_step() {
                burn(x);
            }
        }
        _ => {}
    }
    x.reinsert();
}
