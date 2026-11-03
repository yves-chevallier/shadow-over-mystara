//! Recognition of special moves by input sequences (P6).
//!
//! Each frame, `detect` (0xf157c) advances about ten sequence *recognizers*
//! (roll, double press...) stored in the player record at `+0x1a0` (6 bytes each).
//! A recognizer reads its sequence from a ROM table: pairs (mask, expected value)
//! followed by a word, -1 marking the end of the sequence. When a sequence is complete, the associated
//! action is launched (`moves::combo`), unless the current state forbids it: the *gates* (nine bytes at
//! `A5-0x55e0`, one per action) are set at the start of each frame according to the player's state.
//!
//! Recognizer (6 bytes): two automata (progress byte, delay byte) for the normal orientation
//! (`+0`) and mirrored (`+2`); `+4`: sequence finished; `+5`: orientation at completion.

use crate::objects::Obj;
use super::moves::{combo, Combo};
use super::{ram, Ctx, Player};
use crate::mem::{wr8, A5};
use crate::player::tables::{pc32, byte, word};

/// Action gates: nine bytes (one per triggerable action) followed by three others, read from
/// the code on each frame.
const GATES: u32 = A5 - 0x55e0;

/// Start of the recognizers in the player record.
const RECOGNIZERS: u32 = 0x1a0;


/// Gate variants: address of the first of the three `move.l PC@(..),(A5-...)` of the original code;
/// the three data longs are 0x16, 0x1a and 0x1e bytes further.
mod gates {
    pub const BLOCKED: crate::data::RomAddr = crate::data::RomAddr(0xf19c2);
    pub const HOLDING: crate::data::RomAddr = crate::data::RomAddr(0xf19e2);
    pub const SUBACTION_A: crate::data::RomAddr = crate::data::RomAddr(0xf1a02);
    pub const RUNNING: crate::data::RomAddr = crate::data::RomAddr(0xf1a22);
    pub const SUBACTION_16: crate::data::RomAddr = crate::data::RomAddr(0xf1a42);
    pub const SUBACTION_C: crate::data::RomAddr = crate::data::RomAddr(0xf1a62);
    pub const SUBACTION_1E: crate::data::RomAddr = crate::data::RomAddr(0xf1a82);
    pub const STATE_1200: crate::data::RomAddr = crate::data::RomAddr(0xf1aa2);
    pub const STATE_1000: crate::data::RomAddr = crate::data::RomAddr(0xf1ac2);
    pub const FROZEN: crate::data::RomAddr = crate::data::RomAddr(0xf1b02);
}

/// Sequence tables (code bytes, read as data).
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

/// Copies the twelve gate bytes of variant `variant`.
fn set_gates(g: &mut Ctx, variant: u32) {
    cov!();
    for k in 0..3 {
        let v = pc32(g.b, variant + 0x16 + 4 * k);
        crate::mem::wr32(g.b, GATES + 4 * k, v);
    }
}

/// No forbidden action.
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

/// 0xf18fa: clears all the recognizers.
pub fn clear_recognizers(g: &mut Ctx, p: Player) {
    cov!();
    for k in 0..15 {
        p.set_u32(g, RECOGNIZERS + 4 * k, 0);
    }
}

/// Replaces the low byte of a register.
fn low8(reg: &mut u32, v: u8) {
    *reg = *reg & !0xff | v as u32;
}

/// 0xf1884: advances an automaton (progress byte `state`, delay `state+1`) by one step.
/// `mirror`: direction mirror mask (0 or 3), `side`: value written at completion.
/// Returns true when the sequence is finished.
///
/// Leaves the original's leftovers (re-read by the code that follows, e.g. D3 then D2 by a boss's death:
/// the blood created at 0xb141a takes its position from them): D2.b = held keys, D3.b = mask (then expected).
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
    let mask = byte(g.b, table + at);
    held &= mask;
    prev &= mask;
    low8(&mut g.leftovers.d2, held);
    low8(&mut g.leftovers.d3, mask);
    if (mask as i8) < 0 && prev == held {
        return false;
    }
    let mut expected = byte(g.b, table + at + 1);
    low8(&mut g.leftovers.d3, expected);
    if expected & mirror != 0 {
        expected ^= mirror;
        low8(&mut g.leftovers.d3, expected);
    }
    let matched = prev == expected || (expected & 0xf0 != 0 && held == expected);
    if !matched {
        return false;
    }
    if word(g.b, table + at + 2) == 0xffff {
        wr8(g.b, p.at(rec + 5), side);
        wr8(g.b, p.at(rec + 4), 1);
        return true;
    }
    let idx = crate::mem::rd8(g.b, p.at(state)).wrapping_add(2);
    wr8(g.b, p.at(state), idx);
    wr8(g.b, p.at(state + 1), 0);
    false
}

/// 0xf186e: tries the normal orientation then the mirrored orientation.
/// Leaves D4 (mirror mask) and D5 (value written at completion) of the last automaton tried, re-read later.
fn recognize_both(g: &mut Ctx, p: Player, rec: u32, table: u32, timeout: u8) -> bool {
    cov!();
    g.leftovers.d4 = 0;
    g.leftovers.d5 = 0x20;
    if step(g, p, rec, rec, table, timeout, 0, 0x20) {
        return true;
    }
    recognize_mirror(g, p, rec, table, timeout)
}

/// 0xf187c: tries only the mirrored orientation.
fn recognize_mirror(g: &mut Ctx, p: Player, rec: u32, table: u32, timeout: u8) -> bool {
    cov!();
    g.leftovers.d4 = 3;
    g.leftovers.d5 = 0;
    step(g, p, rec, rec + 2, table, timeout, 3, 0)
}

