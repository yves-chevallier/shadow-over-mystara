use super::*;

const ANIMS: u32 = 0x16f174;
const SOUND_START: crate::sound::Sound = crate::sound::CMD_32E;
const SOUND_END: crate::sound::Sound = crate::sound::CMD_337;
const OWNER_ARMED: u32 = 0xaf;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => init(x),
        2 => active(x),
        4 => {
            crate::sound::play(x.b, SOUND_END, x.o);
            x.free_eshl()
        }
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(f::OWNER) == 0 {
        return;
    }
    x.add8(0x4, 2);
    let owner = x.linked(f::OWNER);
    copy_position16(x, owner);
    let flip = owner.u8(x.b, f::FLIP) ^ 0x20;
    x.set8(f::FLIP, flip);
    x.set8(f::ATTACK_ID, 5);
    x.set8(f::ATTACK_POWER, 0xc);
    x.set8(f::BOX_SET, 7);
    x.add16(0xc, 0x27);
    let dx = if flip != 0 { -0x2di16 } else { 0x2d };
    x.add16(0x8, dx as u16);
    x.anim_start(ANIMS, 0);
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(f::OWNER) == 0 {
        return;
    }
    let owner = x.linked(f::OWNER);
    if (owner.u8(x.b, 0x4) as i8) >= 4 || owner.u16(x.b, 0x4) == 0x20a {
        crate::sound::play(x.b, SOUND_END, x.o);
        return x.free_eshl();
    }
    match x.u8(0x5) {
        0 => {
            if owner.u8(x.b, OWNER_ARMED) != 0 {
                x.add8(0x5, 2);
            }
            x.anim_step();
            x.reinsert();
        }
        2 => {
            x.add8(0x5, 2);
            crate::sound::play(x.b, SOUND_START, x.o);
            x.set16(0x58, 0x30);
            x.anim_start(ANIMS, 1);
        }
        4 => {
            x.add16(0x58, 0xffff);
            if (x.u16(0x58) as i16) < 0 {
                x.add8(0x5, 2);
                x.anim_start(ANIMS, 2);
            } else {
                x.anim_step();
                x.reinsert();
            }
        }
        6 => {
            if x.anim_step() {
                crate::sound::play(x.b, SOUND_END, x.o);
                x.add8(0x4, 4);
                x.set8(0x5, 0);
                let owner = x.linked(f::OWNER);
                owner.set_u8(x.b, OWNER_ARMED, 0);
            }
        }
        _ => {}
    }
}
