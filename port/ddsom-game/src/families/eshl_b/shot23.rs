use super::*;

const ANIMS: u32 = 0x1c03a6;
const WAIT: u8 = 0;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => init(x),
        2 => match x.u8(0x5) {
            WAIT => {
                x.add8(0x54, 0xff);
                if (x.u8(0x54) as i8) < 0 {
                    x.add8(0x5, 2);
                }
                slide(x);
            }
            _ => {
                if x.a5_u8(0x42) == 0 {
                    x.anim_step();
                    return;
                }
                x.set8(0x54, 0x30);
                x.add8(0x4, 2);
                x.set8(0x2d, 0x10);
                speed_table(x, 2);
                x.anim_step();
            }
        },
        4 => {
            x.add8(0x54, 0xff);
            if (x.u8(0x54) as i8) < 0 {
                x.add8(0x4, 2);
            }
            slide(x);
        }
        _ => x.free_eshl(),
    }
}

fn slide<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_by_table_xy();
    x.anim_step();
    reinsert_foreground(x);
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x4, 2);
    x.set8(0x54, 0x30);
    x.set8(0x2d, 0);
    speed_table(x, 0xd);
    let camera = x.a5_u16(0x556);
    x.set16(0x8, camera);
    x.set32(0xc, 0);
    x.set32(0x10, 0);
    x.set16(0xc, 0x70);
    x.add16(0x10, -0x20i16 as u16);
    let mut anim = 0x4a;
    if x.u8(0x2f) != 0 {
        anim += 0x9f;
        x.add16(0x10, 0x80);
        x.add16(0xc, -0x10i16 as u16);
    }
    x.anim_start(ANIMS, anim);
}
