use super::*;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => x.add8(0x4, 2),
        2 => {
            let y = x.a5_u32(ram::CAMERA2_Y).wrapping_add(0x10000);
            x.set_a5_u32(ram::CAMERA2_Y, y);
            let target = x.u16(0x42);
            if target as i16 > (y >> 16) as i16 {
                return;
            }
            x.set_a5_u16(ram::CAMERA2_Y, target);
            if x.u16(0x46) != 0 {
                let linked = x.linked(0x46);
                linked.set_u8(x.b, 0x2e, 0);
                let n = linked.u16(x.b, 0x2a).wrapping_sub(1);
                linked.set_u16(x.b, 0x2a, n);
                crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::A, 0x10);
            }
            x.free(&pools::SET);
        }
        _ => x.free(&pools::SET),
    }
}
