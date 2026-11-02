use super::*;
use crate::families::shell::tables::{tables, Read};

const ANIMS: u32 = 0x169fb0;
/// Variable `A5+0x6f`: bit 0 = forced animation rate.
const FORCE_ANIM: u32 = 0x6f;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            attach(x);
            x.set32(0xc, 0);
            x.set8(f::BOX_SET, 0x3b);
            x.anim_start(ANIMS, 1);
        }
        2 => {
            if frozen(x) {
                return x.reinsert();
            }
            if x.a5_u8(GAME_PAUSED) == 0 {
                let owner = attach(x);
                let flags = owner.u8(x.b, 0xb0);
                if flags != 0 {
                    let differ = (flags & 1 != 0) != (flags & 2 != 0);
                    if !differ || x.a5_u8(FORCE_ANIM) & 1 != 0 {
                        x.anim_step();
                    }
                }
            }
            x.reinsert();
        }
        4 => {
            let owner = x.linked(f::OWNER);
            if owner.u8(x.b, 0x4) == 4 {
                if !frozen(x) {
                    attach(x);
                }
            } else {
                let v = x.u16(0x54);
                owner.set_u16(x.b, 0x54, v);
                owner.set_u32(x.b, 0x4, 0x0400_0000);
            }
            x.reinsert();
        }
        _ => x.free_eshl(),
    }
}

/// Countdown `+0x53` positive: decremented, and the halo stays frozen.
fn frozen<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let n = x.u8(0x53);
    if (n as i8) > 0 {
        x.set8(0x53, n - 1);
        return true;
    }
    false
}

fn attach<R: Bench>(x: &mut Cx<'_, R>) -> Obj {
    let owner = x.linked(f::OWNER);
    for off in [0x8, 0x10] {
        let v = owner.u32(x.b, off);
        x.set32(off, v);
    }
    let row = (4 * x.u8(0x3) as u32) & 0xff;
    let (dx, dy) = (tables().shot11_offsets.at16(x.b, row), tables().shot11_offsets.at16(x.b, row + 2));
    x.add16(0x8, dx);
    x.add16(0x10, dy);
    owner
}
