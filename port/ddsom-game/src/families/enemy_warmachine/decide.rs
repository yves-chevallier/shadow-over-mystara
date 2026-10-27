//! Type 27 : décisions d'IA. L'original enchaîne des tirages dans des tables d'octets puis des tables de mots (saut
//! relatif) qui aboutissent à l'un des « talons » d'état ; on relit les mêmes tables et on exécute le talon visé.

use super::*;
use crate::objects::Obj;

/// Saut par table de mots : `base + mot(base + index)` (adressage `(base,PC,Dn)`, espace des opcodes).
fn table_target(x: &mut Cx, base: u32, index: u32) -> u32 {
    let w = x.rom16(base.wrapping_add(index)) as i16 as i32;
    (base as i32).wrapping_add(w) as u32
}

/// Exécute le talon d'état d'adresse `target` (ceux que les tables de sauts de ce type désignent).
fn stub(x: &mut Cx, target: u32) {
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

/// Tire `r = rand & 0xf`, lit l'octet `bytes[r]` puis saute par la table de mots `words`.
fn pick(x: &mut Cx, bytes: u32, words: u32) {
    let r = rand_and(x, 0xf);
    let b = x.rom8(bytes + r) as u32;
    x.c.d[0] = x.c.d[0] & !0xff | b;
    let target = table_target(x, words, b);
    stub(x, target);
}

/// 0x78fa2 : tir quand le joueur est tout près (table 0x78fb8).
pub(super) fn point_blank(x: &mut Cx) {
    pick(x, 0x78fb8, 0x78fc8);
}

/// 0x78fca : tir sur une cible touchée par la boîte (−0x10, 0x10, 8) ; trois blocs identiques selon `+0xaa`.
pub(super) fn pick_by_phase(x: &mut Cx) {
    let r = rand_and(x, 0xf);
    let phase = x.u8(PHASE) as i8 as i32 as u32;
    let block = table_target(x, 0x78fe4, phase.wrapping_mul(2));
    // 0x78fea, 0x78ff0 et 0x78ff6 lisent la même table
    let _ = block;
    let b = x.rom8(0x79002 + r) as u32;
    x.c.d[0] = x.c.d[0] & !0xff | b;
    let target = table_target(x, 0x79012, b);
    stub(x, target);
}

/// 0x79014 : décision après la boîte de tir (0x78f30) ; `gate` = résultat de `ai_gate` (1 ou 2).
pub(super) fn near_decision(x: &mut Cx, gate: u32) {
    let bytes = if x.u8(GROUP) == 0 {
        0x7907e
    } else if gate as u8 == 1 {
        0x7908e
    } else {
        0x7909e
    };
    pick(x, bytes, 0x790ae);
}

/// 0x790b4 : décision après la boîte large (0x78f4e) : bloc choisi selon le tirage, `+0xaa` et le profil.
pub(super) fn wide_decision(x: &mut Cx, gate: u32) {
    x.c.d[2] = gate;
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
    x.c.d[0] = x.c.d[0] & !0xff | b;
    let target = table_target(x, 0x79198, b);
    stub(x, target);
}

/// 0x78f30 : joueur dans la boîte (−0x50, 0x30, 6) puis décision d'attaque de la bibliothèque ; 0 si rien.
pub(super) fn gate_near(x: &mut Cx) -> u32 {
    if !lib::target_in_box(x, -0x50, 0x30, 6) {
        return 0;
    }
    lib::ai_gate(x)
}

/// 0x78f4e : joueur dans la boîte large (−0xe0, 0x40 ou 0x60, 0xc ou 8 selon le profil), à l'écran.
pub(super) fn gate_wide(x: &mut Cx) -> u32 {
    let lo = camera_x(x).wrapping_sub(0x10) as i16;
    let hi = camera_x(x).wrapping_add(0x190) as i16;
    let at = x.u16(Obj::X) as i16;
    if lo >= at || hi <= at {
        x.c.d[0] = 0;
        return 0;
    }
    let (half_w, half_d) = if x.u8(GROUP) != 0 { (0x60, 8) } else { (0x40, 0xc) };
    if !lib::target_in_box(x, -0xe0, half_w, half_d) {
        x.c.d[0] = 0;
        return 0;
    }
    lib::ai_gate(x)
}

/// 0x78e74 / 0x78e80 : délai de charge `+0x71` d'après la difficulté.
pub(super) fn charge_delay(x: &mut Cx, table: u32) {
    let level = var::RANK.get(x.b) as u32;
    x.c.d[0] = x.c.d[0] & !0xffff | level;
    let v = x.rom8(table + level);
    x.set8(AI_CHARGE, v);
}

/// 0x78ecc / 0x78ede : délai de marche `+0x6b` tiré dans une rangée.
pub(super) fn wander_delay(x: &mut Cx, table: u32) {
    let r = rand_and(x, 0x1f);
    let v = x.rom8(table + r);
    x.set8(AI_WANDER, v);
}
