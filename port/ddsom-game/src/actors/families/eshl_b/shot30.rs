use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::records::view;

const ANIMS: RomAddr = RomAddr(0x16f174);
const SOUND_START: crate::hw::sound::Sound = crate::hw::sound::CMD_32E;
const SOUND_END: crate::hw::sound::Sound = crate::hw::sound::CMD_337;
const OWNER_ARMED: u32 = 0xaf;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => init(x),
        2 => active(x),
        4 => {
            crate::hw::sound::play(x.b, SOUND_END, x.o);
            x.free_eshl()
        }
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(Shot::OWNER) == 0 {
        return;
    }
    x.add8(Obj::STATE, 2);
    let owner = x.linked(Shot::OWNER);
    copy_position16(x, owner);
    let flip = owner.u8(x.b, Base::FLIP) ^ 0x20;
    x.set8(Base::FLIP, flip);
    x.set8(Shot::ATTACK_ID, 5);
    x.set8(Shot::ATTACK_POWER, 0xc);
    x.set8(Base::BOX_SET, 7);
    x.add16(Obj::Z, 0x27);
    let dx = if flip != 0 { -0x2di16 } else { 0x2d };
    x.add16(Obj::X, dx as u16);
    x.anim_start(ANIMS.0, 0);
}

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(Shot::OWNER) == 0 {
        return;
    }
    let owner = x.linked(Shot::OWNER);
    if (owner.u8(x.b, Obj::STATE) as i8) >= 4 || owner.u16(x.b, Obj::STATE) == 0x20a {
        crate::hw::sound::play(x.b, SOUND_END, x.o);
        return x.free_eshl();
    }
    match x.u8(Base::SUB_STATE) {
        0 => {
            if owner.u8(x.b, OWNER_ARMED) != 0 {
                x.add8(Base::SUB_STATE, 2);
            }
            x.anim_step();
            x.reinsert();
        }
        2 => {
            x.add8(Base::SUB_STATE, 2);
            crate::hw::sound::play(x.b, SOUND_START, x.o);
            x.set16(Shot30::TIMER, 0x30);
            x.anim_start(ANIMS.0, 1);
        }
        4 => {
            x.add16(Shot30::TIMER, 0xffff);
            if (x.u16(Shot30::TIMER) as i16) < 0 {
                x.add8(Base::SUB_STATE, 2);
                x.anim_start(ANIMS.0, 2);
            } else {
                x.anim_step();
                x.reinsert();
            }
        }
        6 => {
            if x.anim_step() {
                crate::hw::sound::play(x.b, SOUND_END, x.o);
                x.add8(Obj::STATE, 4);
                x.set8(Base::SUB_STATE, 0);
                let owner = x.linked(Shot::OWNER);
                owner.set_u8(x.b, OWNER_ARMED, 0);
            }
        }
        _ => {}
    }
}

view! {
    /// Fields of shot 30.
    Shot30 {
        /// Countdown (h).
        timer, set_timer, TIMER: u16 @ 0x58;
    }
}
