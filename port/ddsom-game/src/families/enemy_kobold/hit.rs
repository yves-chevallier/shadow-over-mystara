//! Type 0, état touché (`+4 = 4`, 0x567ca). `+5` : 0 réaction par table (`+0x49`), 2 recul, 4 chute et relevé,
//! 6 seconde table de réactions, 8 saisie par un joueur (le Kobold s'enfuit).

use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

/// Tables de réactions communes aux coups, indexées par le code `+0x49`.
const REACT_FIRST: u32 = 0x56816;
const REACT_SECOND: u32 = 0x56a0e;

pub(super) fn run(x: &mut Cx) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        if freeze < 0 {
            x.set8(f::FREEZE, 0);
        }
        x.c.d[0] = x.u8(f::SUB) as u32;
        match x.u8(f::SUB) {
            0 => {
                x.set8(LOOK_PREV, 0x7f);
                react(x, REACT_FIRST);
                zone_shadow(x);
            }
            2 => recoil(x),
            4 => fall(x),
            6 => {
                react(x, REACT_SECOND);
                zone_shadow(x);
            }
            8 => run_away(x),
            _ => {}
        }
    }
    x.reinsert();
}

// --- sous-état 2 : recul ------------------------------------------------------------------------------------

fn recoil(x: &mut Cx) {
    if paused(x) {
        return;
    }
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            look(x, 0x1e);
            x.call(0xec5e6);
            x.set16(Obj::VX, 0x300);
            x.set16(Obj::AX, -0x28i16 as u16);
            let flip = x.u8(0x4a) ^ 0x20;
            x.c.d[0] = x.c.d[0] & !0xff | flip as u32;
            x.set8(f::FLIP, flip);
            if flip != 0 {
                negate(x, Obj::VX);
                negate(x, Obj::AX);
            }
            mv(x, 0x1e);
        }
        2 => {
            puff(x, 3, 2, 8, -1);
            if x.u8(Obj::VX) != 0 {
                x.move_x();
            } else if x.anim_step() {
                x.set32(0x58, 0);
                x.set32(Obj::VX, 0);
                after_strike(x);
            }
        }
        _ => {}
    }
    zone_shadow(x);
}

// --- sous-état 4 : chute et relevé --------------------------------------------------------------------------

fn fall(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            mv(x, 0x15);
        }
        2 => {
            if (x.a5_u8(0x3d) as i8) == 0 {
                let n = x.u16(0x84).wrapping_sub(1);
                x.set16(0x84, n);
                if (n as i16) <= 0 {
                    x.add8(f::SUB2, 2);
                    x.set8(0, 2);
                    x.set16(0x84, 0);
                    x.set8(0x86, 0);
                    x.set8(0x49, 0);
                }
            }
        }
        4 => {
            if x.a5_u8(0x3d) == 0 && x.anim_step() {
                x.set8(0, 1);
                x.set8(VULNERABLE, 1);
                set_state_long(x, STANCE);
                mv(x, 0);
            }
        }
        _ => {}
    }
    zone_shadow(x);
}

// --- sous-état 8 : saisie par un joueur ------------------------------------------------------------------------

fn run_away(x: &mut Cx) {
    if paused(x) {
        return;
    }
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => {
            let right = on_right_half(x);
            let flipped = x.u8(f::FLIP) != 0;
            // se retourne vers le bord de l'écran le plus proche, ou court directement
            let turn = if right { !flipped } else { flipped };
            if turn {
                x.add8(f::SUB2, 2);
                x.set8(VULNERABLE, 1);
                look(x, 5);
                mv(x, 5);
            } else {
                x.add8(f::SUB2, 4);
                run_begin(x);
            }
        }
        2 => {
            if !x.anim_step() {
                return;
            }
            x.add8(f::SUB2, 2);
            let flip = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
            run_begin(x);
        }
        4 | 14 => {
            if x.a5_u8(0x34) == 4 && x.u8(f::SUB2) == 4 {
                return x.add8(f::SUB2, 2);
            }
            x.move_by_table_xy();
            if !x.in_arena_default() {
                set_state_long(x, st::DEAD_LONG);
                return x.call(call::REMOVE);
            }
            x.anim_step();
        }
        6 => {
            if x.u8(0x29) != 0 {
                x.add8(f::SUB2, 2);
            }
            zone_shadow(x);
            x.move_by_table_xy();
            x.anim_step();
        }
        8 => {
            x.add8(f::SUB2, 2);
            x.set8(VULNERABLE, 1);
            look(x, 6);
            x.set16(Obj::VX, -0x380i16 as u16);
            if x.u8(f::FLIP) != 0 {
                negate(x, Obj::VX);
            }
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, 0x4c0);
            x.set16(Obj::AZ, -0x38i16 as u16);
            mv(x, 6);
        }
        10 => {
            if x.u8(f::KEY_FRAME) != 0 {
                x.add8(f::SUB2, 2);
                x.set8(VULNERABLE, 0);
            }
            x.anim_step();
        }
        12 => {
            if x.move_xz().gt() {
                x.anim_step();
                return;
            }
            x.set32(Obj::Z, 0);
            x.add8(f::SUB2, 2);
            x.set8(VULNERABLE, 1);
            look(x, 0x18);
            mv(x, 0x18);
        }
        _ => {}
    }
}

/// 0x56b42 : le Kobold court vers le bord de l'écran (animation 0x16).
fn run_begin(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    x.set8(LOOK, 0x16);
    set_angle(x);
    mv(x, 0x16);
    set_speed(x, 0x21);
}
