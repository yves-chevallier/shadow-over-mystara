use super::*;

const ANIMS: u32 = 0x170c3e;
const SOUND: crate::sound::Sound = crate::sound::CMD_37B_47B;
const X_START: u32 = 0x58;
const Y_START: u32 = 0x5a;
const TIMER: u32 = 0x56;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            spawn(x);
            x.reinsert();
            crate::sound::play(x.b, SOUND, x.o);
        }
        2 => {
            if x.a5_u8(GAME_PAUSED) == 0 {
                match x.u8(0x5) {
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
    x.add8(0x4, 2);
    x.set8(f::BOX_SET, 7);
    x.set8(f::ATTACK_ID, 0x2d);
    let (px, py) = (x.u16(X_START), x.u16(Y_START));
    x.set16(0x8, px);
    x.set16(0x10, py);
    let top = screen_top(x);
    x.set16(0xc, top.wrapping_add(0x100).wrapping_sub(py));
    x.set16(0x14, neg(0x800));
    x.set16(0x18, neg(0xc00));
    let mut dx = 0x80;
    if x.u8(0x3) == 0 {
        dx = neg(dx);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        let vx = neg(x.u16(0x14));
        x.set16(0x14, vx);
    }
    x.add16(0x8, dx);
    x.anim_start(ANIMS, 0);
}

fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    if x.move_xz().gt() {
        return;
    }
    x.add8(0x5, 2);
    x.set32(0xc, 0);
    x.set16(TIMER, 0x30);
    x.anim_start(ANIMS, 1);
}

fn land<R: Bench>(x: &mut Cx<'_, R>) {
    let t = x.u16(TIMER).wrapping_sub(1);
    x.set16(TIMER, t);
    if t == 0 {
        x.add8(0x4, 2);
    }
    if s16(x.u16(TIMER)) <= 0x20 {
        x.set_bit(0x1, 1, true);
        let visible = x.u16(TIMER).wrapping_add(x.counter()) & 1 != 0;
        if visible {
            x.set_bit(0x1, 1, false);
        }
    }
    x.anim_step();
}
