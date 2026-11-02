use super::*;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set16(0x18, 0x30);
            x.set32(0x10, 0);
            let camera = camera_record(x);
            x.set16(0x42, camera);
            stick_to_camera(x);
            // `move.w #0x1ab,D0w ; add.b (3,A0),D0b`: the addition does not overflow into the high byte
            let n = 0x100 | (0xab + x.u8(0x3) as u32) & 0xff;
            x.anim_start(ANIMS, n as u16);
            x.reinsert();
        }
        2 => stick_to_camera(x),
        4 => {
            x.set32(0x4, 0x0600_0000);
            x.free(&pools::SOBJ);
        }
        _ => x.free(&pools::SOBJ),
    }
}
