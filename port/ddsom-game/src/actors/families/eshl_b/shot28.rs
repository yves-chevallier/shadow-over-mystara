use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::shell::vars;
use crate::actors::families::shell::records::{Spawner, view};
use crate::actors::families::shell::tables::{tables, Read};

const ANIMS: RomAddr = RomAddr(0x16ee30);

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set8(Base::BOX_SET, 7);
            x.set8(Shot::ATTACK_ID, 0x2e);
            let source = x.linked(Shot28::SOURCE);
            for off in [0x8, 0xc, 0x10] {
                let v = source.u16(x.b, off);
                x.set16(off, v);
            }
            let row = (source.u8(x.b, Spawner::ROW) & 0xf) as u32;
            let dz = tables().shot28_heights.at16(x.b, 2 * row);
            x.add16(Obj::Z, dz);
            x.add16(Obj::Y, 0xffff);
            let variant = x.u8(Obj::VARIANT);
            let angle = tables().shot28_angles.at8(x.b, (variant & 3) as u32);
            x.set8(Obj::SPEED_INDEX, angle);
            speed_table(x, 0x3f);
            x.anim_start(ANIMS.0, (variant as u32) as u16);
            x.reinsert();
        }
        2 => {
            x.move_by_table_xy();
            if vars::FRAME_BYTE.get(x.b).wrapping_add(x.u8(Obj::VARIANT)) & 3 == 0 {
                x.cull_offscreen();
            }
            x.reinsert();
        }
        4 => x.add8(Obj::STATE, 2),
        _ => x.free_eshl(),
    }
}

view! {
    /// Fields of shot 28 (a spawner, see `Spawner`).
    Shot28 {
        /// Number of created shots, row of the offsets (h).
        count, set_count, COUNT: u16 @ 0x56;
        /// Countdown of the current step (h).
        timer, set_timer, TIMER: u16 @ 0x58;
        /// Linked object: shadow or second timer (h).
        link, set_link, LINK: u16 @ 0x5a;
        /// Source object (16-bit pointer).
        source, set_source, SOURCE: u16 @ 0x5c;
        /// Row of the height tables (h).
        row, set_row, ROW: u8 @ 0x60;
    }
}
