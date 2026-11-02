//! Type 8, hit state (`+4 = 4`, 0x650d4). `+5`: 0 reaction to a hit (table by code `+0x49`), 2 recoil, 4 fall and get
//! up, 6 death in several stages, 8 fall of a creature that was grabbing another, 10 fight resumes.

use crate::families::cx::Bench;
use super::*;
use crate::objects::pools;
use crate::objects::Current;
use crate::objects::Obj;

/// Common reactions to hits, indexed by code `+0x49`.
const REACTIONS: u32 = 0x6518a;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
        zone_and_terrain(x);
        return;
    }
    match x.u8(f::SUB) {
        0 => first(x),
        2 => recoil(x),
        4 => get_up(x),
        6 => dying(x),
        8 => fall(x),
        10 => attacks::regroup(x),
        _ => {}
    }
    if x.u8(f::SUB) != 2 && x.u8(f::SUB) != 8 {
        zone_and_terrain(x);
    }
}

/// End of substates 2 and 4: the state is saved and the rest period counted down.
fn tail<R: Bench>(x: &mut Cx<'_, R>) {
    let s = x.u32(f::STATE);
    x.set32(STATE_SAVED, s);
    cooldown(x);
}

/// 0x65ac8: no more HP → death (substate 6, state 2 during the fall); true if the enemy dies.
fn death_check<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if (x.u16(f::HP) as i16) > 0 {
        return false;
    }
    x.set8(f::SUB, 6);
    x.set8(0, 2);
    true
}

// --- substate 0: reaction -----------------------------------------------------------------------

fn first<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        let mut dismissed = true;
        if x.u8(HIT_FLAGS) & 1 != 0 {
            let victim = x.linked(VICTIM);
            let code = victim.u8(x.b, 0x10d) as i8;
            dismissed = code == 0xd || (0x10..=0x13).contains(&code);
            if !dismissed {
                return resume(x);
            }
        }
        if dismissed {
            x.set8(HIT_FLAGS, 0);
            x.set8(INVULNERABLE, 0);
            if death_check(x) {
                return dying(x);
            }
        }
    }
    react(x, REACTIONS);
    let code = x.u8(0x49);
    x.set8(HIT_CODE_SAVED, code);
    let s = x.u32(f::STATE);
    x.set32(STATE_SAVED, s);
}

/// 0x65168: the creature is no longer held: it resumes the saved state with a short rest period.
fn resume<R: Bench>(x: &mut Cx<'_, R>) {
    let code = x.u8(HIT_CODE_SAVED);
    x.set8(0x49, code);
    let s = x.u32(STATE_SAVED);
    x.set32(f::STATE, s);
    x.set16(COOLDOWN, 0x1e);
    x.set8(HIT_FLAGS, 0);
    let hp = x.u16(0x64);
    x.set16(f::HP, hp);
    x.set8(0, 1);
}

// --- substate 2: recoil ----------------------------------------------------------------------------

fn recoil<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.set16(TIMER, 0x14);
            let (angle, vy) = if x.u8(f::FLIP) != 0 { (0x18, 0x20i16) } else { (8, -0x20) };
            x.set8(Obj::SPEED_INDEX, angle);
            x.set16(Obj::AX, vy as u16);
            set_speed(x, 0x17);
            look(x, 0x20);
            x.add8(f::SUB2, 2);
            x.anim_start(SET_FLY, 0xc);
        }
        2 => {
            if !count_down(x, TIMER) {
                x.add8(f::SUB2, 2);
                x.set16(TIMER, 0x14);
                x.set16(Obj::AX, 0);
            }
            x.move_x();
            x.anim_step();
        }
        4 => {
            if count_down(x, TIMER) {
                x.move_x();
                x.anim_step();
            } else {
                x.set32(0x58, 0);
                go::strike_6(x);
            }
        }
        _ => {}
    }
    tail(x);
}

