use super::*;

const ANIMS: u32 = 0x16ee30;
const HEIGHTS: u32 = 0xa1d18;
const ANGLES: u32 = 0xa1d22;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set8(f::BOX_SET, 7);
            x.set8(f::ATTACK_ID, 0x2e);
            let source = x.linked(0x5c);
            for off in [0x8, 0xc, 0x10] {
                let v = source.u16(x.b, off);
                x.set16(off, v);
            }
            let row = (source.u8(x.b, 0x60) & 0xf) as u32;
            let dz = x.rom16(HEIGHTS + 2 * row);
            x.add16(0xc, dz);
            x.add16(0x10, 0xffff);
            let variant = x.u8(0x3);
            let angle = x.rom8(ANGLES + (variant & 3) as u32);
            x.set8(0x2d, angle);
            speed_table(x, 0x3f);
            x.anim_start(ANIMS, (variant as u32) as u16);
            x.reinsert();
        }
        2 => {
            x.move_by_table_xy();
            if x.a5_u8(0x6f).wrapping_add(x.u8(0x3)) & 3 == 0 {
                x.cull_offscreen();
            }
            x.reinsert();
        }
        4 => x.add8(0x4, 2),
        _ => x.free_eshl(),
    }
}
