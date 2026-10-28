//! Type 14, état touché (`+4 = 4`, 0x6d050). `+5` : 0 réaction par table (`+0x49`), 2 recul, 4 chute et relevé,
//! 6 seconde table de réactions, 8 saisie par un joueur (le Troll s'enfuit). Zone et ombre sont toujours testées.

use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

/// Tables de réactions communes aux coups, indexées par le code `+0x49`.
const REACT_FIRST: u32 = 0x6d0f0;
const REACT_SECOND: u32 = 0x6d284;

pub(super) fn run(x: &mut Cx) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        x.set8(0, 1);
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
    zone_and_terrain(x);
    x.reinsert();
}

/// Recul (0x6d19c).
fn recoil<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set8(f::FREEZE, 4);
        x.set16(Obj::VX, 0x500);
        x.set16(Obj::AX, -0x38i16 as u16);
        let flip = x.u8(0x4a) ^ 0x20;
        x.set8(0x4a, flip);
        if flip != 0 {
            negate(x, Obj::VX);
            negate(x, Obj::AX);
        }
        return mv(x, 0xc);
    }
    if x.u8(Obj::VX) != 0 {
        x.move_x();
    }
    if x.anim_step() {
        set_state_long(x, STANCE);
    }
}

/// Chute puis relevé (0x6d202).
fn fall(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.anim_start(SET_HURT, 8);
        }
        2 => {
            if paused(x) {
                return;
            }
            let n = x.u16(0x84).wrapping_sub(1);
            x.set16(0x84, n);
            if (n as i16) > 0 {
                return;
            }
            x.set8(0x86, 0);
            x.set8(0x49, 0);
            x.set8(0, 2);
            x.add8(f::SUB2, 2);
            mv(x, 6);
        }
        4 => {
            if paused(x) {
                return;
            }
            if x.anim_step() {
                x.set8(0x86, 0);
                x.set8(0, 1);
                x.set8(VULNERABLE, 1);
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}
