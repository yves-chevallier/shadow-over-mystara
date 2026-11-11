//! Type 2, hit state (`+4 = 4`, 0x5ab72). `+5`: 0 and 6 common reaction by table (`+0x49`), 2 recoil, 4 fall, 8 grabbed
//! by a player (the troglodyte runs away).

use crate::actors::families::cx::Bench;
use crate::world::data::RomAddr;
use super::*;
use crate::world::objects::Current;
use crate::world::objects::Obj;

/// Tables of the common reaction, indexed by reaction code `+0x49`.
const REACT_FIRST: RomAddr = RomAddr(0x5acfc);
const REACT_SECOND: RomAddr = RomAddr(0x5ae8e);

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::GROUP) == 0 {
        blink_hit(x);
    }
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        match x.u8(f::SUB) {
            0 => {
                if x.u8(f::SUB2) == 0 {
                    x.set_bit(Enemy::DISPLAY, 1, false);
                }
                react(x, REACT_FIRST.0);
            }
            2 => recoil(x),
            4 => down(x),
            6 => react(x, REACT_SECOND.0),
            8 => caught(x),
            _ => {}
        }
    }
    if x.u8(Trog::NO_SHADOW) == 0 {
        zone_and_terrain(x);
    }
    x.reinsert();
}

/// The blinking of series 0 continues during the hit state (0x5ab72).
fn blink_hit<R: Bench>(x: &mut Cx<'_, R>) {
    let phase = x.u8(Trog::BLINK_PHASE) as i8;
    if phase <= 0 {
        return;
    }
    if phase == 1 {
        x.set_bit(Enemy::DISPLAY, 1, false);
        x.add8(Trog::BLINK_PHASE, 1);
        x.set8(Trog::BLINK_ON, 1);
        x.set16(Trog::BLINK_COUNT, 0xc);
        x.set8(Trog::BLINK_WAIT, 0);
    }
    let n = x.u16(Trog::BLINK_COUNT).wrapping_sub(1);
    x.set16(Trog::BLINK_COUNT, n);
    if (n as i16) < 0 {
        x.set8(Trog::BLINK_ON, 0);
        x.set8(Trog::BLINK_WAIT, 0);
        x.set8(Trog::BLINK_PHASE, 0xff);
        x.set16(Trog::BLINK_REST, 0x258);
        x.set_bit(Enemy::DISPLAY, 1, true);
    }
    let v = x.u8(Enemy::DISPLAY) ^ 2;
    x.set8(Enemy::DISPLAY, v);
}

/// Substate 2: the troglodyte recoils, then gets back on its feet.
fn recoil<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set8(f::FREEZE, 4);
        x.set16(Obj::VX, 0x500);
        x.set16(Obj::AX, -0x38i16 as u16);
        lib::sound(x, crate::services::sound::CMD_127_227_B);
        let flip = x.u8(Enemy::ATTACKER_FLIP) ^ 0x20;
        x.set8(Enemy::ATTACKER_FLIP, flip);
        if flip != 0 {
            negate(x, Obj::VX);
            negate(x, Obj::AX);
        }
        if x.u8(Trog::LOOK_PREV) != 0xdf {
            return body(x, 0xd);
        }
    }
    if x.u8(Obj::VX) != 0 {
        x.move_x();
        return;
    }
    set_state_long(x, STANCE);
    x.set8(Enemy::ACTIVE, 1);
    x.set32(Enemy::HIT_LOCK, 0);
    x.set8(Enemy::AI_68, 4);
}

/// Substate 4: fall, wait lying down, get up.
fn down<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            body(x, 7);
        }
        2 => {
            if paused(x) {
                return;
            }
            let t = x.u16(Enemy::RECOVER).wrapping_sub(1);
            x.set16(Enemy::RECOVER, t);
            if (t as i16) > 0 {
                return;
            }
            x.set8(Enemy::DOWNED, 0);
            x.set8(Enemy::REACTION, 0);
            x.add8(f::SUB2, 2);
        }
        4 => {
            if paused(x) {
                return;
            }
            if step(x) {
                x.set8(Enemy::VULNERABLE, 1);
                x.set8(Enemy::ACTIVE, 1);
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}

/// Substate 8 (0x5ac08): grabbed by a player, the troglodyte turns around and runs away.
pub(super) fn caught<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    match x.u8(f::SUB2) {
        0 => {
            x.set8(Trog::NO_SHADOW, 1);
            let left = !lib::in_right_half(x);
            let flipped = x.u8(f::FLIP) != 0;
            let run_now = if left { !flipped } else { flipped };
            if run_now {
                x.add8(f::SUB2, 4);
                run_away(x);
            } else {
                x.add8(f::SUB2, 2);
                x.set8(Enemy::VULNERABLE, 1);
                x.set8(Trog::LOOK, 0x7f);
                x.set8(Trog::LOOK_PREV, 0x7f);
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                body(x, 1);
            }
        }
        2 => {
            if step(x) {
                x.add8(f::SUB2, 2);
                run_away(x);
            }
        }
        4 => {
            step(x);
            move_xy(x);
            if !x.in_arena_default() {
                set_state_long(x, st::DEAD_LONG);
                detach_from_host(x);
            }
        }
        _ => {}
    }
}

/// 0x5ac8c: runs away toward the edge of the screen.
fn run_away<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 1);
    x.set8(Obj::SPEED_INDEX, 8);
    if x.u8(f::FLIP) == 0 {
        x.set8(Obj::SPEED_INDEX, 0x18);
    }
    body(x, 2);
    x.select_speed(&crate::world::objects::motion::SPEEDS_B, 0x21);
}
