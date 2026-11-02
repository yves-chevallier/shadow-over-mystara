use super::*;
use crate::data::RomAddr;
use crate::ram;

const ANIMS: RomAddr = RomAddr(0x1703e6);

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            let variant = x.u8(Obj::VARIANT) as u32;
            x.anim_start(ANIMS.0, variant as u16);
            x.reinsert();
        }
        2 => {
            x.add16(Obj::X, neg(4));
            let edge = ram::CAMERA_X.get(x.b).wrapping_sub(0x50);
            if s16(edge) > s16(x.u16(Obj::X)) {
                x.free_eshl();
            }
        }
        _ => x.free_eshl(),
    }
}
