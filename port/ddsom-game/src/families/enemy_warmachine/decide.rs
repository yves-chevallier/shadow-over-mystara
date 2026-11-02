//! Type 27: AI decisions. The original chains draws from byte tables then word tables (relative jump) that end up at
//! one of the state "stubs"; the same tables are re-read and the targeted stub is executed.

use crate::families::cx::Bench;
use super::*;
use crate::objects::Obj;

/// Jump through a word table: `base + word(base + index)` (addressing `(base,PC,Dn)`, opcode space).
fn table_target<R: Bench>(x: &mut Cx<'_, R>, base: u32, index: u32) -> u32 {
    let w = x.rom16(base.wrapping_add(index)) as i16 as i32;
    (base as i32).wrapping_add(w) as u32
}

/// Executes the state stub at address `target` (those that the jump tables of this type designate).
fn stub<R: Bench>(x: &mut Cx<'_, R>, target: u32) {
    match target {
        0x7924a => go::charge(x, 1),
        0x7925e => go::attack(x, 0, false),
        0x7926c => go::attack(x, 2, false),
        0x7927a => go::attack(x, 4, false),
        0x7928e => go::attack(x, 6, false),
        0x7929c => go::attack(x, 8, false),
        0x792be => go::attack(x, 0xc, false),
        _ => {}
    }
}

/// Draws `r = rand & 0xf`, reads byte `bytes[r]` then jumps through word table `words`.
fn pick<R: Bench>(x: &mut Cx<'_, R>, bytes: u32, words: u32) {
    let r = rand_and(x, 0xf);
    let b = x.rom8(bytes + r) as u32;
    let target = table_target(x, words, b);
    stub(x, target);
}

/// 0x78fa2: shot when the player is very close (table 0x78fb8).
pub(super) fn point_blank<R: Bench>(x: &mut Cx<'_, R>) {
    pick(x, 0x78fb8, 0x78fc8);
}

/// 0x78fca: shot at a target hit by the box (−0x10, 0x10, 8); three identical blocks according to `+0xaa`.
pub(super) fn pick_by_phase<R: Bench>(x: &mut Cx<'_, R>) {
    let r = rand_and(x, 0xf);
    let phase = x.u8(PHASE) as i8 as i32 as u32;
    let block = table_target(x, 0x78fe4, phase.wrapping_mul(2));
    // 0x78fea, 0x78ff0 and 0x78ff6 read the same table
    let _ = block;
    let b = x.rom8(0x79002 + r) as u32;
    let target = table_target(x, 0x79012, b);
    stub(x, target);
}

/// 0x79014: decision after the shot box (0x78f30); `gate` = result of `ai_gate` (1 or 2).
pub(super) fn near_decision<R: Bench>(x: &mut Cx<'_, R>, gate: u32) {
    let bytes = if x.u8(GROUP) == 0 {
        0x7907e
    } else if gate as u8 == 1 {
        0x7908e
    } else {
        0x7909e
    };
    pick(x, bytes, 0x790ae);
}

/// 0x790b4: decision after the wide box (0x78f4e): block chosen according to the draw, `+0xaa` and the profile.
pub(super) fn wide_decision<R: Bench>(x: &mut Cx<'_, R>, gate: u32) {
    let r = rand_and(x, 0xf);
    let d2 = (gate.wrapping_sub(1) as u16).wrapping_mul(2);
    let d1 = (x.u8(PHASE) as i8 as i16 as u16).wrapping_mul(4);
    let index = d2.wrapping_add(d1) as u32;
    let words = if x.u8(GROUP) == 0 { 0x790e6 } else { 0x790f2 };
    let block = table_target(x, words, index);
    let bytes = match block {
        0x790fe | 0x79106 | 0x7910e | 0x79116 => 0x79158,
        0x7911e | 0x79126 => 0x79168,
        0x7912e | 0x79134 | 0x7913a | 0x79140 => 0x79178,
        0x79146 | 0x7914c => 0x79188,
        _ => return,
    };
    let b = x.rom8(bytes + r) as u32;
    let target = table_target(x, 0x79198, b);
    stub(x, target);
}

/// 0x78f30: player in box (−0x50, 0x30, 6) then attack decision of the library; 0 if nothing.
pub(super) fn gate_near<R: Bench>(x: &mut Cx<'_, R>) -> u32 {
    if !lib::target_in_box(x, -0x50, 0x30, 6) {
        return 0;
    }
    lib::turn_decision(x) as u32
}

/// 0x78f4e: player in the wide box (−0xe0, 0x40 or 0x60, 0xc or 8 depending on the profile), on screen.
pub(super) fn gate_wide<R: Bench>(x: &mut Cx<'_, R>) -> u32 {
    let lo = camera_x(x).wrapping_sub(0x10) as i16;
    let hi = camera_x(x).wrapping_add(0x190) as i16;
    let at = x.u16(Obj::X) as i16;
    if lo >= at || hi <= at {
        return 0;
    }
    let (half_w, half_d) = if x.u8(GROUP) != 0 { (0x60, 8) } else { (0x40, 0xc) };
    if !lib::target_in_box(x, -0xe0, half_w, half_d) {
        return 0;
    }
    lib::turn_decision(x) as u32
}

/// 0x78e74 / 0x78e80: charge delay `+0x71` from the difficulty.
pub(super) fn charge_delay<R: Bench>(x: &mut Cx<'_, R>, table: u32) {
    let level = var::RANK.get(x.b) as u32;
    let v = x.rom8(table + level);
    x.set8(AI_CHARGE, v);
}

/// 0x78ecc / 0x78ede: walk delay `+0x6b` drawn from a row.
pub(super) fn wander_delay<R: Bench>(x: &mut Cx<'_, R>, table: u32) {
    let r = rand_and(x, 0x1f);
    let v = x.rom8(table + r);
    x.set8(AI_WANDER, v);
}
