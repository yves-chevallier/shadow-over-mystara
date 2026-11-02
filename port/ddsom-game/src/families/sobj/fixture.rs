use super::*;

const ANIMS: u32 = 0x1c40c6;
/// Variant -> shape of the behavior at creation (0: none, 1: saves X/Y).
const INIT_KIND: u32 = 0xddc6e;
/// Variant -> shape of the per-frame behavior (0: screen exit, 1: scrolling, 2: following).
const STEP_KIND: u32 = 0xddd16;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            let variant = x.u8(0x3) as u32;
            x.anim_start(ANIMS, variant as u16);
            x.reinsert();
            if x.rom8(INIT_KIND + variant) == 1 {
                let v = x.u16(0x8);
                x.set16(0xa, v);
                let v = x.u16(0x10);
                x.set16(0x12, v);
            }
        }
        2 => {
            if x.u8(0x2f) == 0 {
                x.anim_step();
            }
            let variant = x.u8(0x3) as u32;
            match x.rom8(STEP_KIND + variant) {
                0 => cull_beyond(x, 0x60),
                1 => {
                    let (dx, dy) = camera_delta(x);
                    x.add32(0x8, asr(dx, 1));
                    x.add32(0xc, asr(dy, 1));
                }
                _ => {
                    let d = x.a5_u16(ram::CAMERA2_X).wrapping_sub(x.a5_u16(ram::CAMERA_X));
                    let v = x.u16(0xa).wrapping_sub(d);
                    x.set16(0x8, v);
                    let d = x.a5_u16(ram::CAMERA2_Y).wrapping_sub(x.a5_u16(ram::CAMERA_Y));
                    let v = x.u16(0x12).wrapping_sub(d);
                    x.set16(0x10, v);
                    cull_beyond(x, 0x60);
                }
            }
        }
        _ => x.free(&pools::SOBJ),
    }
}
