//! Type 0, sous-états 6 (gestes d'affût), 8 (gestes de repli) et 10 (promenade sans joueur).

use super::attacks::{begin_wait, hold, paw_begin};
use super::*;
use crate::objects::Courant;

/// Fin d'un geste du sous-état 6 : retour à l'affût (0x5634a).
fn ready_after_anim(x: &mut Cx) {
    if x.anim_step() {
        set_state_long(x, STANCE);
        x.set8(LOOK, 0);
        mv(x, 0);
        ready(x);
    }
}

/// 0x555c2 : le Kobold se met à l'affût (test aléatoire, puis boîtes de portée ou charge).
fn ready(x: &mut Cx) {
    x.call(call::RAND_TARGET_CHECK);
    if x.c.cond(7) {
        stance::reach(x);
    } else {
        x.set16(f::SUB2, 0x400);
    }
}

// --- sous-état 6 --------------------------------------------------------------------------------------------

pub(super) fn run_b(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(STEP) {
        0 => return begin_wait(x),
        2 => return hold(x, 0x0206_0000),
        _ => {}
    }
    match x.u8(f::SUB2) {
        0 => b_roar(x),
        2 => b_stretch(x),
        4 => b_somersault(x),
        6 => b_hop(x, 0x80, 0x2c0, false),
        8 => b_hop_strike(x),
        10 => b_hop(x, 0x280, 0x2c0, true),
        _ => {}
    }
}

/// +6 = 0 : rugissement ou souffle (tirage au hasard).
fn b_roar(x: &mut Cx) {
    match x.u8(STEP) {
        4 => {
            let r = rand_and(x, 1);
            if r != 0 {
                set_state_long(x, 0x0202_0002);
                x.set8(LOOK, 0xc);
                mv(x, 0xc);
                return ready(x);
            }
            x.add8(STEP, 4);
            x.set8(VULNERABLE, 1);
            x.set8(LOOK, 7);
            let r = rand_and(x, 0x1f);
            let n = x.data8(0x55de6 + r);
            x.set8(REPEAT, n);
            mv(x, 7);
        }
        6 => ready_after_anim(x),
        8 => attacks::breath_tick(x),
        _ => {}
    }
}

fn b_stretch(x: &mut Cx) {
    match x.u8(STEP) {
        4 => {
            x.add8(STEP, 2);
            x.set8(VULNERABLE, 1);
            x.set8(LOOK, 0xf);
            mv(x, 0xf);
        }
        6 => ready_after_anim(x),
        _ => {}
    }
}

/// +6 = 4 : saut périlleux de côté.
fn b_somersault(x: &mut Cx) {
    match x.u8(STEP) {
        4 => {
            x.add8(STEP, 2);
            x.set16(SPEED_X2, 0x100);
            let r = rand_and(x, 1);
            if r != 0 {
                x.set16(SPEED_X2, -0x100i16 as u16);
            }
            x.set8(VULNERABLE, 1);
            look(x, 6);
            x.set16(SPEED_Y2, 0);
            x.set16(SPEED_Z, 0x400);
            x.set16(GRAVITY, -0x38i16 as u16);
            mv(x, 6);
        }
        6 => attacks_wait_key(x),
        8 => land_yz(x),
        10 => ready_after_anim(x),
        _ => {}
    }
}

/// 0x55148 : attend la clé d'animation.
fn attacks_wait_key(x: &mut Cx) {
    if x.u8(KEY) != 0 {
        x.add8(STEP, 2);
        x.set8(VULNERABLE, 0);
    }
    x.anim_step();
}

/// 0x5515e : retombée du saut périlleux.
fn land_yz(x: &mut Cx) {
    if x.move_yz().gt() {
        x.anim_step();
        return;
    }
    x.set8(0, 1);
    x.set32(HEIGHT, 0);
    x.add8(STEP, 2);
    x.set8(VULNERABLE, 1);
    look(x, 0x18);
    mv(x, 0x18);
}

/// +6 = 6 et 10 : bond de côté ; `strike` : l'animation 7 de coups au lieu de la 0x1c de marche.
fn b_hop(x: &mut Cx, vx: i16, vz: u16, strike_anim: bool) {
    match x.u8(STEP) {
        4 => {
            x.add8(STEP, 2);
            x.set8(VULNERABLE, 1);
            look(x, 0x1c);
            x.set16(SPEED_X, vx as u16);
            if x.u8(f::FLIP) != 0 {
                negate(x, SPEED_X);
            }
            x.set16(SPEED_Y, 0);
            x.set16(SPEED_Z, vz);
            x.set16(GRAVITY, -0x38i16 as u16);
            if strike_anim {
                strike(x, 7);
            } else {
                mv(x, 0x1c);
            }
        }
        6 => attacks_wait_key_clear(x),
        8 => land(x),
        10 => ready_after_anim(x),
        _ => {}
    }
}

