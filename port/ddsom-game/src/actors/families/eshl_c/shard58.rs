use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x1706f6);
const LINK: u32 = 0x5c;
const TIMER: u32 = 0x42;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(LINK) == 0 {
        return x.set32(Obj::STATE, 0x0600_0000);
    }
    match x.u8(Obj::STATE) {
        0 => init(x),
        2 => {
            if x.u8(Base::SUB_STATE) == 0 {
                let t = x.u16(TIMER).wrapping_sub(1);
                x.set16(TIMER, t);
                if s16(t) < 0 {
                    x.add8(Obj::STATE, 2);
                }
                x.anim_step();
                if x.u8(Base::SETTING) == 0 {
                    x.move_by_table_xy();
                } else {
                    x.move_by_table_xz();
                }
            }
            blink_by_variant(x);
            x.reinsert();
        }
        _ => x.free_eshl(),
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set8(Base::BOX_SET, 7);
    x.set8(Shot::ATTACK_ID, 0x23);
    let maker = x.linked(LINK);
    copy_xyz32(x, maker);
    copy_flip(x, maker);
    x.add16(Obj::Y, 0xffff);
    let flip = x.u8(Base::FLIP) ^ 0x20;
    x.set8(Base::FLIP, flip);
    let row = 8 * x.u8(Obj::VARIANT) as u32;
    let timer = tables().shard58_rows.at16(x.b, row);
    let subtype = tables().shard58_rows.at8(x.b, row + 2);
    let speed_index = tables().shard58_rows.at8(x.b, row + 3);
    let anim = tables().shard58_rows.at8(x.b, row + 4);
    let new_flip = tables().shard58_rows.at8(x.b, row + 5);
    let speeds = tables().shard58_rows.at16(x.b, row + 6);
    x.set16(TIMER, timer);
    x.set8(Obj::SPEED_INDEX, speed_index);
    x.set8(Base::FLIP, new_flip);
    x.set8(Base::SETTING, subtype);
    x.anim_start(ANIMS.0, (anim as u32) as u16);
    x.select_speed(&crate::world::objects::motion::SPEEDS_B, (speeds as u32) as u16);
}
