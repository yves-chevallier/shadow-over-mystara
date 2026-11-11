use super::*;
use crate::actors::families::set::tables as set_tables;
use crate::actors::families::set::vars;
use crate::world::objects::Obj;

/// Table pointers (data space), per variant.
const TABLES: u32 = 0xd6aa0;
const RANK: u32 = 0x7f92;
const RANK_MAX: u32 = 0x7f94;
const RANK_MIN: u32 = 0x7f96;
const RANK_SAVED_FROM: u32 = 0x7f9c;
const RANK_SAVED: u32 = 0x7fa4;
/// Life thresholds and corresponding categories (0 = almost dead ... 6 = fit).
const THRESHOLDS: [(u16, u16); 3] = [(0x30, 2), (0x50, 4), (0xffff, 6)];

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Obj::STATE) {
        // state 0 continues into state 2: twice +2
        0 => {
            x.add8(Obj::STATE, 4);
            adjust(x);
        }
        2 => {
            x.add8(Obj::STATE, 2);
            adjust(x);
        }
        _ => x.free(&pools::SET),
    }
}

fn adjust<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(vars::GAME) == 0 {
        return;
    }
    let variant = x.u8(Obj::VARIANT) as u32;
    let table = set_tables::long(x.b, TABLES + 4 * variant);
    let count = x.a5_u8(vars::PLAYERS_ALIVE) as u32;
    if count == 0 {
        return;
    }
    let row = count << 3;
    let mut category: u16 = 0xffff;
    for p in players() {
        if p.u8(x.b, pl::ACTIVE) == 0 {
            continue;
        }
        let hp = p.u16(x.b, pl::HEALTH);
        if hp <= 0x10 {
            category = 0;
            break;
        }
        let class = THRESHOLDS.iter().find(|(limit, _)| hp <= *limit).map_or(6, |t| t.1);
        if class <= category {
            category = class;
        }
    }
    let at = table.wrapping_add(row.wrapping_add(category as u32) as u16 as i16 as i32 as u32);
    let delta = set_tables::word(x.b, at) as i16;
    if delta == 0 {
        return;
    }
    let saved = x.a5_u16(RANK_SAVED_FROM);
    x.set_a5_u16(RANK_SAVED, saved);
    let mut rank = (x.a5_u16(RANK) as i16).wrapping_add(delta);
    if delta > 0 {
        if rank >= x.a5_u16(RANK_MAX) as i16 {
            rank = x.a5_u16(RANK_MAX) as i16;
        }
    } else if rank < x.a5_u16(RANK_MIN) as i16 {
        rank = x.a5_u16(RANK_MIN) as i16;
    }
    x.set_a5_u16(RANK, rank as u16 & 0x1f);
}
