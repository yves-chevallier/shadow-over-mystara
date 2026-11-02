use super::*;
use crate::families::shell::tables::{tables, Read};
use ddsom_data::effects::Table;

const ANIMS: u32 = 0x171374;
const LINK: u32 = 0x5c;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    match x.u8(0x4) {
        0 => {
            if x.u16(LINK) == 0 {
                return;
            }
            let maker = x.linked(LINK);
            let kind = maker.u8(x.b, 0x3) as usize;
            let t = tables();
            let Some(table) = [&t.marker61_offsets_0, &t.marker61_offsets_1, &t.marker61_offsets_2].get(kind).copied() else {
                return;
            };
            x.add8(0x4, 2);
            x.set8(f::BOX_SET, 7);
            x.set8(f::ATTACK_ID, 0x27);
            place(x, table);
            x.anim_start(ANIMS, 0);
            x.reinsert();
        }
        2 => {
            if x.u16(LINK) == 0 {
                return;
            }
            let maker = x.linked(LINK);
            if maker.u8(x.b, 0x3) > 2 || x.u8(0x5) != 0 {
                return;
            }
            if x.anim_step() {
                x.add8(0x4, 2);
                let maker = x.linked(LINK);
                maker.set_u8(x.b, 0x1c, 1);
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
    let v = x.u8(0x3) as u32;
    let row = 20 * v + 4 * x.u8(0x2f) as u32;
    let (mut dx, dy) = (table.at16(x.b, row), table.at16(x.b, row + 2));
    if maker.u8(x.b, f::FLIP) != 0 {
        dx = neg(dx);
    }
    x.add16(0x8, dx);
    x.add16(0x10, dy);
}
