//! Type 5, hit state (`+4 = 4`, 0x6089a). `+5`: 0 and 6 common reaction by table (`+0x49`), 2 recoil, 4 fall, 8 grabbed
//! by a player (the ghoul runs away).

use crate::families::cx::Bench;
use crate::data::RomAddr;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

/// Tables of the common reaction, indexed by reaction code `+0x49`.
const REACT_FIRST: RomAddr = RomAddr(0x6090c);
const REACT_SECOND: RomAddr = RomAddr(0x60abe);

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        if x.u8(f::SUB2) == 0 {
            x.set8(Ghoul::LOOK, 0xff);
        }
        match x.u8(f::SUB) {
            0 => {
                x.set8(Ghoul::LOOK, 0xff);
                if x.u8(Enemy::REACTION) == 0x64 || !special_hit(x) {
                    react(x, REACT_FIRST.0);
                }
            }
            2 => recoil(x),
            4 => down(x),
            6 => react(x, REACT_SECOND.0),
            8 => caught(x),
            _ => {}
        }
    }
    if x.u8(f::SUB) != 8 {
        zone_and_terrain(x);
    }
    x.reinsert();
}

/// Substate 2: the ghoul recoils (animation 9), then gets back on its feet.
fn recoil<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set8(f::FREEZE, 4);
        x.set16(Obj::VX, 0x500);
        x.set16(Obj::AX, -0x38i16 as u16);
        let flip = x.u8(Enemy::ATTACKER_FLIP) ^ 0x20;
        x.set8(Enemy::ATTACKER_FLIP, flip);
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
    x.set8(Enemy::ACTIVE, 1);
    body(x, 0);
}

/// Substate 4: fall (animation 6), wait lying down, get up.
fn down<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            body(x, 6);
        }
        2 => {
            if special_hit(x) || paused(x) {
                return;
            }
            let t = x.u16(Enemy::RECOVER).wrapping_sub(1);
            x.set16(Enemy::RECOVER, t);
            if (t as i16) > 0 {
                return;
            }
            x.set8(Enemy::ACTIVE, 2);
            x.set16(Enemy::RECOVER, 0);
            x.set8(Enemy::DOWNED, 0);
            x.set8(Enemy::REACTION, 0);
            x.add8(f::SUB2, 2);
            body(x, 7);
        }
        4 => {
            if paused(x) {
                return;
            }
            if step(x) {
                x.set8(Enemy::VULNERABLE, 1);
                x.set8(Enemy::ACTIVE, 1);
                set_state_long(x, STANCE);
                body(x, 0);
            }
        }
        _ => {}
    }
}

/// Substate 8 (0x60b6a): grabbed by a player, the ghoul turns around and runs off the screen.
fn caught<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            let pos = x.u16(Obj::X);
            x.set16(Ghoul::SAVED_X, pos);
            let cam = if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA_ALT_X.get(x.b) } else { var::SCROLL_X.get(x.b) };
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
            if x.u16(Ghoul::SAVED_X) != x.u16(Obj::X) {
                let pos = x.u16(Obj::X);
                x.set16(Ghoul::SAVED_X, pos);
                x.set8(Ghoul::TIMER, 0x3c);
                return {
                    step(x);
                };
            }
            let n = x.u8(Ghoul::TIMER).wrapping_sub(1);
            x.set8(Ghoul::TIMER, n);
            if n == 0 {
                detach_from_host(x);
                return set_state_long(x, st::DEAD_LONG);
            }
            step(x);
        }
        _ => {}
    }
}

/// 0x60bda: the ghoul runs (animation 2).
fn flee_begin<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 2);
    body(x, 2);
}
