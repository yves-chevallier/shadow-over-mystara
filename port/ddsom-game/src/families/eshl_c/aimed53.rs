use super::*;
use crate::data::RomAddr;
use crate::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x175dc4);
const LINK: u32 = 0x5c;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => spawn(x),
        2 => {
            if x.u8(Shot::HIT_FLAGS) & 0x28 != 0 {
                return x.free_eshl();
            }
            x.move_by_table_xy();
            x.anim_step();
            x.cull_offscreen();
            x.reinsert();
        }
        _ => x.free_eshl(),
    }
}

fn spawn<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    if x.u16(LINK) == 0 {
        return x.free_eshl();
    }
    x.set8(Base::BOX_SET, 7);
    x.set8(Shot::ATTACK_ID, 0x23);
    let creator = x.linked(LINK);
    if creator.u16(x.b, LINK) == 0 {
        return x.free_eshl();
    }
    copy_xyz16(x, creator);
    let enemy = obj_of(creator.u16(x.b, LINK));
    let target = enemy.u16(x.b, Shot::OWNER);
    if target == 0 {
        return x.free_eshl();
    }
    let dir = crate::aids::direction_to(x.b, x.o, obj_of(target));
    x.set8(Obj::SPEED_INDEX, dir);
    x.select_speed(&crate::objects::motion::SPEEDS_B, 0x24);
    let row = 2 * dir as u32;
    let flip = tables().aimed53_directions.at8(x.b, row + 1);
    x.set8(Base::FLIP, flip);
    let anim = tables().aimed53_directions.at8(x.b, row);
    x.anim_start(ANIMS.0, (anim as u32) as u16);
    x.reinsert();
}
