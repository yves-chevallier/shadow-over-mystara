use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x16cb10);

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(Shot::OWNER) == 0 {
        return;
    }
    let owner = x.linked(Shot::OWNER);
    if owner.u8(x.b, Obj::STATE) as i8 >= 4 {
        return x.free_eshl();
    }
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set8(Base::BOX_SET, 7);
            x.set8(Shot::ATTACK_POWER, 0);
            x.set8(Shot::ATTACK_ID, 0x1b);
            place(x, owner);
            x.anim_start(ANIMS.0, 0);
            x.reinsert();
        }
        2 => {
            x.move_xy();
            if x.anim_step() {
                x.add8(Obj::STATE, 2);
            }
        }
        _ => x.free_eshl(),
    }
}

fn place<R: Bench>(x: &mut Cx<'_, R>, owner: Obj) {
    copy_position(x, owner);
    let flip = owner.u8(x.b, Base::FLIP);
    x.set8(Base::FLIP, flip);
    let row = 8 * x.u8(Obj::VARIANT) as u32;
    let (dx, dz) = (tables().shot3_fan.at16(x.b, row), tables().shot3_fan.at16(x.b, row + 2));
    x.add16(Obj::X, dx);
    x.add16(Obj::Z, dz);
    let (vx, vy) = (tables().shot3_fan.at16(x.b, row + 4), tables().shot3_fan.at16(x.b, row + 6));
    x.set16(Obj::VX, vx);
    x.set16(Obj::VY, vy);
    for off in [0x16, 0x1a, 0x18, 0x1e] {
        x.set16(off, 0u16);
    }
}
