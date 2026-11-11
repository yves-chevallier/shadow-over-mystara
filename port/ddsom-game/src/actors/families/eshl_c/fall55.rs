use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::vars;

const ANIMS: RomAddr = RomAddr(0x170c3e);
const SOUND: crate::services::sound::Sound = crate::services::sound::CMD_37B_47B;
const X_START: u32 = 0x58;
const Y_START: u32 = 0x5a;
const TIMER: u32 = 0x56;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            spawn(x);
            x.reinsert();
            crate::services::sound::play(x.b, SOUND, x.o);
        }
        2 => {
            if vars::GAME_PAUSED.get(x.b) == 0 {
                match x.u8(Base::SUB_STATE) {
                    0 => fall(x),
                    2 => land(x),
                    _ => {}
                }
            }
            x.reinsert();
        }
        _ => x.free_eshl(),
    }
}

fn spawn<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set8(Base::BOX_SET, 7);
    x.set8(Shot::ATTACK_ID, 0x2d);
    let (px, py) = (x.u16(X_START), x.u16(Y_START));
    x.set16(Obj::X, px);
    x.set16(Obj::Y, py);
    let top = screen_top(x);
    x.set16(Obj::Z, top.wrapping_add(0x100).wrapping_sub(py));
    x.set16(Obj::VX, neg(0x800));
    x.set16(Obj::VZ, neg(0xc00));
    let mut dx = 0x80;
    if x.u8(Obj::VARIANT) == 0 {
        dx = neg(dx);
        let flip = x.u8(Base::FLIP) ^ 0x20;
        x.set8(Base::FLIP, flip);
        let vx = neg(x.u16(Obj::VX));
        x.set16(Obj::VX, vx);
    }
    x.add16(Obj::X, dx);
    x.anim_start(ANIMS.0, 0);
}

fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    if x.move_xz().gt() {
        return;
    }
    x.add8(Base::SUB_STATE, 2);
    x.set32(Obj::Z, 0);
    x.set16(TIMER, 0x30);
    x.anim_start(ANIMS.0, 1);
}

fn land<R: Bench>(x: &mut Cx<'_, R>) {
    let t = x.u16(TIMER).wrapping_sub(1);
    x.set16(TIMER, t);
    if t == 0 {
        x.add8(Obj::STATE, 2);
    }
    if s16(x.u16(TIMER)) <= 0x20 {
        x.set_bit(Obj::FLAGS, 1, true);
        let visible = x.u16(TIMER).wrapping_add(x.counter()) & 1 != 0;
        if visible {
            x.set_bit(Obj::FLAGS, 1, false);
        }
    }
    x.anim_step();
}
