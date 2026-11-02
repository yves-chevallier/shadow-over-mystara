//! Type 11, hit state (`+4 = 4`, 0x684fa). `+5`: 0 reaction by table (`+0x49`), 2 recoil, 4 fall and get up, 6 second
//! reaction table, 8 grabbed by a player (the Golem leaves the screen), 10 nothing. Zone and shadow are always tested.

use crate::families::cx::Bench;
use crate::data::RomAddr;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

/// Reaction tables shared by the hits, indexed by code `+0x49`.
const REACT_FIRST: RomAddr = RomAddr(0x68542);
const REACT_SECOND: RomAddr = RomAddr(0x68724);

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        match x.u8(f::SUB) {
            0 => react(x, REACT_FIRST.0),
            2 => recoil(x),
            4 => fall(x),
            6 => react(x, REACT_SECOND.0),
            8 => grab(x),
            _ => {}
        }
    }
    zone_and_terrain(x);
    x.reinsert();
}

/// Recul (0x685ea).
fn recoil<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) || !hit_gate(x) {
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        let attacker = x.linked(Enemy::HOLDER);
        let strength = attacker.u8(x.b, Enemy::HIT_STRENGTH) >> 1;
        x.set8(f::FREEZE, strength);
        x.set16(Golem::TIMER, 0xf);
        x.set16(Obj::VX, 0x480);
        x.set16(Obj::AX, -0x80i16 as u16);
        let flip = x.u8(Enemy::ATTACKER_FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        if flip != 0 {
            negate(x, Obj::VX);
            negate(x, Obj::AX);
        }
        return mv(x, 4);
    }
    if x.u16(Golem::TIMER) != 0 {
        let n = x.u16(Golem::TIMER).wrapping_sub(1);
        x.set16(Golem::TIMER, n);
        if n != 0 {
            x.anim_step();
            return;
        }
    }
    if x.u8(Obj::VX) != 0 {
        x.move_x();
        return;
    }
    x.set32(Enemy::HIT_LOCK, 0);
    let anim = if x.u8(Enemy::GROUP) != 0 { 8 } else { 3 };
    x.set8(Enemy::AI_68, anim);
    set_state_long(x, STANCE);
    mv(x, 0);
}

/// Fall then get up (0x6869a).
fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            mv(x, 7);
        }
        2 => {
            if !hit_gate(x) || paused(x) {
                return;
            }
            let n = x.u16(Enemy::RECOVER).wrapping_sub(1);
            x.set16(Enemy::RECOVER, n);
            if (n as i16) > 0 {
                return;
            }
            x.set8(Enemy::ACTIVE, 2);
            x.add8(f::SUB2, 2);
            x.set16(Enemy::RECOVER, 0);
            x.set8(Enemy::DOWNED, 0);
            x.set8(Enemy::REACTION, 0);
        }
        4 => {
            if paused(x) {
                return;
            }
            if x.anim_step() {
                x.set8(Enemy::VULNERABLE, 1);
                set_state_long(x, STANCE);
                x.set8(Enemy::ACTIVE, 1);
                mv(x, 0);
            }
        }
        _ => {}
    }
}

/// Grabbed by a player: the Golem walks toward the nearest edge of the screen (0x687cc).
fn grab<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(Golem::GRABBED, 1);
            let at = x.u16(Obj::X);
            x.set16(Golem::TIMER2, at);
            let cam = if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA_ALT_X.get(x.b) } else { var::SCROLL_X.get(x.b) };
            let mut want = 0u8;
            x.set16(Obj::VX, -0x280i16 as u16);
            x.set16(Obj::AX, 0);
            let d = at.wrapping_sub(0xc0).wrapping_sub(cam);
            if (d as i16) > 0 {
                negate(x, Obj::VX);
                want = 0x20;
            }
            if x.u8(f::FLIP) ^ want == 0 {
                x.add8(f::SUB2, 2);
                return mv(x, 0);
            }
            mv(x, 1);
        }
        2 => {
            if !x.anim_step() {
                return;
            }
            let flip = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
            x.add8(f::SUB2, 2);
            mv(x, 0);
        }
        4 => {
            x.anim_step();
            if (x.u8(f::KEY_FRAME) as i8) >= 0 {
                x.move_x();
            }
            if !x.in_arena_default() {
                detach_from_host(x);
                set_state_long(x, st::DEAD_LONG);
                return;
            }
            if x.u8(Enemy::BLOCKED) == 0 {
                zone_and_terrain(x);
                if x.u16(Golem::TIMER2) != x.u16(Obj::X) {
                    let at = x.u16(Obj::X);
                    x.set16(Golem::TIMER2, at);
                    x.set8(Golem::TIMER, 0x3c);
                    return;
                }
                let n = x.u8(Golem::TIMER).wrapping_sub(1);
                x.set8(Golem::TIMER, n);
                if n != 0 {
                    return;
                }
            }
            x.add8(f::SUB2, 2);
            let anim = 0xc + x.u8(Enemy::GROUP) as u32;
            mv(x, anim);
        }
        6 => {
            if x.anim_step() {
                detach_from_host(x);
                set_state_long(x, st::DEAD_LONG);
            }
        }
        _ => {}
    }
}
