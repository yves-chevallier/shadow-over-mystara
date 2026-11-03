//! Type 2, standing troglodyte (`+5 = 2`) and at rest (`+5 = 10`). `+6`: 0 walks toward the target and decides to
//! attack, 2 turn-around, 4 wait, 6 approach leap.

use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Trog::BLINK_ON) != 0 {
        let n = x.u16(Trog::BLINK_COUNT).wrapping_sub(1);
        x.set16(Trog::BLINK_COUNT, n);
        if (n as i16) < 0 {
            x.set8(Trog::BLINK_ON, 0);
            x.set_bit(Enemy::DISPLAY, 1, false);
        }
    }
    match x.u8(f::SUB2) {
        0 => {
            if x.u8(f::SUB3) == 0 {
                begin_walk(x);
            } else {
                walk(x);
            }
        }
        2 => turn(x),
        4 => wait(x),
        6 => jump(x),
        _ => {}
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 && (x.u16(Obj::Z) as i16) <= 0 {
        set_state_long(x, IDLE);
    }
}

/// 0x5a08a: the troglodyte starts walking toward its target.
fn begin_walk<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    x.set8(Enemy::AI_POINT_SET, 0x14);
    x.set8(Enemy::VULNERABLE, 1);
    x.set16(Trog::LOOK, 0);
    body(x, 2);
    walk_speed(x, 0);
    pick_target(x);
    lib::draw_stance(x);
    lib::draw_6b(x);
}

/// 0x5a0ce: walk; turns around, waits or attacks; grabbed by a player, it goes into a fall.
fn walk<R: Bench>(x: &mut Cx<'_, R>) {
    lib::approach_target(x);
    if grabbed(x) {
        x.set16(f::STATE, 0x408);
        x.set16(f::SUB2, 0);
        blink_if_running(x);
        return hit::caught(x);
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

/// 0x5a13c: a player in range? If so, attacks; otherwise advances by one animation step.
fn decide<R: Bench>(x: &mut Cx<'_, R>) {
    if lib::target_in_box(x, -0x20, 0x20, 8) {
        set_state_long(x, 0x0204_1600);
        x.set8(Trog::RESUME, 6);
        return blink_if_running(x);
    }
    if front_box(x) {
        return pick_attack_close(x);
    }
    if side_box(x) {
        return pick_attack_far(x);
    }
    if lib::guard_in_front_strict(x, -0x38, 0x38, 8) {
        let hint = x.u8(f::SUB2);
        x.set8(Trog::RESUME, hint);
        x.set16(f::SUB2, 0x1600);
        return lib::pick_byte(x);
    }
    step(x);
    move_xy(x);
    if x.u8(Enemy::GROUP) != 0 {
        if x.u8(Enemy::AI_POINT_SET) != 0x14 {
            return begin_jump(x);
        }
    } else if x.u8(Trog::BLINK_PHASE) != 0 && (x.u8(Trog::BLINK_PHASE) as i8) < 0 && (x.u16(Trog::BLINK_REST) as i16) < 0 {
        x.set8(Trog::BLINK_PHASE, 0);
    }
    if needs_turn_around(x) {
        x.add8(f::SUB2, 2);
        x.set8(f::SUB3, 0);
    }
}

/// 0x5b002: an attack is drawn from the difficulty row; it will be played after the announcement.
pub(super) fn pick_attack_close<R: Bench>(x: &mut Cx<'_, R>) {
    let table = if x.u8(Enemy::GROUP) != 0 { CLOSE_ATTACKS_ARMED } else { CLOSE_ATTACKS };
    let mv = rank_byte(x, table);
    *x.c.d4() = *x.c.d4() & !0xff | mv as u32;
    if mv <= 8 && lib::chance_10_16(x) {
            return x.set8(Enemy::AI_POINT_SET, 0x15);
        }
    blink_if_running(x);
    x.set8(f::SUB, 4);
    x.set8(Trog::RESUME, mv);
    x.set8(f::SUB3, 0);
    x.set8(f::SUB2, 0x16);
}

/// 0x5b474: approach attack, 0x12 or 0x14 at random.
pub(super) fn pick_attack_far<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB, 2);
    x.set8(Trog::RESUME, 0x12);
    if rand_and(x, 1) != 0 {
        x.add8(Trog::RESUME, 2);
    }
    x.set8(f::SUB3, 0);
    x.set8(f::SUB2, 0x16);
    blink_if_running(x);
}

/// 0x5a208: leap of series 1 toward the target.
fn begin_jump<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(f::SUB2, 6);
    x.set8(f::SUB3, 2);
    x.set16(Obj::VZ, 0x500);
    x.set16(Obj::AZ, -0x38i16 as u16);
    x.set16(Obj::VX, 0x300);
    x.set16(Obj::AX, 0);
    if x.u8(f::FLIP) == 0 {
        negate(x, Obj::VX);
    }
    body(x, 0xa);
}

/// `+6` = 6: approach leap (step 0: take-off, 2: climb and fall, 4: landing).
fn jump<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => begin_jump(x),
        2 => {
            x.move_xz();
            if step(x) && (x.u16(Obj::Z) as i16) < 0 {
                x.add8(f::SUB3, 2);
                x.set16(Obj::Z, 0);
                body(x, 0xc);
            }
        }
        4 => {
            if step(x) {
                x.set16(f::SUB2, 0);
                if needs_turn_around(x) {
                    x.set8(f::SUB2, 2);
                    x.set8(f::SUB3, 0);
                }
            }
        }
        _ => {}
    }
}

/// `+6` = 2: turn-around (animation 1, then orientation inversion).
fn turn<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(Enemy::VULNERABLE, 1);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        return body(x, 1);
    }
    if step(x) {
        x.set8(f::SUB2, 0);
        body(x, 2);
    }
}

/// `+6` = 4 : attente.
fn wait<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(Enemy::VULNERABLE, 1);
        x.set8(Trog::LOOK_PREV, 1);
        return body(x, 0);
    }
    let n = x.u8(Enemy::AI_WAIT).wrapping_sub(1);
    x.set8(Enemy::AI_WAIT, n);
    if (n as i8) < 0 {
        x.set8(f::SUB, 2);
        x.set16(f::SUB2, 0);
    }
    step(x);
}

/// Substate 10: rest until a player is alive.
pub(super) fn rest<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        return rest_begin(x);
    }
    if var::PLAYERS_ALIVE.get(x.b) != 0 {
        set_state_long(x, STANCE);
        pick_target(x);
    } else {
        step(x);
    }
}
