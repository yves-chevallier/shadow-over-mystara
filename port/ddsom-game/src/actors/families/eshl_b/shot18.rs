use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::vars;
use crate::actors::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x16e534);

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set8(Base::BOX_SET, 7);
            x.set8(Shot::ATTACK_POWER, 6);
            x.set8(Shot::ATTACK_ID, 0x1a);
            let angle = if x.u8(Base::FLIP) != 0 { 8 } else { 0x18 };
            x.set8(Obj::SPEED_INDEX, angle);
            let level = vars::RANK.get(x.b);
            let table = tables().shot18_speed_tables.at8(x.b, level as i16 as i32 as u32);
            speed_table(x, level as u32 & 0xff00 | table as u32);
            x.anim_start(ANIMS.0, 7);
        }
        2 => fly(x),
        _ => x.free_eshl(),
    }
}

fn fly<R: Bench>(x: &mut Cx<'_, R>) {
    let hit = x.u8(Shot::HIT_FLAGS);
    if hit & 0x08 != 0 {
        crate::services::sound::play(x.b, crate::services::sound::CMD_122_222, x.o);
        return x.free_eshl();
    }
    if hit & 0x20 != 0 {
        x.add8(Obj::STATE, 2);
        return x.free_eshl();
    }
    if blocked_by_terrain(x) {
        return x.free_eshl();
    }
    x.move_by_table_xy();
    x.anim_step();
    x.cull_offscreen();
    x.reinsert();
}
