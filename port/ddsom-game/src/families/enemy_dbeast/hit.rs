//! Type 19, état touché (`+4 = 4`, 0x7407a) : réactions aux coups. `+5` : 0 réaction commune par
//! table (`+0x49`), 2 recul des pièces, 4 chute, 6 deuxième table de réactions, 8 mort ou fuite.

use super::stance::next_move;
use super::*;
use crate::objects::Courant;

/// Tables de la réaction commune (indexées par le code de réaction `+0x49`).
const REACT_PART: u32 = 0x740c0;
const REACT_BEAST: u32 = 0x742fa;

pub(super) fn run(x: &mut Cx) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
        return x.reinsert();
    }
    match x.u8(f::SUB) {
        0 => {
            x.set8(CUR_ANIM, 0x7f);
            react(x, REACT_PART);
            zone_and_shadow(x);
        }
        2 => {
            if !paused(x) {
                recoil(x);
                zone_and_shadow(x);
            }
        }
        4 => {
            fall(x);
            zone_and_shadow(x);
        }
        6 => {
            react(x, REACT_BEAST);
            zone_and_shadow(x);
        }
        8 => {
            if !paused(x) {
                flee(x);
            }
        }
        _ => {}
    }
    x.reinsert();
}

fn zone_and_shadow(x: &mut Cx) {
    lib::keep_in_zone(x);
    x.call(SHADOW);
}

// --- sous-état 2 : recul d'une pièce --------------------------------------------------------------

fn recoil(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(WANT_ANIM, 0x1e);
            let want = x.u8(WANT_ANIM);
            x.set8(CUR_ANIM, want);
            lib::sound(x, 0xec658);
            x.set16(0x14, 0x300);
            x.set16(0x16, (-0x28i16) as u16);
            let flip = x.u8(0x4a) ^ 0x20;
            x.set8(f::FLIP, flip);
            if flip != 0 {
                x.set16(0x14, 0x300u16.wrapping_neg());
                x.set16(0x16, 0x28);
            }
            play(x, 0x13);
        }
        2 => {
            let low = x.a5_u16(0x6e) & 3;
            x.set_a5_u16(0x6e, low);
            if low == 0 {
                lib::shock(x, 0, 2, 8, 0xffff);
            }
            if x.u8(0x14) != 0 {
                x.move_x();
                return;
            }
            if x.anim_step() {
                x.set32(0x58, 0);
                x.set32(0x14, 0);
                next_move(x);
            }
        }
        _ => {}
    }
}

// --- sous-état 4 : chute --------------------------------------------------------------------------

fn fall(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set32(0x14, 0);
            x.set16(0x18, 0x180);
            x.set16(0x1a, (-0x30i16) as u16);
            play(x, 0xc);
        }
        2 => {
            let t = x.u16(0x84).wrapping_sub(1);
            x.set16(0x84, t);
            if (t as i16) > 0 {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(0, 2);
            x.set16(0x84, 0);
            x.set8(0x86, 0);
            x.set8(0x49, 0);
        }
        4 => {
            if x.u8(f::KEY_FRAME) != 0 {
                x.add8(f::SUB2, 2);
                x.set8(VULNERABLE, 0);
            }
            step(x);
        }
        6 => {
            if !x.move_xz().gt() {
                x.set32(0xc, 0);
            }
            if x.anim_step() {
                x.set8(0, 1);
                x.set32(0x14, 0);
                x.set32(0x18, 0);
                x.set8(VULNERABLE, 1);
                set_state_long(x, ACTIVE_STANCE);
                play(x, 0);
            }
        }
        _ => {}
    }
}

// --- sous-état 8 : mort ou fuite ------------------------------------------------------------------

fn flee(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            let wall = x.a5_u8(0x34);
            let flipped = x.u8(f::FLIP) != 0;
            let turn_back = if wall == 2 {
                flipped
            } else {
                x.call(0x57c66);
                if x.c.cond(7) { flipped } else { !flipped }
            };
            if turn_back {
                x.add8(f::SUB2, 2);
                x.set8(VULNERABLE, 1);
                x.set8(WANT_ANIM, 5);
                x.set8(CUR_ANIM, 5);
                play(x, 5);
            } else {
                x.add8(f::SUB2, 4);
                run_away(x);
            }
        }
        2 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                run_away(x);
            }
        }
        4 => {
            if x.a5_u8(0x34) == 4 {
                return x.add8(f::SUB2, 2);
            }
            leave_screen(x);
        }
        6 => {
            if x.u8(0x29) != 0 {
                x.add8(f::SUB2, 2);
            }
            x.call(sub::TERRAIN_OF_OBJECT);
            let terrain = x.c.d[2] as u8;
            x.set8(0x29, terrain);
            x.move_by_table_xy();
            if x.in_arena_default() {
                step(x);
            } else {
                detach_from_host(x);
            }
        }
        8 => {
            x.add8(f::SUB2, 2);
            x.set8(VULNERABLE, 1);
            x.set8(WANT_ANIM, 6);
            x.set8(CUR_ANIM, 6);
            let vx = if x.u8(f::FLIP) != 0 { 0x480 } else { -0x480 };
            set_velocity(x, vx, 0x4c0, -0x38);
            play(x, 6);
        }
        10 => {
            if x.u8(f::KEY_FRAME) != 0 {
                x.add8(f::SUB2, 2);
                x.set8(VULNERABLE, 0);
            }
            step(x);
        }
        12 => {
            if x.move_xz().gt() {
                return step(x);
            }
            x.set32(0xc, 0);
            x.add8(f::SUB2, 2);
            x.set8(VULNERABLE, 1);
            x.set8(WANT_ANIM, 0x18);
            x.set8(CUR_ANIM, 0x18);
            play(x, 0xf);
        }
        14 => leave_screen(x),
        _ => {}
    }
}

/// 0x7444e : s'enfuit en courant vers le bord de l'écran.
fn run_away(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    x.set8(WANT_ANIM, 0x16);
    x.set8(Obj::SPEED_INDEX, 8);
    if x.u8(f::FLIP) == 0 {
        x.set8(Obj::SPEED_INDEX, 0x18);
    }
    play(x, 0xd);
    speed(x, 0x21);
}

/// 0x7449e : continue jusqu'à sortir de l'écran, puis disparaît.
fn leave_screen(x: &mut Cx) {
    x.move_by_table_xy();
    if x.in_arena_default() {
        return step(x);
    }
    set_state_long(x, 0x0600_0000);
    if x.u8(PARENT) != 0 {
        x.set_a5_u8(0x4e, 1);
    }
    detach_from_host(x);
}
