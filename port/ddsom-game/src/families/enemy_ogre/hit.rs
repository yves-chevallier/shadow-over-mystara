//! Type 18, état touché (`+4 = 4`, 0x7157e). `+5` : 0 réaction par table (`+0x49`), 2 recul, 4 chute et relevé,
//! 6 mort en plusieurs temps, 8 fuite (saisi par un joueur), 10 reprise du combat.

use super::*;
use crate::objects::pools;
use crate::objects::Courant;
use crate::objects::Obj;

/// Table de réactions communes aux coups, indexée par le code `+0x49`.
const REACTIONS: u32 = 0x715d4;

pub(super) fn run(x: &mut Cx) {
    if x.u8(f::FREEZE) as i8 > 0 {
        x.add8(f::FREEZE, 0xff);
        zone_and_terrain(x);
        return;
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
        zone_and_terrain(x);
    }
}

/// 0x71e24 : plus de PV → mort (sous-état 6, état 2 le temps de la chute) ; vrai si l'ogre meurt.
fn death_check(x: &mut Cx) -> bool {
    if (x.u16(f::HP) as i16) > 0 {
        return false;
    }
    x.set8(f::SUB, 6);
    x.set8(0, 2);
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
            x.set8(Obj::SPEED_INDEX, angle);
            x.set16(Obj::AX, vy as u16);
            set_speed(x, 0x17);
            look(x, 0x20);
            x.add8(f::SUB2, 2);
            play(x, SET_A, 9);
        }
        2 => {
            if !count_down(x, TIMER) {
                x.add8(f::SUB2, 2);
                x.set16(TIMER, 0x14);
                x.set16(Obj::AX, 0);
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
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            lib::behaviour_by_kind(x);
            x.set8(LOOK_PREV, 0xff);
            x.anim_start(SET_A, 0xa);
        }
        2 => {
            if (var::PAUSE.get(x.b) as i8) > 0 {
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
            if var::PAUSE.get(x.b) == 0 && x.anim_step() {
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
    let table = if (x.u8(Obj::VARIANT) as i8) < 0 {
        0x10de2
    } else if x.a5_u8(0x48) > 5 {
        0x107e2
    } else {
        0x106e2
    };
    lib::next_threshold(x, table);
    if lib::target_in_box(x, 0, 0x30, 0xc) {
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
            lib::sound(x, 0xe3e8e);
            lib::object_event(x);
            lib::drop_rewards(x);
            if (x.u8(Obj::VARIANT) as i8) >= 0 {
                detach_from_host(x);
            }
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, 0x500);
            x.set16(Obj::AZ, -0x38i16 as u16);
            let right = var::CAMERA1_X.get(x.b).wrapping_add(0xc0);
            let (mut vx, mut flip) = (0x200i16, 0);
            if (right as i16) < (x.u16(Obj::X) as i16) {
                vx = -vx;
                flip = 0x20;
            }
            x.set16(Obj::VX, vx as u16);
            x.set8(f::FLIP, flip);
            x.anim_start(SET_DEATH, 0);
        }
        2 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0 {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(Obj::Z, 0);
            spark(x, 5);
            lib::sound(x, 0xec904);
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
            x.set16(Obj::AX, 0);
            angle_by_flip(x);
            set_speed(x, 0x35);
            look(x, 0x40);
            play(x, SET_A, 4);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) != 2 {
                x.move_x();
            }
            if x.u8(f::KEY_FRAME) == 3 {
                x.set8(f::KEY_FRAME, 0);
                lib::sound(x, 0xecdba);
                spark(x, 4);
                lib::shake_54(x, 0xa);
            }
            if lib::outside_band(x, -0x40, 0x1c0) {
                detach_from_host(x);
                x.free(&pools::ENEMY);
            }
        }
        _ => {}
    }
}
