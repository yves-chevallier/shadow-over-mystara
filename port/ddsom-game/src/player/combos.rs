//! Reconnaissance des coups spéciaux par séquences d'entrées (P6).
//!
//! Chaque image, `detect` (0xf157c) fait avancer une dizaine de *reconnaisseurs* de séquences
//! (dérouler, double appui...) rangés dans l'enregistrement du joueur à `+0x1a0` (6 octets chacun).
//! Un reconnaisseur lit sa séquence dans une table de la ROM : des paires (masque, valeur attendue)
//! suivies d'un mot, -1 marquant la fin de la séquence. Quand une séquence est complète, l'action
//! associée est lancée (`rom::*`), sauf si l'état courant l'interdit : les *portes* (neuf octets en
//! `A5-0x55e0`, un par action) sont posées au début de chaque image selon l'état du joueur.
//!
//! Reconnaisseur (6 octets) : deux automates (octet d'avancement, octet de délai) pour l'orientation
//! normale (`+0`) et miroir (`+2`) ; `+4` : séquence terminée ; `+5` : orientation à l'achèvement.

use super::{ram, Ctx, Player};
use crate::mem::{rd16, wr8, A5};

/// Portes des actions : neuf octets (un par action déclenchable) suivis de trois autres, lus dans
/// le code à chaque image.
const GATES: u32 = A5 - 0x55e0;

/// Début des reconnaisseurs dans l'enregistrement joueur.
const RECOGNIZERS: u32 = 0x1a0;

/// Routines d'origine lancées par un coup spécial reconnu (et non interdit) ; Z = rien changé.
mod rom {
    pub const ACTION_1A0: u32 = 0x37386;
    pub const ACTION_1C4: u32 = 0x3740e;
    pub const ACTION_1BE: u32 = 0x373f4;
    pub const ACTION_1B8: u32 = 0x37366;
    pub const ACTION_1B2: u32 = 0x3733c;
    pub const ACTION_1A6: u32 = 0x373a8;
    pub const ACTION_1AC: u32 = 0x373cc;
    pub const ACTION_1CA: u32 = 0x374aa;
    pub const ACTION_1D6_DWARF: u32 = 0x3743e;
    pub const ACTION_1D6_OTHER: u32 = 0x3746a;
}

/// Variantes de portes : adresse du premier des trois `move.l PC@(..),(A5-...)` du code d'origine ;
/// les trois longs de données sont 0x16, 0x1a et 0x1e octets plus loin.
mod gates {
    pub const BLOCKED: u32 = 0xf19c2;
    pub const HOLDING: u32 = 0xf19e2;
    pub const SUBACTION_A: u32 = 0xf1a02;
    pub const RUNNING: u32 = 0xf1a22;
    pub const SUBACTION_16: u32 = 0xf1a42;
    pub const SUBACTION_C: u32 = 0xf1a62;
    pub const SUBACTION_1E: u32 = 0xf1a82;
    pub const STATE_1200: u32 = 0xf1aa2;
    pub const STATE_1000: u32 = 0xf1ac2;
    pub const FROZEN: u32 = 0xf1b02;
}

/// Tables de séquences (octets du code, lus comme des données).
mod tables {
    pub const R1A0: u32 = 0xf193a;
    pub const R1C4: u32 = 0xf1970;
    pub const R1BE: u32 = 0xf1966;
    pub const R1B8: u32 = 0xf195c;
    pub const R1B2: u32 = 0xf1954;
    pub const R1A6: u32 = 0xf1944;
    pub const R1AC: u32 = 0xf194c;
    pub const R1CA: u32 = 0xf1976;
    pub const R1D6_DWARF: u32 = 0xf197e;
    pub const R1D6_OTHER: u32 = 0xf19ba;
    pub const R1D0: u32 = 0xf19b0;
}

/// Copie les douze octets de portes de la variante `variant`.
fn set_gates(g: &mut Ctx, variant: u32) {
    cov!();
    for k in 0..3 {
        let v = g.prog32(variant + 0x16 + 4 * k);
        crate::mem::wr32(g.b, GATES + 4 * k, v);
    }
}

/// Aucune action interdite.
fn clear_gates(g: &mut Ctx) {
    cov!();
    for k in 0..3 {
        crate::mem::wr32(g.b, GATES + 4 * k, 0);
    }
}

fn gate(g: &mut Ctx, n: u32) -> bool {
    cov!();
    crate::mem::rd8(g.b, GATES + n) != 0
}

