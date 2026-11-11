//! Type 1, hit state (`+4 = 4`, 0x58ca6). `+5`: 0 and 6 common reaction by table (`+0x49`), 2 recoil (counter-attack
//! possible), 4 fall, 8 flight off the screen, 10 nothing.

use crate::actors::families::cx::Bench;
use crate::world::data::RomAddr;
use super::*;
use crate::world::objects::Current;
use crate::world::objects::Obj;

/// Tables of the common reaction, indexed by reaction code `+0x49`.
const REACT_FIRST: RomAddr = RomAddr(0x58d0a);
const REACT_SECOND: RomAddr = RomAddr(0x58f2e);

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        match x.u8(f::SUB) {
            0 => {
                // code 0x64 is never a special reaction
                if x.u8(Enemy::REACTION) == 0x64 || !special_hit(x) {
                    react(x, REACT_FIRST.0);
                }
            }
            2 => recoil(x),
            4 => down(x),
            6 => react(x, REACT_SECOND.0),
            8 => flee(x),
            _ => {}
        }
    }
    if x.u8(f::SUB) != 8 {
        zone_and_terrain(x);
    }
    x.reinsert();
}

// --- substate 2: recoil ------------------------------------------------------------------------

fn recoil<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) || special_hit(x) {
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        lib::sound(x, crate::services::sound::CMD_350_450);
        let freeze = x.u8(Enemy::STUN) >> 1;
        x.set8(f::FREEZE, freeze);
        x.set16(Skeleton::TIMER, 0xf);
        x.set16(Obj::VX, 0x480);
        x.set16(Obj::AX, -0x80i16 as u16);
        let flip = x.u8(Enemy::ATTACKER_FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        if flip != 0 {
            negate(x, Obj::VX);
            negate(x, Obj::AX);
        }
        return body(x, 4);
    }
    // recoils for `Skeleton::TIMER` frames, then gets up
    let t = x.u16(Skeleton::TIMER);
    if t != 0 {
        x.set16(Skeleton::TIMER, t - 1);
        if t - 1 != 0 {
            step(x);
            return;
        }
    }
    if x.u8(Obj::VX) != 0 {
        x.move_x();
        return;
    }
    x.set32(Enemy::HIT_LOCK, 0);
    if x.u8(Enemy::AI_68) == 9 {
        let posture = if x.u8(Enemy::GROUP) != 0 { 8 } else { 3 };
        x.set8(Enemy::AI_68, posture);
        let counter = x.u8(Skeleton::COUNTER) as i8;
        if counter != 0 && (counter > 0 || rand_and(x, 1) != 0) {
            // counter-attack: attack `+0xa9` of the attack series
            x.set16(f::STATE, 0x204);
            let mv = x.u8(Skeleton::COUNTER_MOVE);
            x.set8(f::SUB2, mv);
            x.set8(f::SUB3, 0);
            x.set16(Skeleton::COUNTER, 0);
            return;
        }
    }
    x.set16(Skeleton::COUNTER, 0);
    set_state_long(x, STANCE);
    body(x, 0);
}

// --- substate 4: fall --------------------------------------------------------------------------

fn down<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            body(x, 7);
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
            x.add8(f::SUB2, 2);
            x.set16(Enemy::RECOVER, 0);
            x.set8(Enemy::DOWNED, 0);
            x.set8(Enemy::REACTION, 0);
        }
        4 => {
            if paused(x) {
                return;
            }
            if step(x) {
                x.set8(Enemy::VULNERABLE, 1);
                set_state_long(x, STANCE);
                x.set8(Enemy::ACTIVE, 1);
                body(x, 0);
            }
        }
        _ => {}
    }
}

// --- substate 8: flight ------------------------------------------------------------------------

fn flee<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(Skeleton::CAUGHT, 1);
            let x0 = x.u16(Obj::X);
            x.set16(Skeleton::SAVED_POS, x0);
            let cam = if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA_ALT_X.get(x.b) } else { var::SCROLL_X.get(x.b) };
            *x.c.d2() = 0;
            x.set16(Obj::VX, -0x280i16 as u16);
            x.set16(Obj::AX, 0);
            let edge = x0.wrapping_sub(0xc0) as i16;
            if edge > cam as i16 {
                negate(x, Obj::VX);
                *x.c.d2() = 0x20;
            }
            if x.u8(f::FLIP) != (*x.c.d2()) as u8 {
                body(x, 1);
            } else {
                x.add8(f::SUB2, 2);
                body(x, 0);
            }
        }
        2 => {
            if !step(x) {
                return;
            }
            let flip = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
            x.add8(f::SUB2, 2);
            body(x, 0);
        }
        4 => run_away(x),
        6 => {
            if step(x) {
                detach_from_host(x);
                set_state_long(x, st::DEAD_LONG);
            }
        }
        _ => {}
    }
}

/// 0x59082: runs toward the edge of the screen; off screen, it disappears.
fn run_away<R: Bench>(x: &mut Cx<'_, R>) {
    step(x);
    if (x.u8(f::KEY_FRAME) as i8) >= 0 {
        x.move_x();
    }
    if !x.in_arena_default() {
        detach_from_host(x);
        return set_state_long(x, st::DEAD_LONG);
    }
    if x.u8(Enemy::BLOCKED) == 0 {
        zone_and_terrain(x);
        if x.u16(Skeleton::SAVED_POS) != x.u16(Obj::X) {
            let x0 = x.u16(Obj::X);
            x.set16(Skeleton::SAVED_POS, x0);
            x.set8(Skeleton::TIMER, 0x14);
            return;
        }
        let n = x.u8(Skeleton::TIMER).wrapping_sub(1);
        x.set8(Skeleton::TIMER, n);
        if n != 0 {
            return;
        }
    }
    // motionless: it goes back to playing the normal animation
    x.add8(f::SUB2, 2);
    let series = x.u8(Enemy::GROUP);
    if series != 0 {
        let v = x.u8(Enemy::PALETTE) | 0x80;
        x.set8(Enemy::PENDING_PALETTE, v);
    }
    x.set8(Enemy::OWN_PALETTE, 0);
    body(x, 0xc + series as u32);
}
