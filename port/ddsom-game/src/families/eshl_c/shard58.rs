use super::*;

const ANIMS: u32 = 0x1706f6;
const LINK: u32 = 0x5c;
const ROWS: u32 = 0xa5bd6;
const TIMER: u32 = 0x42;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(LINK) == 0 {
        return x.set32(0x4, 0x0600_0000);
    }
    match x.u8(0x4) {
        0 => init(x),
        2 => {
            if x.u8(0x5) == 0 {
                let t = x.u16(TIMER).wrapping_sub(1);
                x.set16(TIMER, t);
                if s16(t) < 0 {
                    x.add8(0x4, 2);
                }
                x.anim_step();
                if x.u8(0x2f) == 0 {
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
    x.add8(0x4, 2);
    x.set8(f::BOX_SET, 7);
    x.set8(f::ATTACK_ID, 0x23);
    let maker = x.linked(LINK);
    copy_xyz32(x, maker);
    copy_flip(x, maker);
    x.add16(0x10, 0xffff);
    let flip = x.u8(f::FLIP) ^ 0x20;
    x.set8(f::FLIP, flip);
    let row = ROWS + 8 * x.u8(0x3) as u32;
    let timer = x.rom16(row);
    let subtype = x.rom8(row + 2);
    let speed_index = x.rom8(row + 3);
    let anim = x.rom8(row + 4);
    let new_flip = x.rom8(row + 5);
    let speeds = x.rom16(row + 6);
    x.set16(TIMER, timer);
    x.set8(0x2d, speed_index);
    x.set8(f::FLIP, new_flip);
    x.set8(0x2f, subtype);
    x.anim_start(ANIMS, (anim as u32) as u16);
    x.select_speed(&crate::objects::motion::SPEEDS_B, (speeds as u32) as u16);
}