/// 0xf18fa : efface tous les reconnaisseurs.
pub fn clear_recognizers(g: &mut Ctx, p: Player) {
    cov!();
    for k in 0..15 {
        p.set_u32(g, RECOGNIZERS + 4 * k, 0);
    }
}

/// 0xf1884 : fait avancer un automate (octet d'avancement `state`, délai `state+1`) d'un pas.
/// `mirror` : masque de miroir des directions (0 ou 3), `side` : valeur écrite à l'achèvement.
/// Renvoie vrai quand la séquence est terminée.
fn step(g: &mut Ctx, p: Player, rec: u32, state: u32, table: u32, timeout: u8, mirror: u8, side: u8) -> bool {
    cov!();
    let mut held = p.u8(g, Player::KEYS_HELD);
    let mut prev = p.u8(g, Player::KEYS_PREV);
    let timer = crate::mem::rd8(g.b, p.at(state + 1)).wrapping_add(1);
    wr8(g.b, p.at(state + 1), timer);
    if (timeout as i8) < (timer as i8) {
        crate::mem::wr16(g.b, p.at(state), 0);
        wr8(g.b, p.at(rec + 4), 0);
    }
    let at = crate::mem::rd8(g.b, p.at(state)) as u32;
    let mask = g.data8(table + at);
    held &= mask;
    prev &= mask;
    if (mask as i8) < 0 && prev == held {
        return false;
    }
    let mut expected = g.data8(table + at + 1);
    if expected & mirror != 0 {
        expected ^= mirror;
    }
    let matched = prev == expected || (expected & 0xf0 != 0 && held == expected);
    if !matched {
        return false;
    }
    if rd16(g.b, table + at + 2) == 0xffff {
        wr8(g.b, p.at(rec + 5), side);
        wr8(g.b, p.at(rec + 4), 1);
        return true;
    }
    let idx = crate::mem::rd8(g.b, p.at(state)).wrapping_add(2);
    wr8(g.b, p.at(state), idx);
    wr8(g.b, p.at(state + 1), 0);
    false
}

/// 0xf186e : essaie l'orientation normale puis l'orientation miroir.
fn recognize_both(g: &mut Ctx, p: Player, rec: u32, table: u32, timeout: u8) -> bool {
    cov!();
    step(g, p, rec, rec, table, timeout, 0, 0x20) || step(g, p, rec, rec + 2, table, timeout, 3, 0)
}

/// 0xf187c : essaie seulement l'orientation miroir.
fn recognize_mirror(g: &mut Ctx, p: Player, rec: u32, table: u32, timeout: u8) -> bool {
    cov!();
    step(g, p, rec, rec + 2, table, timeout, 3, 0)
}

/// 0xf157c : met à jour les portes puis tous les reconnaisseurs. Dès qu'un coup spécial change l'état
/// du joueur (Z = 0 au retour de l'action), les suivants ne sont pas examinés.
pub fn detect(g: &mut Ctx, p: Player) {
    cov!();
    if g.a5u8(ram::INPUT_RAW) != 0 || g.a5u8(ram::SLOW_MODE) != 0 {
        return;
    }
    select_gates(g, p);
    run_recognizers(g, p);
}

/// Choisit les actions interdites selon l'état du joueur.
fn select_gates(g: &mut Ctx, p: Player) {
    cov!();
    use gates::*;
    if g.a5u8(ram::PAUSE) != 0 || p.u8(g, Player::SUB) != 0 {
        return set_gates(g, BLOCKED);
    }
    if g.a5u8(ram::FROZEN) != 0 {
        return set_gates(g, FROZEN);
    }
    let action = p.u8(g, Player::ACTION);
    if p.u16(g, Player::HP) == 0
        || p.u16(g, 0xc) != 0
        || p.u8(g, Player::SLOW_8A) != 0
        || matches!(action, 0x14 | 0x22 | 0x28 | 0x2a | 0xe)
        || p.u8(g, Player::SUBACTION) == 0x18
        || p.state(g) & !0xff == 0x0200_2000
    {
        return set_gates(g, BLOCKED);
    }
    match p.state(g) & !0xff {
        0x0200_0600 => return set_gates(g, RUNNING),
        0x0200_1000 => return set_gates(g, STATE_1000),
        0x0200_1200 => return set_gates(g, STATE_1200),
        _ => {}
    }
    match p.u8(g, Player::SUBACTION) {
        0x16 => return set_gates(g, SUBACTION_16),
        0x1e | 0x1a => return set_gates(g, SUBACTION_1E),
        0xc => return set_gates(g, SUBACTION_C),
        0xa => return set_gates(g, SUBACTION_A),
        _ => {}
    }
    if p.u8(g, 0x15c) != 0 {
        return set_gates(g, HOLDING);
    }
    clear_gates(g);
}