/// 0x54fae : attend la clé d'animation (la clé n'est pas remise à zéro).
fn attacks_wait_key_clear(x: &mut Cx) {
    wait_key(x);
}

/// 0x54fc4 : retombée d'un saut.
fn land(x: &mut Cx) {
    if x.move_xz().gt() {
        x.anim_step();
        return;
    }
    x.set32(HEIGHT, 0);
    x.add8(STEP, 2);
    x.set8(VULNERABLE, 1);
    look(x, 0x18);
    mv(x, 0x18);
}

/// +6 = 8 : bond de côté puis coup.
fn b_hop_strike(x: &mut Cx) {
    match x.u8(STEP) {
        4 => b_hop(x, 0x80, 0x2c0, false),
        6 => attacks_wait_key_clear(x),
        8 => land(x),
        10 => {
            if x.anim_step() {
                x.add8(STEP, 2);
                x.set8(STRIKE_POWER, 5);
                x.set8(LOOK, 0x28);
                strike(x, 0);
            }
        }
        12 => ready_after_anim(x),
        _ => {}
    }
}

// --- sous-état 8 ----------------------------------------------------------------------------------------------

pub(super) fn run_c(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 | 6 => c_wait(x),
        2 => c_paw(x),
        4 => c_side(x),
        _ => {}
    }
}

fn c_wait(x: &mut Cx) {
    x.c.d[0] = x.u8(STEP) as u32;
    match x.u8(STEP) {
        4 => {
            x.add8(STEP, 0xfe);
            c_begin(x);
        }
        0 => {
            x.add8(STEP, 2);
            x.set16(TIMER, 0x10);
            c_begin(x);
        }
        2 => {
            let n = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, n);
            if (n as i16) < 0 {
                return set_state_long(x, STANCE);
            }
            if must_turn(x) {
                x.set32(RESUME_STATE, 0x0208_0004);
                let sub2 = x.u8(f::SUB2);
                x.set8(RESUME_SUB2, sub2);
                return set_state_long(x, 0x0202_0200);
            }
            x.anim_step();
        }
        _ => {}
    }
}

fn c_begin(x: &mut Cx) {
    look(x, 0);
    x.set8(VULNERABLE, 1);
    mv(x, 0);
}

/// +6 = 2 : coup de patte.
fn c_paw(x: &mut Cx) {
    match x.u8(STEP) {
        0 => paw_begin(x),
        2 => {
            let n = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, n);
            if (n as i16) >= 0 {
                x.move_by_table_xy();
                x.anim_step();
            } else {
                set_state_long(x, STANCE);
            }
        }
        4 => wait_key(x),
        6 => land(x),
        8 => stand_when_done(x),
        _ => {}
    }
}

/// +6 = 4 : pas de côté ou saut de côté.
fn c_side(x: &mut Cx) {
    x.c.d[0] = x.u8(STEP) as u32;
    match x.u8(STEP) {
        0 => {
            x.set8(VULNERABLE, 1);
            let r = rand_and(x, 1);
            let target = x.u16(f::TARGET) != 0;
            let mut above = false;
            if target {
                let t = x.linked(f::TARGET);
                above = (t.u16(x.b, DEPTH).wrapping_sub(x.u16(DEPTH)) as i16) >= 0;
            }
            if r == 0 {
                x.set8(STEP, 4);
                x.set16(SPEED_X2, -0x80i16 as u16);
                if target && !above {
                    negate(x, SPEED_X2);
                }
                x.set16(SPEED_Y2, 0);
                x.set16(SPEED_Z, 0x3c0);
                x.set16(GRAVITY, -0x38i16 as u16);
                look(x, 6);
                return mv(x, 6);
            }
            x.add8(STEP, 2);
            x.set8(ANGLE, if above { 0x10 } else { 0 });
            look(x, 0xc);
            x.set16(TIMER, 0x24);
            mv(x, 0xc);
            set_speed(x, 0xb);
        }
        2 => {
            let n = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, n);
            if (n as i16) < 0 {
                return set_state_long(x, STANCE);
            }
            x.move_xy();
            x.anim_step();
        }
        4 => attacks_wait_key(x),
        6 => land_yz(x),
        8 => stand_when_done(x),
        _ => {}
    }
}

// --- sous-état 10 : promenade -----------------------------------------------------------------------------------

pub(super) fn idle(x: &mut Cx) {
    if x.a5_u8(0x35) != 0 {
        set_state_long(x, STANCE);
        x.set8(LOOK, 0x7f);
        pick_target(x);
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        let r = rand_and(x, 3);
        mv(x, 8 + r);
        return;
    }
    x.anim_step();
}
