//! Type 18, état touché (`+4 = 4`, 0x7157e). `+5` : 0 réaction par table (`+0x49`), 2 recul, 4 chute et relevé,
//! 6 mort en plusieurs temps, 8 fuite (saisi par un joueur), 10 reprise du combat.

use super::*;
use crate::objects::pools;
use crate::objects::Courant;

/// Table de réactions communes aux coups, indexée par le code `+0x49`.
const REACTIONS: u32 = 0x715d4;

pub(super) fn run(x: &mut Cx) {
    if x.u8(f::FREEZE) as i8 > 0 {
        x.add8(f::FREEZE, 0xff);
        return zone_shadow(x);
    }
    x.c.d[0] = x.u8(f::SUB) as u32;
    match x.u8(f::SUB) {
        0 => first(x),
        2 => recoil(x),
        4 => get_up(x),
        6 => dying(x),
        8 => flee(x),
        10 => regroup(x),
        _ => {}
    }
    if x.u8(f::SUB) != 8 {
        zone_shadow(x);
    }
}

/// 0x71e24 : plus de PV → mort (sous-état 6, état 2 le temps de la chute) ; vrai si l'ogre meurt.
fn death_check(x: &mut Cx) -> bool {
    if (x.u16(f::HP) as i16) > 0 {
        x.c.d[0] = 0;
        return false;
    }
    x.set8(f::SUB, 6);
    x.set8(0, 2);
    x.c.d[0] = 1;
    true
}

fn first(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        x.set8(INVULNERABLE, 0);
        if death_check(x) {
            return dying(x);
        }
    }
    react(x, REACTIONS);
}

// --- sous-état 2 : recul ----------------------------------------------------------------------------------

fn recoil(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => {
            x.set16(TIMER, 0x14);
            let (angle, vy) = if x.u8(f::FLIP) != 0 { (0x18, 0x20i16) } else { (8, -0x20) };
            x.set8(ANGLE, angle);
            x.set16(SPEED_Y, vy as u16);
            set_speed(x, 0x17);
            look(x, 0x20);
            x.add8(f::SUB2, 2);
            play(x, SET_A, 9);
        }
        2 => {
            if !count_down(x, TIMER) {
                x.add8(f::SUB2, 2);
                x.set16(TIMER, 0x14);
                x.set16(SPEED_Y, 0);
            }
            x.move_x();
            lib::step(x);
        }
        4 => {
            if count_down(x, TIMER) {
                x.move_x();
                lib::step(x);
            } else {
                x.set32(0x58, 0);
                go::strike_6(x);
            }
        }
        _ => {}
    }
}

// --- sous-état 4 : chute et relevé ---------------------------------------------------------------------------

fn get_up(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.call(0xfffc);
            x.set8(LOOK_PREV, 0xff);
            x.anim_start(SET_A, 0xa);
        }
        2 => {
            if (x.a5_u8(0x3d) as i8) > 0 {
                return;
            }
            let n = x.u16(0x84).wrapping_sub(1);
            x.set16(0x84, n);
            if (n as i16) > 0 {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(VULNERABLE, 1);
            x.set8(NO_GRAB, 0);
            x.set8(0x49, 0);
        }
        4 => {
            if x.a5_u8(0x3d) == 0 && x.anim_step() {
                regroup(x);
            }
        }
        _ => {}
    }
}

/// 0x717ea : le combat reprend : cible choisie, palier de PV franchi, nouveau geste.
pub(super) fn regroup(x: &mut Cx) {
    x.set8(0, 1);
    x.set8(LOOK_PREV, 0xff);
    if !pick_target(x) {
        return go::retire(x);
    }
    let table = if (x.u8(VARIANT) as i8) < 0 {
        0x10de2
    } else if x.a5_u8(0x48) > 5 {
        0x107e2
    } else {
        0x106e2
    };
    lib::hp_threshold(x, table);
    if target_in_box(x, 0, 0x30, 0xc) {
        go::strike_6(x);
    } else {
        go::wander(x);
    }
}

// --- sous-état 6 : mort --------------------------------------------------------------------------------------

fn dying(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(0, 2);
            x.set32(f::IMMUNITY, 0xffff_ffff);
            x.call(0xe3e8e);
            x.call(0x19ad6);
            x.call(0xb780);
            x.call(0xbad6);
            x.call(0xe2f6);
            if (x.u8(VARIANT) as i8) >= 0 {
                x.call(call::REMOVE);
            }
            x.set16(SPEED_Y, 0);
            x.set16(SPEED_Z, 0x500);
            x.set16(GRAVITY, -0x38i16 as u16);
            let right = x.a5_u16(0x5a6).wrapping_add(0xc0);
            let (mut vx, mut flip) = (0x200i16, 0);
            if (right as i16) < (x.u16(X) as i16) {
                vx = -vx;
                flip = 0x20;
            }
            x.set16(SPEED_X, vx as u16);
            x.set8(f::FLIP, flip);
            x.anim_start(SET_DEATH, 0);
        }
        2 => {
            x.move_xz();
            if (x.u16(HEIGHT) as i16) > 0 {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(HEIGHT, 0);
            spark(x, 5);
            x.call(0xec904);
            x.anim_start(SET_A, 0xb);
        }
        4 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
                x.set16(TIMER, 0x1e);
            }
        }
        6 => {
            if !count_down(x, TIMER) {
                x.add8(f::SUB2, 2);
                x.set16(TIMER, 0x3c);
            }
        }
        8 => {
            if !count_down(x, TIMER) {
                x.free(&pools::ENEMY);
            } else {
                let v = x.u8(1) ^ 2;
                x.set8(1, v);
            }
        }
        _ => {}
    }
}

// --- sous-état 8 : fuite --------------------------------------------------------------------------------------

fn flee(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set16(SPEED_Y, 0);
            angle_by_flip(x);
            set_speed(x, 0x35);
            look(x, 0x40);
            play(x, SET_A, 4);
        }
        2 => {
            lib::step(x);
            if x.u8(KEY) != 2 {
                x.move_x();
            }
            if x.u8(KEY) == 3 {
                x.set8(KEY, 0);
                x.call(0xecdba);
                spark(x, 4);
                x.c.d[0] = 0xa;
                x.call(call::SHOCK);
            }
            if lib::outside(x, -0x40, 0x1c0) {
                x.call(call::REMOVE);
                x.free(&pools::ENEMY);
            }
        }
        _ => {}
    }
}