/// 0xf157c: updates the gates then all the recognizers. As soon as a special move changes the state
/// of the player (Z = 0 on return from the action), the following ones are not examined.
pub fn detect(g: &mut Ctx, p: Player) {
    cov!();
    if ram::INPUT_RAW.get(g.b) != 0 || ram::SLOW_MODE.get(g.b) != 0 {
        return;
    }
    select_gates(g, p);
    run_recognizers(g, p);
}

/// Chooses the forbidden actions according to the player's state.
fn select_gates(g: &mut Ctx, p: Player) {
    cov!();
    use gates::*;
    if ram::PAUSE.get(g.b) != 0 || p.u8(g, Player::SUB) != 0 {
        return set_gates(g, BLOCKED.0);
    }
    if ram::FROZEN.get(g.b) != 0 {
        return set_gates(g, FROZEN.0);
    }
    let action = p.u8(g, Player::ACTION);
    if p.u16(g, Player::HP) == 0
        || p.u16(g, Obj::Z) != 0
        || p.u8(g, Player::SLOW_8A) != 0
        || matches!(action, 0x14 | 0x22 | 0x28 | 0x2a | 0xe)
        || p.u8(g, Player::SUBACTION) == 0x18
        || p.state(g) & !0xff == 0x0200_2000
    {
        return set_gates(g, BLOCKED.0);
    }
    match p.state(g) & !0xff {
        0x0200_0600 => return set_gates(g, RUNNING.0),
        0x0200_1000 => return set_gates(g, STATE_1000.0),
        0x0200_1200 => return set_gates(g, STATE_1200.0),
        _ => {}
    }
    match p.u8(g, Player::SUBACTION) {
        0x16 => return set_gates(g, SUBACTION_16.0),
        0x1e | 0x1a => return set_gates(g, SUBACTION_1E.0),
        0xc => return set_gates(g, SUBACTION_C.0),
        0xa => return set_gates(g, SUBACTION_A.0),
        _ => {}
    }
    if p.u8(g, Player::SPECIAL_ACTIVE) != 0 {
        return set_gates(g, HOLDING.0);
    }
    clear_gates(g);
}

/// The recognizers, in the order of the original.
fn run_recognizers(g: &mut Ctx, p: Player) {
    cov!();
    // (recognizer, table, delay, two orientations, gate, action)
    if recognized(g, p, 0x1a0, tables::R1A0, 0xc, true, 0, Combo::Dash) {
        return;
    }
    // the magic-user (class 4) does not have the following three recognizers
    if p.u8(g, Player::CHARACTER) >> 1 != 4 {
        // buttons 1 and 2 pressed together (one of the two was just pressed): immediate throw
        let pressed = !p.u8(g, Player::KEYS_PREV) & p.u8(g, Player::KEYS_HELD) & 0x30;
        if !gate(g, 1) {
            let fire = pressed == 0x30 || recognize_mirror(g, p, 0x1c4, tables::R1C4, 6);
            if fire && combo(g, p, Combo::Attack24) {
                return;
            }
        }
        if recognized(g, p, 0x1be, tables::R1BE, 0xc, false, 2, Combo::Attack22) {
            return;
        }
        if recognized(g, p, 0x1b8, tables::R1B8, 0xc, true, 3, Combo::Attack12) {
            return;
        }
    }
    if recognized(g, p, 0x1b2, tables::R1B2, 0xc, true, 4, Combo::Attack10) {
        return;
    }
    if recognized(g, p, 0x1a6, tables::R1A6, 0xc, true, 5, Combo::Action10) {
        return;
    }
    if recognized(g, p, 0x1ac, tables::R1AC, 0xc, false, 6, Combo::Action12) {
        return;
    }
    let kind = p.u8(g, Player::CHARACTER) & 0xe;
    if kind == 0xa && recognized(g, p, 0x1ca, tables::R1CA, 0xc, true, 7, Combo::Backstab) {
        return;
    }
    if kind == 6 && recognized(g, p, 0x1d6, tables::R1D6_DWARF, 6, false, 8, Combo::DwarfAttack26) {
        return;
    }
    if (kind == 0xa || kind == 4) && recognized(g, p, 0x1d6, tables::R1D6_OTHER, 0x10, false, 8, Combo::Attack30) {
        return;
    }
    recognize_both(g, p, 0x1d0, tables::R1D0, 0xc);
}

/// 0xf1b22: true when at least three players hold the first three buttons (previous
/// frame) and a magic-user armed with weapon 0x21 is free to cast their ultimate spell.
pub fn all_ready(g: &mut Ctx) -> bool {
    cov!();
    if ram::FROZEN.get(g.b) != 0 || ram::PAUSE.get(g.b) != 0 {
        return false;
    }
    let (mut holding, mut caster) = (0, false);
    for q in Player::all() {
        let busy = q.u8(g, Player::ACTIVE) == 0
            || q.u16(g, Player::HP) == 0
            || q.u16(g, Obj::Z) != 0
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

/// A recognizer: true if its sequence is finished, its gate is open and the action
/// launched changed the player's state (stop of the examinations).
fn recognized(g: &mut Ctx, p: Player, rec: u32, table: u32, timeout: u8, both: bool, gate_n: u32, action: Combo) -> bool {
    cov!();
    let done = if both { recognize_both(g, p, rec, table, timeout) } else { recognize_mirror(g, p, rec, table, timeout) };
    if !done || gate(g, gate_n) {
        return false;
    }
    combo(g, p, action)
}
