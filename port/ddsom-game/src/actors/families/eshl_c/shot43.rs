use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x170222);

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set8(Base::BOX_SET, 7);
            x.select_speed(&crate::world::objects::motion::SPEEDS_B, 0x40);
            let dir = x.u8(Obj::SPEED_INDEX) as u32 >> 1;
            let extra = tables().shot43_anims_of_direction.at8(x.b, dir);
            ai_anim(x, ANIMS.0, 0x10u32.wrapping_add(extra as u32) & 0xff);
        }
        2 => fly(x),
        _ => x.free_eshl(),
    }
    if x.u8(Obj::ACTIVE) != 0 {
        x.reinsert();
    }
}

fn fly<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Base::SUB_STATE) != 0 {
        return;
    }
    if x.u8(Shot::HIT_FLAGS) & 0x28 != 0 {
        return x.free_eshl();
    }
    x.move_xyz();
    if crate::services::ai_aids::outside_band(x.b, x.o, neg(0x50), 0x1d0) {
        x.free_eshl();
    }
}
