use super::*;
use crate::families::shell::tables::{tables, Read};

const ANIMS: u32 = 0x16fafa;
const FLY: u8 = 0;
const FADE: u8 = 2;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    guarded(x, state)
}

fn state<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            x.set8(f::BOX_SET, 7);
            x.set8(f::ATTACK_POWER, 0xc);
            x.set16(0x58, 0x5a);
            let variant = x.u8(0x3) as u32;
            let direction = tables().shot35_directions.at8(x.b, variant);
            x.set8(0x2d, direction);
            speed_table(x, 0xb);
            ai_start(x, ANIMS, 0);
        }
        2 => {
            match x.u8(0x5) {
                FLY => {
                    x.add16(0x58, 0xffff);
                    if (x.u16(0x58) as i16) <= 0 {
                        x.add8(0x5, 2);
                        ai_start(x, ANIMS, 1);
                    } else {
                        ai_step(x);
                        if (x.u16(0x58) as i16) > 0x38 {
                            x.move_by_table_xy();
                        }
                    }
                }
                FADE => {
                    if ai_step(x) {
                        x.add8(0x4, 2);
                    } else {
                        x.move_by_table_xy();
                    }
                }
                _ => {}
            }
            blink_by_parity(x);
            x.push_terrain();
        }
        _ => x.free_eshl(),
    }
}
