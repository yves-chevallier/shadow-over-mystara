//! Type 1, état touché (`+4 = 4`, 0x58ca6). `+5` : 0 et 6 réaction commune par table (`+0x49`),
//! 2 recul (riposte possible), 4 chute, 8 fuite hors de l'écran, 10 rien.

use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

/// Tables de la réaction commune, indexées par le code de réaction `+0x49`.
const REACT_FIRST: u32 = 0x58d0a;
const REACT_SECOND: u32 = 0x58f2e;

pub(super) fn run(x: &mut Cx) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        match x.u8(f::SUB) {
            0 => {
                // le code 0x64 n'est jamais une réaction spéciale
                if x.u8(0x49) == 0x64 || !special_hit(x, 0x58d00) {
                    react(x, REACT_FIRST);
                }
            }
            2 => recoil(x),
            4 => down(x),
            6 => react(x, REACT_SECOND),
            8 => flee(x),
            _ => {}
        }
    }
    if x.u8(f::SUB) != 8 {
        zone_shadow(x);
    }
    x.reinsert();
}

// --- sous-état 2 : recul ------------------------------------------------------------------------

fn recoil(x: &mut Cx) {
    if paused(x) || special_hit(x, 0x58dc6) {
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.call(0xec658);
        let freeze = x.u8(0x56) >> 1;
        x.set8(f::FREEZE, freeze);
        x.set16(TIMER, 0xf);
        x.set16(Obj::VX, 0x480);
        x.set16(Obj::AX, -0x80i16 as u16);
        let flip = x.u8(0x4a) ^ 0x20;
        x.set8(f::FLIP, flip);
        if flip != 0 {
            negate(x, Obj::VX);
            negate(x, Obj::AX);
        }
        return body(x, 4);
    }
    // recule pendant `TIMER` images, puis se relève
    let t = x.u16(TIMER);
    if t != 0 {
        x.set16(TIMER, t - 1);
        if t - 1 != 0 {
            step(x);
            return;
        }
    }
    if x.u8(Obj::VX) != 0 {
        x.move_x();
        return;
    }
    x.set32(0x58, 0);
    if x.u8(POSTURE) == 9 {
        let posture = if x.u8(SERIES) != 0 { 8 } else { 3 };
        x.set8(POSTURE, posture);
        let counter = x.u8(COUNTER) as i8;
        if counter != 0 && (counter > 0 || random(x) & 1 != 0) {
            // riposte : le coup `+0xa9` de la série des coups
            x.set16(f::STATE, 0x204);
            let mv = x.u8(COUNTER_MOVE);
            x.set8(f::SUB2, mv);
            x.set8(f::SUB3, 0);
            x.set16(COUNTER, 0);
            return;
        }
    }
    x.set16(COUNTER, 0);
    set_state_long(x, STANCE);
    body(x, 0);
}

// --- sous-état 4 : chute --------------------------------------------------------------------------

fn down(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            body(x, 7);
        }
        2 => {
            if special_hit(x, 0x58ed0) || paused(x) {
                return;
            }
            let t = x.u16(0x84).wrapping_sub(1);
            x.set16(0x84, t);
            if (t as i16) > 0 {
                return;
            }
            x.set8(0, 2);
            x.add8(f::SUB2, 2);
            x.set16(0x84, 0);
            x.set8(0x86, 0);
            x.set8(0x49, 0);
        }
        4 => {
            if paused(x) {
                return;
            }
            if step(x) {
                x.set8(VULNERABLE, 1);
                set_state_long(x, STANCE);
                x.set8(0, 1);
                body(x, 0);
            }
        }
        _ => {}
    }
}

// --- sous-état 8 : fuite ------------------------------------------------------------------------

fn flee(x: &mut Cx) {
    if paused(x) {
        return;
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(CAUGHT, 1);
            let x0 = x.u16(Obj::X);
            x.set16(SAVED_POS, x0);
            let cam = if x.a5_u8(0x36) != 0 { x.a5_u16(0x556) } else { x.a5_u16(0x506) };
            x.c.d[1] = x.c.d[1] & 0xffff_0000 | cam as u32;
            x.c.d[2] = 0;
            x.set16(Obj::VX, -0x280i16 as u16);
            x.set16(Obj::AX, 0);
            let edge = x0.wrapping_sub(0xc0) as i16;
            if edge > cam as i16 {
                negate(x, Obj::VX);
                x.c.d[2] = 0x20;
            }
            if x.u8(f::FLIP) != x.c.d[2] as u8 {
                body(x, 1);
            } else {
                x.add8(f::SUB2, 2);
                body(x, 0);
            }
        }
        2 => {
            if !step(x) {
                return;
            }
            let flip = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
            x.add8(f::SUB2, 2);
            body(x, 0);
        }
        4 => run_away(x),
        6 => {
            if step(x) {
                x.call(call::REMOVE);
                set_state_long(x, st::DEAD_LONG);
            }
        }
        _ => {}
    }
}

/// 0x59082 : court vers le bord de l'écran ; hors de l'écran, il disparaît.
fn run_away(x: &mut Cx) {
    step(x);
    if (x.u8(f::KEY_FRAME) as i8) >= 0 {
        x.move_x();
    }
    if !x.in_arena_default() {
        x.call(call::REMOVE);
        return set_state_long(x, st::DEAD_LONG);
    }
    if x.u8(0x29) == 0 {
        zone_shadow(x);
        if x.u16(SAVED_POS) != x.u16(Obj::X) {
            let x0 = x.u16(Obj::X);
            x.set16(SAVED_POS, x0);
            x.set8(TIMER, 0x14);
            return;
        }
        let n = x.u8(TIMER).wrapping_sub(1);
        x.set8(TIMER, n);
        if n != 0 {
            return;
        }
    }
    // immobile : il se remet à jouer l'animation normale
    x.add8(f::SUB2, 2);
    let series = x.u8(SERIES);
    if series != 0 {
        let v = x.u8(0x2b) | 0x80;
        x.set8(0x91, v);
    }
    x.set8(0x2a, 0);
    body(x, 0xc + series as u32);
}
