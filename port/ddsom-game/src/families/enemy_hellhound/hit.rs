//! Type 6, état touché (`+4 = 4`, 0x62a54). `+5` : 0 réaction par table (`+0x49`), 2 recul, 4 chute et relevé,
//! 6 seconde table de réactions, 8 saisie par un joueur (le chien s'enfuit hors de l'écran).

use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

/// Tables de réactions communes aux coups, indexées par le code `+0x49`.
const REACT_FIRST: u32 = 0x62aa2;
const REACT_SECOND: u32 = 0x62d76;

pub(super) fn run(x: &mut Cx) {
    hit_loop_at(x, NO_ZONE, |x| {
        x.c.d[0] = x.u8(f::SUB) as u32;
        match x.u8(f::SUB) {
            0 => {
                x.set8(LOOK_PREV, 0x7f);
                react(x, REACT_FIRST);
            }
            2 => recoil(x),
            4 => fall(x),
            6 => react(x, REACT_SECOND),
            8 => run_away(x),
            _ => {}
        }
    });
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
            x.set16(Obj::VX, 0x300);
            x.set16(Obj::AX, -0x28i16 as u16);
            let flip = x.u8(0x4a) ^ 0x20;
            x.c.d[0] = x.c.d[0] & !0xff | flip as u32;
            x.set8(f::FLIP, flip);
            if flip != 0 {
                negate(x, Obj::VX);
                negate(x, Obj::AX);
            }
            mv(x, 0xd);
        }
        2 => {
            puff(x, 3, 0, 2, 0x18, -1);
            if x.u8(Obj::VX) != 0 {
                x.move_x();
                return;
            }
            if x.anim_step() {
                x.set32(0x58, 0);
                x.set32(Obj::VX, 0);
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}

// --- sous-état 4 : chute et relevé --------------------------------------------------------------------------

fn fall(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            mv(x, 0xe);
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
            x.add8(f::SUB2, 2);
            x.set8(0, 2);
            x.set16(0x84, 0);
            x.set8(0x86, 0);
            x.set8(0x49, 0);
        }
        4 => {
            if paused(x) || !x.anim_step() {
                return;
            }
            if !lib::target_in_box(x, 0, 0x40, 8) {
                x.set8(0x86, 0);
                x.set8(0, 1);
                x.set8(VULNERABLE, 1);
                set_state_long(x, STANCE);
                return mv(x, 0);
            }
            x.add8(f::SUB2, 2);
            x.set16(Obj::VX, 0x300);
            if x.u8(f::FLIP) != 0 {
                negate(x, Obj::VX);
            }
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, 0x380);
            x.set16(Obj::AZ, -0x38i16 as u16);
            mv(x, 6);
            let r = rand_and(x, 1);
            if r != 0 {
                x.set16(Obj::VX, -0x300i16 as u16);
                if x.u8(f::FLIP) != 0 {
                    negate(x, Obj::VX);
                }
                mv(x, 5);
            }
        }
        6 => {
            if x.u8(f::KEY_FRAME) != 0 {
                x.add8(f::SUB2, 2);
                x.set8(VULNERABLE, 0);
            }
            x.anim_step();
        }
        8 => {
            if x.move_xz().gt() {
                x.anim_step();
                return;
            }
            x.set32(Obj::Z, 0);
            dust(x);
            x.add8(f::SUB2, 2);
            x.set8(VULNERABLE, 1);
            look(x, 0xc);
            mv(x, 0xc);
        }
        10 => {
            if x.anim_step() {
                x.set8(0, 1);
                x.set8(0x86, 0);
                x.set8(VULNERABLE, 1);
                set_state_long(x, STANCE);
                mv(x, 0);
            }
        }
        _ => {}
    }
}

// --- sous-état 8 : saisie par un joueur ------------------------------------------------------------------------

fn run_away(x: &mut Cx) {
    if paused(x) {
        return;
    }
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => {
            x.set8(NO_ZONE, 1);
            let to_right = if x.a5_u8(0x34) == 4 {
                x.u8(f::FLIP) == 0
            } else {
                x.call(0x57c66);
                x.c.cond(7) == (x.u8(f::FLIP) == 0)
            };
            if to_right {
                x.add8(f::SUB2, 4);
                return run_begin(x);
            }
            x.add8(f::SUB2, 2);
            x.set8(VULNERABLE, 1);
            look(x, 0x7f);
            mv(x, 4);
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
        4 => {
            x.anim_step();
            x.move_by_table_xy();
            if !x.in_arena_default() {
                set_state_long(x, st::DEAD_LONG);
                detach_from_host(x);
            }
        }
        _ => {}
    }
}

/// 0x62ec0 : le chien court vers le bord de l'écran.
fn run_begin(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    set_angle(x);
    mv(x, 3);
    set_speed(x, 0x25);
}
