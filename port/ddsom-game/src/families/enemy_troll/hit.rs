//! Type 14, hit state (`+4 = 4`, 0x6d050). `+5`: 0 reaction by table (`+0x49`), 2 recoil, 4 fall and get up, 6 second
//! reaction table, 8 grabbed by a player (the Troll runs away). Zone and shadow are always tested.

use crate::families::cx::Bench;
use crate::data::RomAddr;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

/// Reaction tables shared by the hits, indexed by code `+0x49`.
const REACT_FIRST: RomAddr = RomAddr(0x6d0f0);
const REACT_SECOND: RomAddr = RomAddr(0x6d284);

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        x.set8(Enemy::ACTIVE, 1);
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

/// Recul (0x6d19c).
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
        return mv(x, 0xc);
    }
    if x.u8(Obj::VX) != 0 {
        x.move_x();
    }
    if x.anim_step() {
        set_state_long(x, STANCE);
    }
}

/// Fall then get up (0x6d202).
fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.anim_start(SET_HURT.0, 8);
        }
        2 => {
            if paused(x) {
                return;
            }
            let n = x.u16(Enemy::RECOVER).wrapping_sub(1);
            x.set16(Enemy::RECOVER, n);
            if (n as i16) > 0 {
                return;
            }
            x.set8(Enemy::DOWNED, 0);
            x.set8(Enemy::REACTION, 0);
            x.set8(Enemy::ACTIVE, 2);
            x.add8(f::SUB2, 2);
            mv(x, 6);
        }
        4 => {
            if paused(x) {
                return;
            }
            if x.anim_step() {
                x.set8(Enemy::DOWNED, 0);
                x.set8(Enemy::ACTIVE, 1);
                x.set8(VULNERABLE, 1);
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}
