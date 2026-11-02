use super::*;
use crate::mem::{rd32, wr32};

const PALETTE_RAM: u32 = 0x91c000;
const PALETTE_BYTES: u32 = 32;
const TABLES: u32 = 0x291f00;
const ALT_TABLE_OFFSET: u32 = 0x360;
const ALT_TABLE_FLAG: u32 = 0x70;
const STEPS: u8 = 0x10;
const PERIOD: u8 = 8;
/// Fields: destination, source table, countdown, current palette.
const DEST: u32 = 0x20;
const SOURCE: u32 = 0x24;
const TICK: u32 = 0x29;
const STEP: u32 = 0x2a;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(0x4) {
        0 => {
            x.add8(0x4, 2);
            let variant = x.u8(0x3) as u16;
            x.set32(DEST, PALETTE_RAM + ((variant << 5) as i16 as i32 as u32));
            x.set8(TICK, 1);
            let alt = x.a5_u16(ALT_TABLE_FLAG) != 0;
            x.set32(SOURCE, TABLES + if alt { ALT_TABLE_OFFSET } else { 0 });
            x.set8(STEP, STEPS);
        }
        2 => {
            let t = x.u8(TICK).wrapping_sub(1);
            x.set8(TICK, t);
            if t != 0 {
                return;
            }
            x.set8(TICK, PERIOD);
            let step = ((x.u8(STEP) as u16) << 5) as i16 as i32 as u32;
            let dest = x.u32(DEST);
            let src = x.u32(SOURCE).wrapping_add(step);
            for i in (0..PALETTE_BYTES).step_by(4) {
                let v = rd32(x.b, src + i);
                wr32(x.b, dest + i, v);
            }
            let s = x.u8(STEP).wrapping_sub(1);
            x.set8(STEP, s);
            if (s as i8) < 0 {
                x.add8(0x4, 2);
                x.free(&pools::SET);
            }
        }
        4 => {
            x.add8(0x4, 2);
            x.free(&pools::SET);
        }
        _ => x.free(&pools::SET),
    }
}
