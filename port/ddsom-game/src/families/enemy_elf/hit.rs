//! Type 12, hit state (`+4 = 4`, 0x6a160). `+5`: 0 and 6 common reaction by table (`+0x49`), 2 recoil, 4 fall, 8
//! grabbed by a player (the elf disappears).

use crate::families::cx::Bench;
use crate::data::RomAddr;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

/// Tables of the common reaction, indexed by reaction code `+0x49`.
const REACT_FIRST: RomAddr = RomAddr(0x6a206);
const REACT_SECOND: RomAddr = RomAddr(0x6a3a0);

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
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
    zone_and_terrain(x);
    x.reinsert();
}

/// Substate 2: the elf recoils (animation 7), then gets back on its feet.
fn recoil<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set8(f::FREEZE, 4);
        x.set32(Obj::VX, 0x0500_0000);
        x.set16(Obj::AX, -0x38i16 as u16);
        let flip = x.u8(Enemy::ATTACKER_FLIP) ^ 0x20;
        x.set8(Enemy::ATTACKER_FLIP, flip);
        if flip != 0 {
            negate(x, Obj::VX);
            negate(x, Obj::AX);
        }
        return body(x, 7);
    }
    if x.u8(Obj::VX) != 0 {
        x.move_x();
    }
    if step(x) {
        set_state_long(x, STANCE);
        x.set32(Enemy::HIT_LOCK, 0);
    }
}

/// Substate 4: fall (animation 6), wait lying down, get up.
fn down<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            body(x, 6);
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
            x.set8(Enemy::ACTIVE, 2);
            x.add8(f::SUB2, 2);
            body(x, 6);
        }
        4 => {
            if paused(x) {
                return;
            }
            if step(x) {
                x.set8(Enemy::DOWNED, 0);
                x.set8(Enemy::ACTIVE, 1);
                x.set8(Enemy::VULNERABLE, 1);
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}

/// Substate 8 (0x6a198): grabbed by a player; once animation 4 is over, the elf disappears.
pub(super) fn caught<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        return crouch(x);
    }
    if step(x) {
        detach_from_host(x);
        x.add8(f::STATE, 2);
    }
}
