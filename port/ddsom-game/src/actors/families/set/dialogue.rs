use super::*;
use crate::hw::mem::{wr32, wr8, A5};
use crate::actors::families::set::tables as set_tables;
use crate::world::objects::Obj;
use crate::actors::families::set::fields::SetObj;

/// Text tables per game mode (longs, program space), then per variant (longs ×4).
const TEXT_TABLES: u32 = 0xcc438;
const GAME_MODE: u32 = 0x72;
/// Creation of a SOBJ object of type 0xc: D0 = variant, carry = failure; A1 = the object.
const PALETTE_SRC: u32 = 0x292260;
const PALETTE_DST: u32 = 0x91c180;
/// Stop variable (A5 - 0x5fbe) and "bubble displayed" variable (A5 - 0x5b8c).
const STOP: u32 = A5 - 0x5fbe;
const SHOWN: u32 = A5 - 0x5b8c;
/// Text variables published for display.
const TEXT_ADDR: u32 = 0x5ce;
const TEXT_SIZE: u32 = 0x5d2;
const TEXT_WIDTH: u32 = 0x5d4;
const TEXT_WIDTH_VALUE: u16 = 0xc4;
const LIFETIME: u8 = 0x78;
const TIMER: u32 = 0x20;
const FRAME: u32 = 0x22;
const FRAME_WIDTH: u32 = 0x14;
const FRAME_FREE_STATE: u32 = 0x6000000;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if crate::hw::mem::rd8(x.b, STOP) == 0 {
        if x.u8(Obj::STATE) == 0 {
            return create(x);
        }
        let t = x.u8(TIMER).wrapping_sub(1);
        x.set8(TIMER, t);
        if t != 0 {
            return;
        }
    }
    wr8(x.b, SHOWN, 0);
    x.set_a5_u32(TEXT_ADDR, 0);
    let frame = x.u16(FRAME);
    if frame != 0 {
        wr32(x.b, (frame as i16 as i32 as u32 & 0xff_ffff) + 4, FRAME_FREE_STATE);
    }
    x.free(&pools::SET);
}

fn create<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    x.set8(TIMER, LIFETIME);
    let variant = (x.u8(Obj::VARIANT) << 2) as u32;
    let mode = x.a5_u16(GAME_MODE).wrapping_sub(2).wrapping_mul(2) as i16 as i32 as u32;
    let table = set_tables::pc_long(x.b, TEXT_TABLES.wrapping_add(mode));
    let text = set_tables::long(x.b, table + variant);
    let width = set_tables::word(x.b, text);
    x.set16(FRAME, 0);
    let variant = x.u8(SetObj::PARAM);
    if let Ok(frame) = crate::actors::families::creators::sobj_12_pure(x.b, variant) {
        frame.set_u16(x.b, FRAME_WIDTH, width);
        x.set16(FRAME, frame.0 as u16);
    }
    let size = set_tables::word(x.b, text + 2);
    x.set_a5_u16(TEXT_SIZE, size);
    x.set_a5_u16(TEXT_WIDTH, TEXT_WIDTH_VALUE);
    x.set_a5_u32(TEXT_ADDR, text + 4);
    for i in (0..32).step_by(4) {
        let v = set_tables::long(x.b, PALETTE_SRC + i);
        wr32(x.b, PALETTE_DST + i, v);
    }
}
