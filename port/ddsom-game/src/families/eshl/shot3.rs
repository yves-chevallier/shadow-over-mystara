use super::*;
use crate::families::shell::tables::{tables, Read};

const ANIMS: u32 = 0x16cb10;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(f::OWNER) == 0 {
        return;
    }
    let owner = x.linked(f::OWNER);
    if owner.u8(x.b, 0x4) as i8 >= 4 {
        return x.free_eshl();
    }
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set8(f::BOX_SET, 7);
            x.set8(f::ATTACK_POWER, 0);
            x.set8(f::ATTACK_ID, 0x1b);
            place(x, owner);
            x.anim_start(ANIMS, 0);
            x.reinsert();
        }
        2 => {
            x.move_xy();
            if x.anim_step() {
                x.add8(0x4, 2);
            }
        }
        _ => x.free_eshl(),
    }
}

fn place<R: Bench>(x: &mut Cx<'_, R>, owner: Obj) {
    copy_position(x, owner);
    let flip = owner.u8(x.b, f::FLIP);
    x.set8(f::FLIP, flip);
    let row = 8 * x.u8(0x3) as u32;
    let (dx, dz) = (tables().shot3_fan.at16(x.b, row), tables().shot3_fan.at16(x.b, row + 2));
    x.add16(0x8, dx);
    x.add16(0xc, dz);
    let (vx, vy) = (tables().shot3_fan.at16(x.b, row + 4), tables().shot3_fan.at16(x.b, row + 6));
    x.set16(0x14, vx);
    x.set16(0x1c, vy);
    for off in [0x16, 0x1a, 0x18, 0x1e] {
        x.set16(off, 0);
    }
}
