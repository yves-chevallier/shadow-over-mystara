use super::*;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            let anim = if x.u8(0x3) == 0 { 0x13b } else { 0x13c };
            x.anim_start(ANIMS, anim);
            track(x)
        }
        2 => track(x),
        _ => x.free(&pools::SOBJ),
    }
}

fn track<R: Bench>(x: &mut Cx<'_, R>) {
    let target = x.linked(0x14);
    for off in [0x8, 0xc, 0x10] {
        let v = target.u16(x.b, off);
        x.set16(off, v);
    }
    let lift = if x.u8(0x3) == 0 { 0x20 } else { 0x10 };
    x.add16(0xc, 0u16.wrapping_sub(lift));
    x.add16(0x10, 1);
    let limit = x.a5_u16(ram::CAMERA_X).wrapping_sub(0x40) as i16;
    if limit > x.u16(0x8) as i16 {
        return x.free(&pools::SOBJ);
    }
    x.anim_step();
    x.reinsert();
}
