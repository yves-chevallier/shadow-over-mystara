//! Type 5, standing ghoul (`+5 = 2`) and patrolling (`+5 = 10`). `+6`: 0 and 6 walk toward the target and decide to
//! attack, 2 turn-around, 4 wait.

use crate::actors::families::cx::Bench;
use super::*;
use crate::world::objects::Obj;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if special_hit(x) {
        return;
    }
    if grabbed(x) {
        return set_state_long(x, KNOCKED);
    }
    match x.u8(f::SUB2) {
        0 | 6 => {
            if x.u8(f::SUB3) == 0 {
                begin_walk(x);
            } else {
                walk(x);
            }
        }
        2 => turn(x),
        4 => wait(x),
        _ => {}
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        set_state_long(x, IDLE);
    }
}

/// The wanted look `v`; animation `anim` is started (and true returned) only if the look changes.
fn set_look<R: Bench>(x: &mut Cx<'_, R>, v: u8, anim: u32) -> bool {
    x.set8(Ghoul::LOOK, v);
    if v == x.u8(Ghoul::LOOK_PREV) {
        return false;
    }
    x.set8(Ghoul::LOOK_PREV, v);
    body(x, anim);
    true
}

/// 0x5fcec: the ghoul starts walking toward its target.
pub(super) fn begin_walk<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    x.set8(Enemy::VULNERABLE, 1);
    set_look(x, 0, 2);
    set_speed(x, 0xd);
    pick_target(x);
    lib::draw_stance(x);
    lib::draw_6b(x);
}

/// 0x5fd3e: walk; turns around, waits or attacks.
fn walk<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        x.set8(Enemy::AI_POINT_SET, 0);
        lib::walk_near_point(x);
    } else {
        x.set8(Enemy::AI_POINT_SET, 9);
        lib::approach_target(x);
    }
    let n = x.u8(Enemy::AI_STANCE).wrapping_sub(1);
    x.set8(Enemy::AI_STANCE, n);
    if (n as i8) < 0 {
        lib::draw_stance(x);
        if !pick_target(x) {
            return;
        }
        let n = x.u8(Enemy::AI_WANDER).wrapping_sub(1);
        x.set8(Enemy::AI_WANDER, n);
        if n == 0 {
            lib::draw_6b(x);
            if lib::maybe_draw_6e(x) {
                return x.set16(f::SUB2, 0x400);
            }
        }
    }
    decide(x);
}

/// 0x5fd96: a player in range? If so, attacks; otherwise advances by one animation step.
fn decide<R: Bench>(x: &mut Cx<'_, R>) {
    if lib::target_in_box(x, -0x20, 0x20, 8) {
        x.set16(f::STATE, 0x204);
        return pick_attack(x, CLOSE_ATTACKS, 0);
    }
    if box_attack(x, 0, 0x50, 8, CLOSE_ATTACKS, 0) || box_attack(x, -0x80, 0x30, 8, FAR_ATTACKS, 0xa) {
        return;
    }
    if lib::guard_in_front_strict(x, -8, 0x58, 0x18) {
        return lib::pick_byte(x);
    }
    step(x);
    move_xy(x);
    if needs_turn_around(x) {
        x.set16(f::SUB2, 0x200);
    }
}

/// `+6` = 2: turn-around (animation 1, then orientation inversion).
fn turn<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        look(x, 2);
        x.set8(Enemy::VULNERABLE, 1);
        return body(x, 1);
    }
    if step(x) {
        x.set16(f::SUB2, 0);
        body(x, 0);
    }
}

/// `+6` = 4 : attente.
fn wait<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(Enemy::VULNERABLE, 1);
        if set_look(x, 4, 0) {
            return;
        }
    }
    let n = x.u8(Enemy::AI_WAIT).wrapping_sub(1);
    x.set8(Enemy::AI_WAIT, n);
    if (n as i8) < 0 {
        x.set16(f::SUB2, 0);
    }
    step(x);
}

/// Substate 10: patrols as long as no player is alive.
pub(super) fn patrol<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(Ghoul::TIMER, 0x20);
            set_angle(x, 8, 0x18);
            set_speed(x, 3);
            body(x, 2);
        }
        2 => {
            let t = x.u8(Ghoul::TIMER).wrapping_sub(1);
            x.set8(Ghoul::TIMER, t);
            if t == 0 {
                x.add8(f::SUB2, 2);
                x.set8(Ghoul::TIMER, 0x40);
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                body(x, 1);
            } else {
                move_xy(x);
                step(x);
            }
        }
        4 => {
            if step(x) {
                x.set8(f::SUB2, 2);
                x.add8(Obj::SPEED_INDEX, 0x10);
                let a = x.u8(Obj::SPEED_INDEX) & 0x1f;
                x.set8(Obj::SPEED_INDEX, a);
                body(x, 2);
            }
        }
        _ => {}
    }
    if var::PLAYERS_ALIVE.get(x.b) != 0 {
        set_state_long(x, STANCE);
    }
}
