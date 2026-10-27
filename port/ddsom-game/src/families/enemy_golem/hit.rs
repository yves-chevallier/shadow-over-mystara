//! Type 11, état touché (`+4 = 4`, 0x684fa). `+5` : 0 réaction par table (`+0x49`), 2 recul, 4 chute et relevé,
//! 6 seconde table de réactions, 8 saisie par un joueur (le Golem sort de l'écran), 10 rien. Zone et ombre sont
//! toujours testées.

use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

/// Tables de réactions communes aux coups, indexées par le code `+0x49`.
const REACT_FIRST: u32 = 0x68542;
const REACT_SECOND: u32 = 0x68724;

pub(super) fn run(x: &mut Cx) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        x.c.d[0] = x.u8(f::SUB) as u32;
        match x.u8(f::SUB) {
            0 => react(x, REACT_FIRST),
            2 => recoil(x),
            4 => fall(x),
            6 => react(x, REACT_SECOND),
            8 => grab(x),
            _ => {}
        }
    }
    zone_shadow(x);
    x.reinsert();
}

/// Recul (0x685ea).
fn recoil(x: &mut Cx) {
    if paused(x) || !hit_gate(x) {
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        let attacker = x.linked(0x54);
        let strength = attacker.u8(x.b, 0x57) >> 1;
        x.set8(f::FREEZE, strength);
        x.set16(TIMER, 0xf);
        x.set16(Obj::VX, 0x480);
        x.set16(Obj::AX, -0x80i16 as u16);
        let flip = x.u8(0x4a) ^ 0x20;
        x.c.d[0] = x.c.d[0] & !0xff | flip as u32;
        x.set8(f::FLIP, flip);
        if flip != 0 {
            negate(x, Obj::VX);
            negate(x, Obj::AX);
        }
        return mv(x, 4);
    }
    if x.u16(TIMER) != 0 {
        let n = x.u16(TIMER).wrapping_sub(1);
        x.set16(TIMER, n);
        if n != 0 {
            x.anim_step();
            return;
        }
    }
    if x.u8(Obj::VX) != 0 {
        x.move_x();
        return;
    }
    x.set32(0x58, 0);
    let anim = if x.u8(GROUP) != 0 { 8 } else { 3 };
    x.set8(0x68, anim);
    set_state_long(x, STANCE);
    mv(x, 0);
}

/// Chute puis relevé (0x6869a).
fn fall(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            mv(x, 7);
        }
        2 => {
            if !hit_gate(x) || paused(x) {
                return;
            }
            let n = x.u16(0x84).wrapping_sub(1);
            x.set16(0x84, n);
            if (n as i16) > 0 {
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
            if x.anim_step() {
                x.set8(0x67, 1);
                set_state_long(x, STANCE);
                x.set8(0, 1);
                mv(x, 0);
            }
        }
        _ => {}
    }
}

/// Saisi par un joueur : le Golem marche vers le bord de l'écran le plus proche (0x687cc).
fn grab(x: &mut Cx) {
    if paused(x) {
        return;
    }
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(0xa7, 1);
            let at = x.u16(Obj::X);
            x.set16(TIMER2, at);
            let cam = if x.a5_u8(0x36) != 0 { x.a5_u16(0x556) } else { x.a5_u16(0x506) };
            let mut want = 0u8;
            x.set16(Obj::VX, -0x280i16 as u16);
            x.set16(Obj::AX, 0);
            let d = at.wrapping_sub(0xc0).wrapping_sub(cam);
            if (d as i16) > 0 {
                negate(x, Obj::VX);
                want = 0x20;
            }
            if x.u8(f::FLIP) ^ want == 0 {
                x.add8(f::SUB2, 2);
                return mv(x, 0);
            }
            mv(x, 1);
        }
        2 => {
            if !x.anim_step() {
                return;
            }
            let flip = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
            x.add8(f::SUB2, 2);
            mv(x, 0);
        }
        4 => {
            x.anim_step();
            if (x.u8(f::KEY_FRAME) as i8) >= 0 {
                x.move_x();
            }
            if !x.in_arena_default() {
                x.call(call::REMOVE);
                set_state_long(x, st::DEAD_LONG);
                return;
            }
            if x.u8(0x29) == 0 {
                zone_shadow(x);
                if x.u16(TIMER2) != x.u16(Obj::X) {
                    let at = x.u16(Obj::X);
                    x.set16(TIMER2, at);
                    x.set8(TIMER, 0x3c);
                    return;
                }
                let n = x.u8(TIMER).wrapping_sub(1);
                x.set8(TIMER, n);
                if n != 0 {
                    return;
                }
            }
            x.add8(f::SUB2, 2);
            let anim = 0xc + x.u8(GROUP) as u32;
            mv(x, anim);
        }
        6 => {
            if x.anim_step() {
                x.call(call::REMOVE);
                set_state_long(x, st::DEAD_LONG);
            }
        }
        _ => {}
    }
}
