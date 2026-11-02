use super::*;

const ANIM: u32 = 0x43;
const OFFSET_X: u16 = 0x50;
const OFFSET_Y: u16 = 0x58;
const HOST: u32 = 0x14;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set16(0x18, OFFSET_X);
            x.set16(0x1a, OFFSET_Y);
            x.anim_start(ANIMS, ANIM as u16);
            x.reinsert();
        }
        2 => {
            let host = Obj(x.u32(HOST) & 0xff_ffff);
            if host.u8(x.b, 0x4) == 6 {
                x.add8(0x4, 2);
                return;
            }
            let px = host.u16(x.b, 0x8).wrapping_add(x.u16(0x18));
            let py = host.u16(x.b, 0x10).wrapping_sub(x.u16(0x1a));
            x.set16(0x8, px);
            x.set16(0x10, py);
            x.anim_step();
        }
        _ => x.free(&pools::SOBJ),
    }
}
