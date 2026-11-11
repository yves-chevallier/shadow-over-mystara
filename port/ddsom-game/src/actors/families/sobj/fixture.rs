use super::*;
use crate::world::data::RomAddr;
use crate::world::ram;
use crate::actors::families::shell::records::Base;
use crate::actors::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x1c40c6);

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            let variant = x.u8(Obj::VARIANT) as u32;
            x.anim_start(ANIMS.0, variant as u16);
            x.reinsert();
            if tables().sobj_fixture_init_kind.at8(x.b, variant) == 1 {
                let v = x.u16(Obj::X);
                x.set16(Base::X_FRACTION, v);
                let v = x.u16(Obj::Y);
                x.set16(Base::Y_FRACTION, v);
            }
        }
        2 => {
            if x.u8(Base::SETTING) == 0 {
                x.anim_step();
            }
            let variant = x.u8(Obj::VARIANT) as u32;
            match tables().sobj_fixture_step_kind.at8(x.b, variant) {
                0 => cull_beyond(x, 0x60),
                1 => {
                    let (dx, dy) = camera_delta(x);
                    x.add32(Obj::X, asr(dx, 1));
                    x.add32(Obj::Z, asr(dy, 1));
                }
                _ => {
                    let d = ram::CAMERA_ALT_X.get(x.b).wrapping_sub(ram::CAMERA_X.get(x.b));
                    let v = x.u16(Base::X_FRACTION).wrapping_sub(d);
                    x.set16(Obj::X, v);
                    let d = ram::CAMERA_ALT_Y.get(x.b).wrapping_sub(ram::CAMERA_Y.get(x.b));
                    let v = x.u16(Base::Y_FRACTION).wrapping_sub(d);
                    x.set16(Obj::Y, v);
                    cull_beyond(x, 0x60);
                }
            }
        }
        _ => x.free(&pools::SOBJ),
    }
}
