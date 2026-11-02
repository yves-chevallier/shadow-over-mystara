use super::*;
use crate::families::shell::tables::{tables, Read};

const ANIMS: u32 = 0x1649d0;
const FLY: u8 = 0;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => init(x),
        2 => {
            if x.u8(0x5) == FLY {
                x.move_xz();
                if (x.u16(0xc) as i16) > 0 {
                    return;
                }
                x.set32(0xc, 0);
                x.add8(0x5, 2);
            } else {
                let hidden = !x.bit(0x1, 1);
                x.set_bit(0x1, 1, hidden);
                x.add8(0x54, 0xff);
                if x.u8(0x54) == 0 {
                    x.free_eshl();
                }
            }
        }
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    x.set8(0x54, 0x1e);
    x.set8(f::ATTACK_ID, 0x1a);
    x.set8(f::ATTACK_POWER, 5);
    let row = (x.u8(0x2f) << 2) as u32;
    let mut dx = tables().shot22_offsets.at16(x.b, row);
    let dz = tables().shot22_offsets.at16(x.b, row + 2);
    x.add16(0xc, dz);
    x.add16(0x10, 0xffff);
    x.add16(0xc, 1);
    let variant = x.u8(0x3);
    let row = (variant as u32) << 3;
    let vx = tables().shot22_speeds.at32(x.b, row);
    x.set32(0x14, vx);
    let vz = tables().shot22_speeds.at32(x.b, row + 4);
    x.set32(0x18, vz);
    if x.u8(f::FLIP) != 0 {
        dx = dx.wrapping_neg();
        x.set32(0x14, vx.wrapping_neg());
    }
    x.add16(0x8, dx);
    x.anim_start(ANIMS, (variant.wrapping_add(10) as u32) as u16);
    x.reinsert();
}
