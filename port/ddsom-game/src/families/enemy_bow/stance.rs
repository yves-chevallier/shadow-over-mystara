//! Type 3, substates 2 (standing: approach, walk), 8 (rest), 10 (wait) and 12 (turn-around).

use crate::families::cx::Bench;
use super::shoot::start_shot;
use super::*;
use crate::objects::Obj;
use crate::objects::Current;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Bow::EXPIRED) != 0 || grabbed(x) {
        set_state_long(x, HELD);
        return hit::grab(x);
    }
    match x.u8(f::SUB2) {
        0 => stance_begin(x),
        2 => advance(x),
        4 => strafe(x),
        _ => {}
    }
}

/// 0x5c17e: first standing step: chooses a target and faces it.
pub(super) fn stance_begin<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 2);
    if !pick_target(x) {
        set_state_long(x, IDLE);
        return idle_begin(x);
    }
    lib::draw_6b(x);
    x.set8(Enemy::AI_POINT_SET, 3);
    lib::approach_target(x);
    set_speed(x, 0xd);
    mv(x, 2);
}

/// 0x5c2de: start of the wait.
fn idle_begin<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 2);
    mv(x, 0);
}

/// +6 = 2: walks toward the target; shoots or dodges when it is in range.
fn advance<R: Bench>(x: &mut Cx<'_, R>) {
    if !x.in_arena_default() {
        x.set8(Enemy::AI_POINT_SET, 5);
        x.add8(f::SUB2, 2);
        x.set8(Bow::TIMER, 0x20);
        return strafe(x);
    }
    let n = x.u8(Enemy::AI_STANCE).wrapping_sub(1);
    x.set8(Enemy::AI_STANCE, n);
    if (n as i8) < 0 {
        let mut waiting = true;
        if pick_target(x) {
            lib::draw_stance(x);
            let m = x.u8(Enemy::AI_WANDER).wrapping_sub(1);
            x.set8(Enemy::AI_WANDER, m);
            if (m as i8) >= 0 {
                waiting = false;
            }
        }
        if waiting && lib::maybe_draw_6e(x) {
                set_state_long(x, IDLE);
                return check_facing(x);
            }
    }
    lib::approach_target(x);
    x.move_by_table_xy();
    x.anim_step();
    if point_blank(x) {
        set_state_long(x, 0x0204_0202);
        return start_shot(x);
    }
    decide(x)
}

/// 0x5c22c: a player within shooting range? Then shoot or dodge depending on the box that saw it.
pub(super) fn decide<R: Bench>(x: &mut Cx<'_, R>) {
    if in_front(x) || on_line(x) {
        // the target seen by the box (A1 in the original: target `+0x50`, non-zero since the box found it); D5 says
        // which box (0 in front, 2 on the line)
        let found = x.linked(f::TARGET);
        let facing = x.u8(f::FLIP);
        let same_side = facing == found.u8(x.b, f::FLIP);
        if lib::decision(x, same_side) {
            set_state_long(x, 0x0204_0000);
            let d5 = *x.c.d5() as u8;
            x.set8(f::SUB2, d5);
            return check_facing(x);
        }
        if lib::guard_in_front_strict(x, -0x38, 0x38, 0x10) {
            x.set8(f::SUB, 0xa);
            x.set16(f::SUB2, 0);
        }
    }
    check_facing(x)
}

/// 0x5c27a: if the Bow Gnoll has its back to its target, turn around (state to resume saved in `+0xa0`).
pub(super) fn check_facing<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u16(f::TARGET) == 0 {
        return;
    }
    let target = load_target(x);
    let dx = x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X));
    let want = if (dx as i16) < 0 { 0x20 } else { 0 };
    if want as u8 == x.u8(f::FLIP) {
        return;
    }
    let state = x.u32(f::STATE);
    x.set32(Bow::RESUME_STATE, state);
    x.set8(Bow::RESUME_SUB2, 0);
    x.set8(f::SUB, 0xc);
    x.set16(f::SUB2, 0);
}

/// +6 = 4: lateral walk for `+0xa4` frames, then back to the approach.
fn strafe<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.u8(Bow::TIMER).wrapping_sub(1);
    x.set8(Bow::TIMER, n);
    if (n as i8) < 0 {
        x.set8(f::SUB2, 2);
        x.set8(Enemy::AI_POINT_SET, 3);
        return advance(x);
    }
    lib::approach_target(x);
    x.move_by_table_xy();
    x.anim_step();
}

// --- substate 8: rest ------------------------------------------------------------------------------

pub(super) fn rest<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            set_state_long(x, IDLE);
            x.set8(Enemy::AI_WAIT, 0x1e);
            idle_begin(x);
        }
        2 => side_step(x, true),
        4 => side_step(x, false),
        6 => {
            set_state_long(x, STANCE);
            decide(x);
        }
        _ => {}
    }
}

/// +6 = 2 (in X, looks toward the target) and 4 (in depth): side step for `+0x6b` frames.
fn side_step<R: Bench>(x: &mut Cx<'_, R>, along_x: bool) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(Enemy::AI_WANDER, 0x18);
        let target = load_target(x);
        if along_x {
            x.set8(Obj::SPEED_INDEX, 0x18);
            let d = x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X));
            if (d as i16) >= 0 {
                x.set8(Obj::SPEED_INDEX, 8);
            }
        } else {
            x.set8(Obj::SPEED_INDEX, 0xf);
            let d = x.u16(Obj::Y).wrapping_sub(target.u16(x.b, Obj::Y));
            if (d as i16) >= 0 {
                x.set8(Obj::SPEED_INDEX, 0);
            }
        }
        return mv(x, 2);
    }
    let n = x.u8(Enemy::AI_WANDER).wrapping_sub(1);
    x.set8(Enemy::AI_WANDER, n);
    if (n as i8) < 0 {
        set_state_long(x, STANCE);
    }
    x.move_by_table_xy();
    x.anim_step();
}

// --- substate 10: wait ---------------------------------------------------------------------------

pub(super) fn idle<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        return idle_begin(x);
    }
    let n = x.u8(Enemy::AI_WAIT).wrapping_sub(1);
    x.set8(Enemy::AI_WAIT, n);
    if (n as i8) < 0 {
        x.set8(f::SUB, 2);
        x.set16(f::SUB2, 0);
    }
    x.anim_step();
}

// --- substate 12: turn-around -------------------------------------------------------------------------

pub(super) fn turn_around<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            mv(x, 3);
        }
        2 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                let state = x.u32(Bow::RESUME_STATE);
                x.set32(f::STATE, state);
                mv(x, 0);
            }
        }
        _ => {}
    }
}
