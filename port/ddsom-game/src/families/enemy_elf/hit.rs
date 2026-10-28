//! Type 12, état touché (`+4 = 4`, 0x6a160). `+5` : 0 et 6 réaction commune par table (`+0x49`), 2 recul,
//! 4 chute, 8 saisie par un joueur (l'elfe disparaît).

use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

/// Tables de la réaction commune, indexées par le code de réaction `+0x49`.
const REACT_FIRST: u32 = 0x6a206;
const REACT_SECOND: u32 = 0x6a3a0;

pub(super) fn run(x: &mut Cx) {
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
    zone_and_terrain(x);
    x.reinsert();
}

/// Sous-état 2 : l'elfe recule (animation 7), puis se remet debout.
fn recoil<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set8(f::FREEZE, 4);
        x.set32(Obj::VX, 0x0500_0000);
        x.set16(Obj::AX, -0x38i16 as u16);
        let flip = x.u8(0x4a) ^ 0x20;
        x.set8(0x4a, flip);
        if flip != 0 {
            negate(x, Obj::VX);
            negate(x, Obj::AX);
        }
        return body(x, 7);
    }
    if x.u8(Obj::VX) != 0 {
        x.move_x();
    }
    if step(x) {
        set_state_long(x, STANCE);
        x.set32(0x58, 0);
    }
}

/// Sous-état 4 : chute (animation 6), attente couché, relevé.
fn down<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            body(x, 6);
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
            x.set8(0, 2);
            x.add8(f::SUB2, 2);
            body(x, 6);
        }
        4 => {
            if paused(x) {
                return;
            }
            if step(x) {
                x.set8(0x86, 0);
                x.set8(0, 1);
                x.set8(VULNERABLE, 1);
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}

/// Sous-état 8 (0x6a198) : saisi par un joueur ; l'animation 4 terminée, l'elfe disparaît.
pub(super) fn caught(x: &mut Cx) {
    if paused(x) {
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        return crouch(x);
    }
    if step(x) {
        detach_from_host(x);
        x.add8(f::STATE, 2);
    }
}
