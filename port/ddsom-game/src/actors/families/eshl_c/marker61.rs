use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::records::Host;
use crate::actors::families::shell::tables::{tables, Read};
use ddsom_data::effects::Table;

const ANIMS: RomAddr = RomAddr(0x171374);
const LINK: u32 = 0x5c;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    match x.u8(Obj::STATE) {
        0 => {
            if x.u16(LINK) == 0 {
                return;
            }
            let maker = x.linked(LINK);
            let kind = maker.u8(x.b, Obj::VARIANT) as usize;
            let t = tables();
            let Some(table) = [&t.marker61_offsets_0, &t.marker61_offsets_1, &t.marker61_offsets_2].get(kind).copied() else {
                return;
            };
            x.add8(Obj::STATE, 2);
            x.set8(Base::BOX_SET, 7);
            x.set8(Shot::ATTACK_ID, 0x27);
            place(x, table);
            x.anim_start(ANIMS.0, 0);
            x.reinsert();
        }
        2 => {
            if x.u16(LINK) == 0 {
                return;
            }
            let maker = x.linked(LINK);
            if maker.u8(x.b, Obj::VARIANT) > 2 || x.u8(Base::SUB_STATE) != 0 {
                return;
            }
            if x.anim_step() {
                x.add8(Obj::STATE, 2);
                let maker = x.linked(LINK);
                maker.set_u8(x.b, Host::SHOT_MARK, 1);
            }
        }
        _ => x.free_eshl(),
    }
}

/// Settles on the creator with the offset from the table (mirrored according to the creator's orientation).
fn place<R: Bench>(x: &mut Cx<'_, R>, table: &Table) {
    if x.u16(LINK) == 0 {
        return;
    }
    let maker = x.linked(LINK);
    copy_xyz16(x, maker);
    let v = x.u8(Obj::VARIANT) as u32;
    let row = 20 * v + 4 * x.u8(Base::SETTING) as u32;
    let (mut dx, dy) = (table.at16(x.b, row), table.at16(x.b, row + 2));
    if maker.u8(x.b, Base::FLIP) != 0 {
        dx = neg(dx);
    }
    x.add16(Obj::X, dx);
    x.add16(Obj::Y, dy);
}