/// Les reconnaisseurs, dans l'ordre de l'original.
fn run_recognizers(g: &mut Ctx, p: Player) {
    cov!();
    // (reconnaisseur, table, délai, deux orientations, porte, action)
    if recognized(g, p, 0x1a0, tables::R1A0, 0xc, true, 0, rom::ACTION_1A0) {
        return;
    }
    // le magicien (classe 4) n'a pas les trois reconnaisseurs suivants
    if p.u8(g, Player::CHARACTER) >> 1 != 4 {
        // boutons 1 et 2 enfoncés ensemble (un des deux vient d'être appuyé) : lancer immédiat
        let pressed = !p.u8(g, Player::KEYS_PREV) & p.u8(g, Player::KEYS_HELD) & 0x30;
        if !gate(g, 1) {
            let fire = pressed == 0x30 || recognize_mirror(g, p, 0x1c4, tables::R1C4, 6);
            if fire && !g.rom(rom::ACTION_1C4, p) {
                return;
            }
        }
        if recognized(g, p, 0x1be, tables::R1BE, 0xc, false, 2, rom::ACTION_1BE) {
            return;
        }
        if recognized(g, p, 0x1b8, tables::R1B8, 0xc, true, 3, rom::ACTION_1B8) {
            return;
        }
    }
    if recognized(g, p, 0x1b2, tables::R1B2, 0xc, true, 4, rom::ACTION_1B2) {
        return;
    }
    if recognized(g, p, 0x1a6, tables::R1A6, 0xc, true, 5, rom::ACTION_1A6) {
        return;
    }
    if recognized(g, p, 0x1ac, tables::R1AC, 0xc, false, 6, rom::ACTION_1AC) {
        return;
    }
    let kind = p.u8(g, Player::CHARACTER) & 0xe;
    if kind == 0xa && recognized(g, p, 0x1ca, tables::R1CA, 0xc, true, 7, rom::ACTION_1CA) {
        return;
    }
    if kind == 6 && recognized(g, p, 0x1d6, tables::R1D6_DWARF, 6, false, 8, rom::ACTION_1D6_DWARF) {
        return;
    }
    if (kind == 0xa || kind == 4) && recognized(g, p, 0x1d6, tables::R1D6_OTHER, 0x10, false, 8, rom::ACTION_1D6_OTHER) {
        return;
    }
    recognize_both(g, p, 0x1d0, tables::R1D0, 0xc);
}

/// 0xf1b22 : vrai quand au moins trois joueurs tiennent les trois premiers boutons (image
/// précédente) et qu'un magicien armé de l'arme 0x21 est libre de lancer son sort ultime.
pub fn all_ready(g: &mut Ctx) -> bool {
    cov!();
    if g.a5u8(ram::FROZEN) != 0 || g.a5u8(ram::PAUSE) != 0 {
        return false;
    }
    let (mut holding, mut caster) = (0, false);
    for q in Player::all() {
        let busy = q.u8(g, Player::ACTIVE) == 0
            || q.u16(g, Player::HP) == 0
            || q.u16(g, 0xc) != 0
            || q.state(g) & !0xff == 0x0200_2000
            || q.u8(g, Player::KEYS_PREV) & 0x70 != 0x70;
        if busy {
            continue;
        }
        holding += 1;
        let free = q.u8(g, Player::CHARACTER) >> 1 == 4
            && q.u8(g, Player::WEAPON_B) == 0x21
            && q.u8(g, Player::SUB) == 0
            && q.u8(g, Player::SLOW_8A) == 0
            && !matches!(q.u8(g, Player::ACTION), 0x14 | 0x22 | 0x28 | 0x2a | 0xe)
            && q.u8(g, Player::SUBACTION) != 0x18;
        caster |= free;
    }
    holding >= 3 && caster
}

/// Un reconnaisseur : vrai si sa séquence est terminée, que sa porte est ouverte et que l'action
/// lancée a changé l'état du joueur (arrêt des examens).
fn recognized(g: &mut Ctx, p: Player, rec: u32, table: u32, timeout: u8, both: bool, gate_n: u32, action: u32) -> bool {
    cov!();
    let done = if both { recognize_both(g, p, rec, table, timeout) } else { recognize_mirror(g, p, rec, table, timeout) };
    if !done || gate(g, gate_n) {
        return false;
    }
    !g.rom(action, p)
}
