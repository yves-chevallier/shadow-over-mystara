//! Type 0, hit state (`+4 = 4`, 0x567ca). `+5`: 0 reaction by table (`+0x49`), 2 recoil, 4 fall and get up, 6 second
//! reaction table, 8 grabbed by a player (the Kobold runs away).

use crate::actors::families::cx::Bench;
use crate::world::data::RomAddr;
use super::*;
use crate::world::objects::Current;
use crate::world::objects::Obj;

/// Reaction tables shared by the hits, indexed by code `+0x49`.
const REACT_FIRST: RomAddr = RomAddr(0x56816);
const REACT_SECOND: RomAddr = RomAddr(0x56a0e);

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        if freeze < 0 {
            x.set8(f::FREEZE, 0);
        }
        match x.u8(f::SUB) {
            0 => {
                x.set8(Kobold::LOOK_PREV, 0x7f);
                react(x, REACT_FIRST.0);
                zone_and_terrain(x);
            }
            2 => recoil(x),
            4 => fall(x),
            6 => {
                react(x, REACT_SECOND.0);
                zone_and_terrain(x);
            }
            8 => run_away(x),
            _ => {}
        }
    }
    x.reinsert();
}

// --- substate 2: recoil ------------------------------------------------------------------------------------

fn recoil<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            look(x, 0x1e);
            lib::sound(x, crate::hw::sound::CMD_34F_44F);
            x.set16(Obj::VX, 0x300);
            x.set16(Obj::AX, -0x28i16 as u16);
            let flip = x.u8(Enemy::ATTACKER_FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
            if flip != 0 {
                negate(x, Obj::VX);
                negate(x, Obj::AX);
            }
            mv(x, 0x1e);
        }
        2 => {
            puff(x, 3, 2, 8, -1);
            if x.u8(Obj::VX) != 0 {
                x.move_x();
            } else if x.anim_step() {
                x.set32(Enemy::HIT_LOCK, 0);
                x.set32(Obj::VX, 0);
                after_strike(x);
            }
        }
        _ => {}
    }
    zone_and_terrain(x);
}

// --- substate 4: fall and get up --------------------------------------------------------------------------

fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            mv(x, 0x15);
        }
        2 => {
            if (var::PAUSE.get(x.b) as i8) == 0 {
                let n = x.u16(Enemy::RECOVER).wrapping_sub(1);
                x.set16(Enemy::RECOVER, n);
                if (n as i16) <= 0 {
                    x.add8(f::SUB2, 2);
                    x.set8(Enemy::ACTIVE, 2);
                    x.set16(Enemy::RECOVER, 0);
                    x.set8(Enemy::DOWNED, 0);
                    x.set8(Enemy::REACTION, 0);
                }
            }
        }
        4 => {
            if var::PAUSE.get(x.b) == 0 && x.anim_step() {
                x.set8(Enemy::ACTIVE, 1);
                x.set8(Enemy::VULNERABLE, 1);
                set_state_long(x, STANCE);
                mv(x, 0);
            }
        }
        _ => {}
    }
    zone_and_terrain(x);
}

// --- substate 8: grabbed by a player ------------------------------------------------------------------------

fn run_away<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    match x.u8(f::SUB2) {
        0 => {
            let right = on_right_half(x);
            let flipped = x.u8(f::FLIP) != 0;
            // turns toward the nearest edge of the screen, or runs straight off
            let turn = if right { !flipped } else { flipped };
            if turn {
                x.add8(f::SUB2, 2);
                x.set8(Enemy::VULNERABLE, 1);
                look(x, 5);
                mv(x, 5);
            } else {
                x.add8(f::SUB2, 4);
                run_begin(x);
            }
        }
        2 => {
            if !x.anim_step() {
                return;
            }
            x.add8(f::SUB2, 2);
            let flip = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
            run_begin(x);
        }
        4 | 14 => {
            if var::SCENE.get(x.b) == 4 && x.u8(f::SUB2) == 4 {
                return x.add8(f::SUB2, 2);
            }
            x.move_by_table_xy();
            if !x.in_arena_default() {
                set_state_long(x, st::DEAD_LONG);
                return detach_from_host(x);
            }
            x.anim_step();
        }
        6 => {
            if x.u8(Enemy::BLOCKED) != 0 {
                x.add8(f::SUB2, 2);
            }
            zone_and_terrain(x);
            x.move_by_table_xy();
            x.anim_step();
        }
        8 => {
            x.add8(f::SUB2, 2);
            x.set8(Enemy::VULNERABLE, 1);
            look(x, 6);
            x.set16(Obj::VX, -0x380i16 as u16);
            if x.u8(f::FLIP) != 0 {
                negate(x, Obj::VX);
            }
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, 0x4c0);
            x.set16(Obj::AZ, -0x38i16 as u16);
            mv(x, 6);
        }
        10 => {
            if x.u8(f::KEY_FRAME) != 0 {
                x.add8(f::SUB2, 2);
                x.set8(Enemy::VULNERABLE, 0);
            }
            x.anim_step();
        }
        12 => {
            if x.move_xz().gt() {
                x.anim_step();
                return;
            }
            x.set32(Obj::Z, 0);
            x.add8(f::SUB2, 2);
            x.set8(Enemy::VULNERABLE, 1);
            look(x, 0x18);
            mv(x, 0x18);
        }
        _ => {}
    }
}

/// 0x56b42: the Kobold runs toward the edge of the screen (animation 0x16).
fn run_begin<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 1);
    x.set8(Kobold::LOOK, 0x16);
    set_angle(x);
    mv(x, 0x16);
    set_speed(x, 0x21);
}
