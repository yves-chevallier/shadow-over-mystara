use super::*;
use crate::families::shell::tables::{tables, Read};

const ANIMS: u32 = 0x16da84;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set8(f::ATTACK_POWER, 0x10);
            x.set8(f::ATTACK_ID, 1);
            x.set8(f::BOX_SET, 0x40);
            x.add32(0x10, u32::MAX);
            // Original glitch: the placement row is the low word of D5, left by the previous handler
            let row = *x.c.d5() as u16 as i16 as i32 as u32;
            let (z, dx) = (tables().shot10_placement.at16(x.b, row), tables().shot10_placement.at16(x.b, row + 2));
            x.set16(0xc, z);
            let flip = x.u8(f::FLIP) != 0;
            x.add16(0x8, if flip { dx } else { dx.wrapping_neg() });
            x.anim_start(ANIMS, 0);
        }
        2 => {
            let owner = x.linked(f::OWNER);
            if owner.u8(x.b, 0x4) as i8 >= 4 {
                return x.free_eshl();
            }
            match x.u8(0x5) {
                2 => {
                    x.add8(0x5, 2);
                    x.set16(0x52, 0xf0);
                    x.anim_start(ANIMS, 1);
                }
                0 | 1 => {
                    x.anim_step();
                    x.reinsert();
                }
                _ => {
                    let t = x.u16(0x52).wrapping_sub(1);
                    x.set16(0x52, t);
                    if (t as i16) < 0 {
                        x.add8(0x4, 2);
                    }
                    x.anim_step();
                    x.reinsert();
                }
            }
        }
        _ => x.free_eshl(),
    }
}
