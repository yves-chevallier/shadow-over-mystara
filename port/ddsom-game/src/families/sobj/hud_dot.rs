use super::*;
use crate::families::shell::records::view;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set16(Special::CAMERA_DZ, 0x30);
            x.set32(Obj::Y, 0);
            let camera = camera_record(x);
            x.set16(HudDot::CAMERA, camera);
            stick_to_camera(x);
            // `move.w #0x1ab,D0w ; add.b (3,A0),D0b`: the addition does not overflow into the high byte
            let n = 0x100 | (0xab + x.u8(Obj::VARIANT) as u32) & 0xff;
            x.anim_start(ANIMS.0, n as u16);
            x.reinsert();
        }
        2 => stick_to_camera(x),
        4 => {
            x.set32(Obj::STATE, 0x0600_0000);
            x.free(&pools::SOBJ);
        }
        _ => x.free(&pools::SOBJ),
    }
}

view! {
    /// Fields of the interface dot.
    HudDot {
        /// Camera it sticks to (16-bit pointer).
        camera, set_camera, CAMERA: u16 @ 0x42;
    }
}
