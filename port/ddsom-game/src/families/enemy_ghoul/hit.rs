//! Type 5, état touché (`+4 = 4`, 0x6089a). `+5` : 0 et 6 réaction commune par table (`+0x49`), 2 recul,
//! 4 chute, 8 saisie par un joueur (le ghoul s'enfuit).

use crate::families::cx::Banc;
use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

/// Tables de la réaction commune, indexées par le code de réaction `+0x49`.
const REACT_FIRST: u32 = 0x6090c;
const REACT_SECOND: u32 = 0x60abe;

pub(super) fn run(x: &mut Cx) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        if x.u8(f::SUB2) == 0 {
            x.set8(LOOK, 0xff);
        }
        x.c.d[0] = x.u8(f::SUB) as u32;
        match x.u8(f::SUB) {
            0 => {
                x.set8(LOOK, 0xff);
                if x.u8(0x49) == 0x64 || !special_hit(x) {
                    react(x, REACT_FIRST);
                }
            }
            2 => recoil(x),
            4 => down(x),
            6 => react(x, REACT_SECOND),
            8 => caught(x),
            _ => {}
        }
    }
    if x.u8(f::SUB) != 8 {
        zone_and_terrain(x);
    }
    x.reinsert();
}

/// Sous-état 2 : le ghoul recule (animation 9), puis se remet debout.
fn recoil<R: Banc>(x: &mut Cx<'_, R>) {
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
        return body(x, 9);
    }
    if x.u8(Obj::VX) != 0 {
        x.move_x();
        return;
    }
    set_state_long(x, STANCE);
    x.set8(0, 1);
    body(x, 0);
}

/// Sous-état 4 : chute (animation 6), attente couché, relevé.
fn down(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            body(x, 6);
        }
        2 => {
            if special_hit(x) || paused(x) {
                return;
            }
            let t = x.u16(0x84).wrapping_sub(1);
            x.set16(0x84, t);
            if (t as i16) > 0 {
                return;
            }
            x.set8(0, 2);
            x.set16(0x84, 0);
            x.set8(0x86, 0);
            x.set8(0x49, 0);
            x.add8(f::SUB2, 2);
            body(x, 7);
        }
        4 => {
            if paused(x) {
                return;
            }
            if step(x) {
                x.set8(VULNERABLE, 1);
                x.set8(0, 1);
                set_state_long(x, STANCE);
                body(x, 0);
            }
        }
        _ => {}
    }
}

/// Sous-état 8 (0x60b6a) : saisi par un joueur, le ghoul se retourne et s'enfuit hors de l'écran.
fn caught(x: &mut Cx) {
    if paused(x) {
        return;
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            let pos = x.u16(Obj::X);
            x.set16(SAVED_X, pos);
            let cam = if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA_ALT_X.get(x.b) } else { var::SCROLL_X.get(x.b) };
            set_dw(x, 0, cam);
            x.set16(Obj::AX, 0);
            x.set16(Obj::VX, 0x300);
            let mut want = 0x20;
            let d1 = pos.wrapping_sub(cam);
            if (d1 as i16) < 0xc0 {
                negate(x, Obj::VX);
                want = 0;
            }
            if want != x.u8(f::FLIP) {
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                return body(x, 1);
            }
            flee_begin(x);
        }
        2 => {
            if step(x) {
                flee_begin(x);
            }
        }
        4 => {
            let vx = x.u16(Obj::VX);
            if vx != 0x300 && vx != 0xfd00 {
                return x.set8(f::SUB2, 0);
            }
            x.move_x();
            if !x.in_arena_default() {
                detach_from_host(x);
                return set_state_long(x, st::DEAD_LONG);
            }
            lib::keep_in_zone(x);
            if x.u16(SAVED_X) != x.u16(Obj::X) {
                let pos = x.u16(Obj::X);
                x.set16(SAVED_X, pos);
                x.set8(TIMER, 0x3c);
                return {
                    step(x);
                };
            }
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if n == 0 {
                detach_from_host(x);
                return set_state_long(x, st::DEAD_LONG);
            }
            step(x);
        }
        _ => {}
    }
}

/// 0x60bda : le ghoul court (animation 2).
fn flee_begin<R: Banc>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 2);
    body(x, 2);
}
