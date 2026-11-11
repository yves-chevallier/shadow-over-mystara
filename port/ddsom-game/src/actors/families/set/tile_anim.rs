use super::*;
use crate::hw::mem::{rd16, rd32, wr32};
use crate::actors::families::set::tables as set_tables;
use crate::world::objects::Obj;

/// An entry of the script table.
const ENTRY_SIZE_SHIFT: u16 = 4;
const ENTRY_SCRIPT: u32 = 0;
const ENTRY_TILEMAP: u32 = 4;
const ENTRY_LOOP: u32 = 8;
/// Champs de l'objet.
const SCRIPT: u32 = 0x30;
const TILEMAP: u32 = 0x34;
const LOOP: u32 = 0x38;
const TIMER: u32 = 0x3a;
const SMALL_ROWS: u32 = 0x2f;
const ROW_BYTES: u32 = 0x40;
const ROW_BYTES_SMALL: u32 = 0x20;
const ATTR_KEEP: u32 = 0xff80;
const TRANSPARENT: u16 = 0x1800;

/// Script table of type 4 and type 6 (program space).
const TABLE_4: u32 = 0xc93a2;
const TABLE_6: u32 = 0xcb024;
const GAME_PAUSED: u32 = 0x3d;

pub fn run4<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(GAME_PAUSED) != 0 || x.u8(Obj::HIDDEN) != 0 {
        return;
    }
    run(x, TABLE_4, false);
}

pub fn run6<R: Bench>(x: &mut Cx<'_, R>) {
    run(x, TABLE_6, true);
}

fn run<R: Bench>(x: &mut Cx<'_, R>, table: u32, skip_transparent: bool) {
    match x.u8(Obj::STATE) {
        0 => {
            x.add8(Obj::STATE, 2);
            x.set16(TIMER, 0);
            let at = table.wrapping_add((((x.u8(Obj::VARIANT) as u16) << ENTRY_SIZE_SHIFT) as i16) as u32);
            let long = |x: &mut Cx<'_, R>, off: u32| set_tables::pc_long(x.b, at + off);
            let (script, tilemap, looping) = (long(x, ENTRY_SCRIPT), long(x, ENTRY_TILEMAP), set_tables::pc_word(x.b, at + ENTRY_LOOP));
            x.set32(SCRIPT, script);
            x.set32(TILEMAP, tilemap);
            x.set16(LOOP, looping);
            step(x, skip_transparent);
        }
        2 => step(x, skip_transparent),
        _ => x.free(&pools::SET),
    }
}

/// One step: waits for the end of the current block's duration, then applies the next block.
fn step<R: Bench>(x: &mut Cx<'_, R>, skip_transparent: bool) {
    let t = x.u16(TIMER).wrapping_sub(1);
    x.set16(TIMER, t);
    if (t as i16) >= 0 {
        return;
    }
    let mut script = x.u32(SCRIPT);
    let map = x.u32(TILEMAP);
    let duration = set_tables::word(x.b, script);
    script += 2;
    x.set16(TIMER, duration);
    if (duration as i16) < 0 {
        if x.u16(LOOP) != 0 {
            x.set8(Obj::STATE, 0);
        } else {
            x.add8(Obj::STATE, 2);
        }
        return;
    }
    let cols = set_tables::word(x.b, script).wrapping_sub(1);
    let rows = set_tables::word(x.b, script + 2).wrapping_sub(1);
    script += 4;
    let stride = if x.u8(SMALL_ROWS) != 0 { ROW_BYTES_SMALL } else { ROW_BYTES };
    let mut row_start = map;
    for _ in 0..=rows as u32 {
        for col in 0..=cols as u32 {
            let cell = row_start + 4 * col;
            let tile = set_tables::long(x.b, script);
            script += 4;
            let keep = skip_transparent && set_tables::word(x.b, script - 4) == TRANSPARENT;
            if !keep {
                let attr = rd16(x.b, cell + 2) as u32 & ATTR_KEEP;
                wr32(x.b, cell, attr.wrapping_add(tile));
            } else {
                let old = rd32(x.b, cell);
                wr32(x.b, cell, old);
            }
        }
        row_start += stride;
    }
    x.set32(SCRIPT, script);
}
