use super::*;

const PALETTE: u32 = 0x28ff20;
const PALETTE_DST: u32 = 0x91c3c0;
const SLIDE_ANIMS: u32 = 0x1c40c6;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x4) == 0 {
        x.add8(0x4, 2);
        copy_longs(x, PALETTE, PALETTE_DST, 8);
        or_longs(x, PALETTE_DST, BRIGHT, 8);
        x.set8(0x2b, 0x1e);
        x.set8(0x2a, 1);
        let v = x.a5_u16(ram::CAMERA_X).wrapping_add(0x20);
        x.set16(0x8, v);
        x.set16(0xc, 0x1c8);
        x.set16(0x10, 0xff00);
        x.anim_start(SLIDE_ANIMS, 0x3b);
        x.reinsert();
    }
    let d = x.a5_u16(ram::CAMERA_X).wrapping_sub(x.a5_u16(ram::PREV_CAMERA_X));
    x.add16(0x8, d);
    if !x.anim_step() {
        return;
    }
    for i in 0..4 {
        let at = PLAYERS + 2 * i;
        let who = rd16(x.b, at);
        if who != 0 {
            wr16(x.b, who as i16 as i32 as u32 + 4, 0x400);
            wr16(x.b, at, 0);
        }
    }
    x.free(&pools::SOBJ);
}
