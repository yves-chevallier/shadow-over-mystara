use super::*;
use crate::data::RomAddr;
use crate::families::shell::vars;
use crate::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x16fbf4);
const DELAY: u32 = 0x58;
const WAIT: u8 = 0;
const PLAY: u8 = 2;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set8(Base::BOX_SET, 7);
            x.set8(Shot::ATTACK_POWER, 0x1e);
            x.set_bit(Obj::FLAGS, 1, true);
            place(x);
            let r = random(x);
            let delay = tables().spawn47_delays.at16(x.b, 2 * (r & 0xf));
            x.set16(DELAY, delay);
            ai_anim(x, ANIMS.0, 1);
        }
        2 => match x.u8(Base::SUB_STATE) {
            WAIT => {
                let t = x.u16(DELAY).wrapping_sub(1);
                x.set16(DELAY, t);
                if s16(t) <= 0 {
                    x.add8(Base::SUB_STATE, 2);
                    x.set_bit(Obj::FLAGS, 1, false);
                }
            }
            PLAY => {
                if ai_step(x) {
                    x.add8(Obj::STATE, 2);
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

/// Offsets the position at random: brings it back toward the screen if it is too far from it.
fn place<R: Bench>(x: &mut Cx<'_, R>) {
    let idx = 2 * (random(x) & 0xf);
    let cam = vars::CAMERA1_X.get(x.b);
    let (lo, hi) = (cam.wrapping_add(0x20), cam.wrapping_add(0x160));
    let px = x.u16(Obj::X);
    let dx = if s16(lo) >= s16(px) {
        tables().spawn47_x_outside.at16(x.b, idx)
    } else if s16(hi) <= s16(px) {
        neg(tables().spawn47_x_outside.at16(x.b, idx))
    } else {
        tables().spawn47_x_inside.at16(x.b, idx)
    };
    x.add16(Obj::X, dx);
    let idx = 2 * (random(x) & 0xf);
    let cam = vars::CAMERA1_Y.get(x.b);
    let (lo, hi) = (cam.wrapping_add(0x38), cam.wrapping_add(0x68));
    let py = x.u16(Obj::Y);
    let dy = if s16(lo) >= s16(py) {
        tables().spawn47_y_outside.at16(x.b, idx)
    } else if s16(hi) <= s16(py) {
        neg(tables().spawn47_y_outside.at16(x.b, idx))
    } else {
        tables().spawn47_y_inside.at16(x.b, idx)
    };
    x.add16(Obj::Y, dy);
}
