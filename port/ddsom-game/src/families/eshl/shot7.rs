use super::*;
use crate::data::RomAddr;
use crate::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x16d1d6);

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            if x.u16(Shot::OWNER) == 0 {
                return;
            }
            x.add8(Obj::STATE, 2);
            x.set8(Base::BOX_SET, 7);
            x.set8(Shot::ATTACK_POWER, 6);
            x.set8(Shot::ATTACK_ID, 0x1c);
            let owner = x.linked(Shot::OWNER);
            copy_position(x, owner);
            x.add16(Obj::Y, 0xffff);
            let row = 4 * x.u8(Obj::VARIANT) as u32;
            let (dx, dy) = (tables().shot7_offsets.at16(x.b, row), tables().shot7_offsets.at16(x.b, row + 2));
            x.add16(Obj::X, dx);
            x.add16(Obj::Y, dy);
            x.anim_start(ANIMS.0, 0);
            x.reinsert();
            wait_end(x);
        }
        2 => wait_end(x),
        _ => x.free_eshl(),
    }
}

fn wait_end<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        x.add8(Obj::STATE, 2);
    }
}
