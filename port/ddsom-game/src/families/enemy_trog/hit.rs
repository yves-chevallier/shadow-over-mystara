//! Type 2, état touché (`+4 = 4`, 0x5ab72). `+5` : 0 et 6 réaction commune par table (`+0x49`), 2 recul,
//! 4 chute, 8 saisie par un joueur (le troglodyte s'enfuit).

use super::*;
use crate::objects::Courant;

/// Tables de la réaction commune, indexées par le code de réaction `+0x49`.
const REACT_FIRST: u32 = 0x5acfc;
const REACT_SECOND: u32 = 0x5ae8e;

pub(super) fn run(x: &mut Cx) {
    if x.u8(SERIES) == 0 {
        blink_hit(x);
    }
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        x.c.d[0] = x.u8(f::SUB) as u32;
        match x.u8(f::SUB) {
            0 => {
                if x.u8(f::SUB2) == 0 {
                    x.set_bit(1, 1, false);
                }
                react(x, REACT_FIRST);
            }
            2 => recoil(x),
            4 => down(x),
            6 => react(x, REACT_SECOND),
            8 => caught(x),
            _ => {}
        }
    }
    if x.u8(NO_SHADOW) == 0 {
        zone_shadow(x);
    }
    x.reinsert();
}

/// Le clignotement de la série 0 continue pendant l'état touché (0x5ab72).
fn blink_hit(x: &mut Cx) {
    let phase = x.u8(BLINK_PHASE) as i8;
    if phase <= 0 {
        return;
    }
    if phase == 1 {
        x.set_bit(1, 1, false);
        x.add8(BLINK_PHASE, 1);
        x.set8(BLINK_ON, 1);
        x.set16(BLINK_COUNT, 0xc);
        x.set8(BLINK_WAIT, 0);
    }
    let n = x.u16(BLINK_COUNT).wrapping_sub(1);
    x.set16(BLINK_COUNT, n);
    if (n as i16) < 0 {
        x.set8(BLINK_ON, 0);
        x.set8(BLINK_WAIT, 0);
        x.set8(BLINK_PHASE, 0xff);
        x.set16(BLINK_REST, 0x258);
        x.set_bit(1, 1, true);
    }
    let v = x.u8(1) ^ 2;
    x.set8(1, v);
}

/// Sous-état 2 : le troglodyte recule, puis se remet debout.
fn recoil(x: &mut Cx) {
    if paused(x) {
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set8(f::FREEZE, 4);
        x.set16(SPEED_X, 0x500);
        x.set16(SPEED_Y, -0x38i16 as u16);
        x.call(0xec6ca);
        let flip = x.u8(0x4a) ^ 0x20;
        x.set8(0x4a, flip);
        if flip != 0 {
            negate(x, SPEED_X);
            negate(x, SPEED_Y);
        }
        if x.u8(LOOK_PREV) != 0xdf {
            return body(x, 0xd);
        }
    }
    if x.u8(SPEED_X) != 0 {
        x.move_x();
        return;
    }
    set_state_long(x, STANCE);
    x.set8(0, 1);
    x.set32(0x58, 0);
    x.set8(POSTURE, 4);
}

/// Sous-état 4 : chute, attente couché, relevé.
fn down(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            body(x, 7);
        }
        2 => {
            if paused(x) {
                return;
            }
            let t = x.u16(0x84).wrapping_sub(1);
            x.set16(0x84, t);
            if (t as i16) > 0 {
                return;
            }
            x.set8(0x86, 0);
            x.set8(0x49, 0);
            x.add8(f::SUB2, 2);
        }
        4 => {
            if paused(x) {
                return;
            }
            if step(x) {
                x.set8(VULNERABLE, 1);
                x.set8(0, 1);
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}

/// Sous-état 8 (0x5ac08) : saisi par un joueur, le troglodyte se retourne et s'enfuit.
pub(super) fn caught(x: &mut Cx) {
    if paused(x) {
        return;
    }
    match x.u8(f::SUB2) {
        0 => {
            x.set8(NO_SHADOW, 1);
            x.call(call::BEHIND_CHECK);
            let flipped = x.u8(f::FLIP) != 0;
            let run_now = if x.c.cond(7) { !flipped } else { flipped };
            if run_now {
                x.add8(f::SUB2, 4);
                run_away(x);
            } else {
                x.add8(f::SUB2, 2);
                x.set8(VULNERABLE, 1);
                x.set8(LOOK, 0x7f);
                x.set8(LOOK_PREV, 0x7f);
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                body(x, 1);
            }
        }
        2 => {
            if step(x) {
                x.add8(f::SUB2, 2);
                run_away(x);
            }
        }
        4 => {
            step(x);
            move_xy(x);
            if !x.in_arena_default() {
                set_state_long(x, st::DEAD_LONG);
                x.call(call::REMOVE);
            }
        }
        _ => {}
    }
}

/// 0x5ac8c : s'enfuit en courant vers le bord de l'écran.
fn run_away(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    x.set8(ANGLE, 8);
    if x.u8(f::FLIP) == 0 {
        x.set8(ANGLE, 0x18);
    }
    body(x, 2);
    x.select_speed(&crate::objects::motion::SPEEDS_B, 0x21);
}
