use super::*;

const ANIMS: u32 = 0x170222;
const ANIM_OF_DIRECTION: u32 = 0xa37ba;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set8(f::BOX_SET, 7);
            x.select_speed(&crate::objects::motion::SPEEDS_B, 0x40);
            let dir = x.u8(0x2d) as u32 >> 1;
            let extra = x.rom8(ANIM_OF_DIRECTION + dir);
            ai_anim(x, ANIMS, 0x10u32.wrapping_add(extra as u32) & 0xff);
        }
        2 => fly(x),
        _ => x.free_eshl(),
    }
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

fn fly<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x5) != 0 {
        return;
    }
    if x.u8(f::HIT_FLAGS) & 0x28 != 0 {
        return x.free_eshl();
    }
    x.move_xyz();
    if crate::ai_aids::outside_band(x.b, x.o, neg(0x50), 0x1d0) {
        x.free_eshl();
    }
}
