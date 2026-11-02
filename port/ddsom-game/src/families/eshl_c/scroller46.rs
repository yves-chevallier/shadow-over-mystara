use super::*;

const ANIMS: u32 = 0x1703e6;
/// Left edge of the screen (variable `A5+0x506`).
const SCREEN_LEFT: u32 = 0x506;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            let variant = x.u8(0x3) as u32;
            x.anim_start(ANIMS, variant as u16);
            x.reinsert();
        }
        2 => {
            x.add16(0x8, neg(4));
            let edge = x.a5_u16(SCREEN_LEFT).wrapping_sub(0x50);
            if s16(edge) > s16(x.u16(0x8)) {
                x.free_eshl();
            }
        }
        _ => x.free_eshl(),
    }
}