// --- substate 4: fall and get up --------------------------------------------------------------------

fn get_up<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(LOOK_PREV, 0xff);
            x.anim_start(SET_FLY, 0xe);
        }
        2 => {
            if (var::PAUSE.get(x.b) as i8) > 0 {
                return tail(x);
            }
            let n = x.u16(0x84).wrapping_sub(1);
            x.set16(0x84, n);
            if (n as i16) <= 0 {
                x.add8(f::SUB2, 2);
                x.set8(VULNERABLE, 1);
                x.set8(NO_GRAB, 0);
                x.set8(0x49, 0);
            }
        }
        4 => {
            if var::PAUSE.get(x.b) == 0 && x.anim_step() {
                attacks::regroup(x);
            }
        }
        _ => {}
    }
    tail(x);
}

// --- substate 6: death ------------------------------------------------------------------------------

fn dying<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(0, 2);
            x.set32(f::IMMUNITY, 0xffff_ffff);
            lib::sound(x, 0xe385e);
            lib::object_event(x);
            detach_from_host(x);
            lib::drop_rewards(x);
            let flip = x.u8(0x4a) ^ 0x20;
            x.set8(f::FLIP, flip);
            let vx = if flip != 0 { -0x200i16 } else { 0x200 };
            x.set16(Obj::VX, vx as u16);
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, 0x480);
            x.set16(Obj::AZ, -0x40i16 as u16);
            x.move_xz();
            x.anim_start(SET_FLY, 0xf);
        }
        2 => {
            x.move_xz();
            if (x.u16(Obj::VZ) as i16) > 0 {
                x.anim_step();
            } else {
                x.add8(f::SUB2, 2);
                x.anim_start(SET_FLY, 0x10);
            }
        }
        4 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0 {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(Obj::Z, 0);
            spark(x, 9);
            x.anim_step();
        }
        6 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
                x.set16(Obj::VZ, 0x280);
                x.set16(Obj::AZ, -0x40i16 as u16);
                x.move_xz();
                x.anim_start(SET_FLY, 0x11);
            }
        }
        8 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0 {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(TIMER, 0x10);
            x.set16(Obj::Z, 0);
            let vy = if (x.u16(Obj::VX) as i16) > 0 { -0x20i16 } else { 0x20 };
            x.set16(Obj::AX, vy as u16);
            spark(x, 9);
            x.anim_step();
        }
        10 => {
            if count_down(x, TIMER) {
                x.move_x();
            } else {
                x.add8(f::SUB2, 2);
                x.set16(TIMER, 0x14);
            }
        }
        12 => {
            if !count_down(x, TIMER) {
                x.add8(f::SUB2, 2);
                x.anim_start(SET_FLY, 0x12);
            }
        }
        14 => {
            if x.anim_step() {
                x.free(&pools::ENEMY);
            }
        }
        _ => {}
    }
}

// --- substate 8: fall of a flying creature -------------------------------------------------------

fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set16(Obj::AX, 0);
            let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            x.set8(Obj::SPEED_INDEX, angle);
            set_speed(x, 0x2b);
            look(x, 0x40);
            play(x, SET_FLY, 1);
        }
        2 => {
            if (x.u16(Obj::Z) as i16) >= 0x14 && x.u8(f::KEY_FRAME) == 4 {
                x.add8(f::SUB2, 2);
                x.set16(TIMER, 2);
            } else {
                rise(x);
            }
            lib::step(x);
        }
        4 => {
            lib::step(x);
            bob(x);
            if count_down(x, TIMER) {
                return;
            }
            x.add8(f::SUB2, 2);
            lib::sound(x, 0xe3776);
            play(x, SET_FLY, 5);
        }
        6 => {
            x.move_x();
            if lib::outside_band(x, -0x30, 0x1b0) {
                detach_from_host(x);
                x.free(&pools::ENEMY);
            }
        }
        _ => {}
    }
}
