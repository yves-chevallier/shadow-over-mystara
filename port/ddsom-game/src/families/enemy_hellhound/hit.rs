//! Type 6, hit state (`+4 = 4`, 0x62a54). `+5`: 0 reaction by table (`+0x49`), 2 recoil, 4 fall and get up, 6 second
//! reaction table, 8 grabbed by a player (the dog runs off the screen).

use crate::families::cx::Bench;
use crate::data::RomAddr;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

/// Reaction tables shared by the hits, indexed by code `+0x49`.
const REACT_FIRST: RomAddr = RomAddr(0x62aa2);
const REACT_SECOND: RomAddr = RomAddr(0x62d76);

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    hit_loop_at(x, Hellhound::NO_ZONE, |x| {
        match x.u8(f::SUB) {
            0 => {
                x.set8(Hellhound::LOOK_PREV, 0x7f);
                react(x, REACT_FIRST.0);
            }
            2 => recoil(x),
            4 => fall(x),
            6 => react(x, REACT_SECOND.0),
            8 => run_away(x),
            _ => {}
        }
    });
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
            x.set16(Obj::VX, 0x300);
            x.set16(Obj::AX, -0x28i16 as u16);
            let flip = x.u8(Enemy::ATTACKER_FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
            if flip != 0 {
                negate(x, Obj::VX);
                negate(x, Obj::AX);
            }
            mv(x, 0xd);
        }
        2 => {
            puff(x, 3, 0, 2, 0x18, -1);
            if x.u8(Obj::VX) != 0 {
                x.move_x();
                return;
            }
            if x.anim_step() {
                x.set32(Enemy::HIT_LOCK, 0);
                x.set32(Obj::VX, 0);
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}

// --- substate 4: fall and get up --------------------------------------------------------------------------

fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            mv(x, 0xe);
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
            x.add8(f::SUB2, 2);
            x.set8(Enemy::ACTIVE, 2);
            x.set16(Enemy::RECOVER, 0);
            x.set8(Enemy::DOWNED, 0);
            x.set8(Enemy::REACTION, 0);
        }
        4 => {
            if paused(x) || !x.anim_step() {
                return;
            }
            if !lib::target_in_box(x, 0, 0x40, 8) {
                x.set8(Enemy::DOWNED, 0);
                x.set8(Enemy::ACTIVE, 1);
                x.set8(Enemy::VULNERABLE, 1);
                set_state_long(x, STANCE);
                return mv(x, 0);
            }
            x.add8(f::SUB2, 2);
            x.set16(Obj::VX, 0x300);
            if x.u8(f::FLIP) != 0 {
                negate(x, Obj::VX);
            }
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, 0x380);
            x.set16(Obj::AZ, -0x38i16 as u16);
            mv(x, 6);
            let r = rand_and(x, 1);
            if r != 0 {
                x.set16(Obj::VX, -0x300i16 as u16);
                if x.u8(f::FLIP) != 0 {
                    negate(x, Obj::VX);
                }
                mv(x, 5);
            }
        }
        6 => {
            if x.u8(f::KEY_FRAME) != 0 {
                x.add8(f::SUB2, 2);
                x.set8(Enemy::VULNERABLE, 0);
            }
            x.anim_step();
        }
        8 => {
            if x.move_xz().gt() {
                x.anim_step();
                return;
            }
            x.set32(Obj::Z, 0);
            dust(x);
            x.add8(f::SUB2, 2);
            x.set8(Enemy::VULNERABLE, 1);
            look(x, 0xc);
            mv(x, 0xc);
        }
        10 => {
            if x.anim_step() {
                x.set8(Enemy::ACTIVE, 1);
                x.set8(Enemy::DOWNED, 0);
                x.set8(Enemy::VULNERABLE, 1);
                set_state_long(x, STANCE);
                mv(x, 0);
            }
        }
        _ => {}
    }
}

// --- substate 8: grabbed by a player ------------------------------------------------------------------------

fn run_away<R: Bench>(x: &mut Cx<'_, R>) {
    if paused(x) {
        return;
    }
    match x.u8(f::SUB2) {
        0 => {
            x.set8(Hellhound::NO_ZONE, 1);
            let to_right = if var::SCENE.get(x.b) == 4 {
                x.u8(f::FLIP) == 0
            } else {
                let left = !lib::in_right_half(x);
                left == (x.u8(f::FLIP) == 0)
            };
            if to_right {
                x.add8(f::SUB2, 4);
                return run_begin(x);
            }
            x.add8(f::SUB2, 2);
            x.set8(Enemy::VULNERABLE, 1);
            look(x, 0x7f);
            mv(x, 4);
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
        4 => {
            x.anim_step();
            x.move_by_table_xy();
            if !x.in_arena_default() {
                set_state_long(x, st::DEAD_LONG);
                detach_from_host(x);
            }
        }
        _ => {}
    }
}

/// 0x62ec0: the dog runs toward the edge of the screen.
fn run_begin<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 1);
    set_angle(x);
    mv(x, 3);
    set_speed(x, 0x25);
}
