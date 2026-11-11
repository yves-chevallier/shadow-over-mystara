use super::*;
use crate::actors::families::shell::records::view;

const ANIM: u32 = 0x43;
const OFFSET_X: u16 = 0x50;
const OFFSET_Y: u16 = 0x58;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set16(Tag::OFFSET_X, OFFSET_X);
            x.set16(Tag::OFFSET_Y, OFFSET_Y);
            x.anim_start(ANIMS.0, ANIM as u16);
            x.reinsert();
        }
        2 => {
            let host = Obj(x.u32(Tag::HOST) & 0xff_ffff);
            if host.u8(x.b, Obj::STATE) == 6 {
                x.add8(Obj::STATE, 2);
                return;
            }
            let px = host.u16(x.b, Obj::X).wrapping_add(x.u16(Tag::OFFSET_X));
            let py = host.u16(x.b, Obj::Y).wrapping_sub(x.u16(Tag::OFFSET_Y));
            x.set16(Obj::X, px);
            x.set16(Obj::Y, py);
            x.anim_step();
        }
        _ => x.free(&pools::SOBJ),
    }
}

view! {
    /// Fields of the label stuck to an object.
    Tag {
        /// Object carrying the label (address).
        host, set_host, HOST: u32 @ 0x14;
        /// Offset X from the host.
        offset_x, set_offset_x, OFFSET_X: u16 @ 0x18;
        /// Offset Y from the host (subtracted).
        offset_y, set_offset_y, OFFSET_Y: u16 @ 0x1a;
    }
}
