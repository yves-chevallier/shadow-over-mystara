//! Type 18, hit state (`+4 = 4`, 0x7157e). `+5`: 0 reaction by table (`+0x49`), 2 recoil, 4 fall and get up, 6 death in
//! several stages, 8 flight (grabbed by a player), 10 fight resumes.

use crate::families::cx::Bench;
use crate::data::RomAddr;
use super::*;
use crate::objects::pools;
use crate::objects::Current;
use crate::objects::Obj;

/// Table of reactions shared by the hits, indexed by code `+0x49`.
const REACTIONS: RomAddr = RomAddr(0x715d4);

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::FREEZE) as i8 > 0 {
        x.add8(f::FREEZE, 0xff);
        zone_and_terrain(x);
        return;
    }
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

/// 0x71e24: no more HP → death (substate 6, state 2 during the fall); true if the ogre dies.
fn death_check<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if (x.u16(f::HP) as i16) > 0 {
        return false;
    }
    x.set8(f::SUB, 6);
    x.set8(Enemy::ACTIVE, 2);
    true
}

fn first<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        x.set8(Ogre::INVULNERABLE, 0);
        if death_check(x) {
            return dying(x);
        }
    }
    react(x, REACTIONS.0);
}

// --- substate 2: recoil ----------------------------------------------------------------------------------

fn recoil<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.set16(Ogre::TIMER, 0x14);
            let (angle, vy) = if x.u8(f::FLIP) != 0 { (0x18, 0x20i16) } else { (8, -0x20) };
            x.set8(Obj::SPEED_INDEX, angle);
            x.set16(Obj::AX, vy as u16);
            set_speed(x, 0x17);
            look(x, 0x20);
            x.add8(f::SUB2, 2);
            play(x, SET_A.0, 9);
        }
        2 => {
            if !count_down(x, Ogre::TIMER) {
                x.add8(f::SUB2, 2);
                x.set16(Ogre::TIMER, 0x14);
                x.set16(Obj::AX, 0);
            }
            x.move_x();
            lib::step(x);
        }
        4 => {
            if count_down(x, Ogre::TIMER) {
                x.move_x();
                lib::step(x);
            } else {
                x.set32(Enemy::HIT_LOCK, 0);
                go::strike_6(x);
            }
        }
        _ => {}
    }
}

// --- substate 4: fall and get up ---------------------------------------------------------------------------

fn get_up<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            lib::behaviour_by_kind(x);
            x.set8(Ogre::LOOK_PREV, 0xff);
            x.anim_start(SET_A.0, 0xa);
        }
        2 => {
            if (var::PAUSE.get(x.b) as i8) > 0 {
                return;
            }
            let n = x.u16(Enemy::RECOVER).wrapping_sub(1);
            x.set16(Enemy::RECOVER, n);
            if (n as i16) > 0 {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(Enemy::DOWNED, 0);
            x.set8(Enemy::REACTION, 0);
        }
        4 => {
            if var::PAUSE.get(x.b) == 0 && x.anim_step() {
                regroup(x);
            }
        }
        _ => {}
    }
}

/// 0x717ea: the fight resumes: target chosen, HP threshold crossed, new move.
pub(crate) fn regroup<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::ACTIVE, 1);
    x.set8(Ogre::LOOK_PREV, 0xff);
    if !pick_target(x) {
        return go::retire(x);
    }
    // threshold blocks 0x10de2, 0x107e2 and 0x106e2
    let table = if (x.u8(Obj::VARIANT) as i8) < 0 {
        8
    } else if var::LEVEL.get(x.b) > 5 {
        2
    } else {
        1
    };
    lib::next_threshold(x, table);
    if lib::target_in_box(x, 0, 0x30, 0xc) {
        go::strike_6(x);
    } else {
        go::wander(x);
    }
}

// --- substate 6: death --------------------------------------------------------------------------------------

fn dying<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(Enemy::ACTIVE, 2);
            x.set32(f::IMMUNITY, 0xffff_ffff);
            lib::sound(x, crate::sound::CMD_30E_40E);
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
            x.anim_start(SET_DEATH.0, 0);
        }
        2 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0 {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(Obj::Z, 0);
            spark(x, 5);
            lib::sound(x, crate::sound::CMD_327_427_B);
            x.anim_start(SET_A.0, 0xb);
        }
        4 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
                x.set16(Ogre::TIMER, 0x1e);
            }
        }
        6 => {
            if !count_down(x, Ogre::TIMER) {
                x.add8(f::SUB2, 2);
                x.set16(Ogre::TIMER, 0x3c);
            }
        }
        8 => {
            if !count_down(x, Ogre::TIMER) {
                x.free(&pools::ENEMY);
            } else {
                let v = x.u8(Enemy::DISPLAY) ^ 2;
                x.set8(Enemy::DISPLAY, v);
            }
        }
        _ => {}
    }
}

// --- substate 8: flight --------------------------------------------------------------------------------------

fn flee<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set16(Obj::AX, 0);
            angle_by_flip(x);
            set_speed(x, 0x35);
            look(x, 0x40);
            play(x, SET_A.0, 4);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) != 2 {
                x.move_x();
            }
            if x.u8(f::KEY_FRAME) == 3 {
                x.set8(f::KEY_FRAME, 0);
                lib::sound(x, crate::sound::CMD_329_429);
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
