//! Riyakaa, active state: substate 2 (choice of an attack) and the helpers of the decisions.

use super::*;
use crate::actors::families::cx::Bench;
use crate::world::objects::Current;

// --- substate 2: choice of an attack ---------------------------------------------------------

pub(super) fn decide<R: Bench>(x: &mut Cx<'_, R>) {
    if grabbed(x) {
        x.set16(f::STATE, 0x0408);
        x.set16(f::SUB2, 0);
        return grab(x);
    }
    match x.u8(f::SUB2) {
        0 => {
            if x.u8(Enemy::SUB_STEP) != 0 {
                decide_follow(x);
            } else {
                look_begin(x);
            }
        }
        2 => decide_turn(x),
        4 => decide_wait(x),
        _ => {}
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        x.set32(f::STATE, 0x020a_0000);
    }
}

/// 0x6f546: start of the search for a target; the animation depends on the presence of a player very close.
pub(super) fn look_begin<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Enemy::SUB_STEP, 2);
    x.set8(Enemy::AI_POINT_SET, 6);
    x.set8(Enemy::VULNERABLE, 1);
    let (mut d4, mut d5, mut d6) = (0xd, 0, 2);
    if !lib::target_in_box(x, 0, 0xa0, 0x20) {
        x.set8(Riyakaa::WALK_PENDING, 1);
        d4 = 0x17;
        d6 = 3;
        d5 = 2;
    }
    x.set8(Riyakaa::POSE, d5);
    x.anim_start(BODY.0, d6);
    x.select_speed(&crate::world::objects::motion::SPEEDS_B, d4);
    lib::pick_target(x);
    lib::draw_stance(x);
    lib::draw_6b(x);
}

/// 0x6f5a6: follows the target as long as the gesture lasts.
pub(super) fn decide_follow<R: Bench>(x: &mut Cx<'_, R>) {
    lib::approach_target(x);
    let n = countdown(x, Enemy::AI_STANCE) as i8;
    if n < 0 {
        lib::draw_stance(x);
        if lib::pick_target(x).is_none() {
            return;
        }
        if countdown(x, Enemy::AI_WANDER) == 0 {
            lib::draw_6b(x);
            if lib::maybe_draw_6e(x) {
                x.set16(f::SUB2, 0x400);
                return;
            }
        }
    }
    decide_near(x);
}

/// 0x6f5e0: opponent very close (strike), within jumping range (leap) or to be reached (walk).
pub(super) fn decide_near<R: Bench>(x: &mut Cx<'_, R>) {
    if lib::target_in_box(x, -0x20, 0x20, 8) {
        x.set32(f::STATE, 0x0204_0c00);
        x.set8(Riyakaa::RESUME_STEP, 6);
        return;
    }
    if want_attack(x) {
        return choose_attack(x);
    }
    if lib::guard_in_front_strict(x, -0x38, 0x38, 8) {
        let v = x.u8(f::SUB2);
        x.set8(Riyakaa::RESUME_STEP, v);
        x.set16(f::SUB2, 0xc00);
        return lib::pick_byte(x);
    }
    x.anim_step();
    x.move_by_table_xy();
    if needs_turn(x) {
        x.add8(f::SUB2, 2);
        x.set8(Enemy::SUB_STEP, 0);
    }
}

/// 0x6f650: turns around toward the target.
pub(super) fn decide_turn<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::SUB_STEP) == 0 {
        x.add8(Enemy::SUB_STEP, 2);
        x.set8(Enemy::VULNERABLE, 1);
        return body(x, 4);
    }
    if !x.anim_step() {
        return;
    }
    toggle_flip(x);
    x.set8(f::SUB2, 0);
    body(x, 2);
}

/// 0x6f694: standing wait before setting off again.
pub(super) fn decide_wait<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::SUB_STEP) == 0 {
        x.add8(Enemy::SUB_STEP, 2);
        x.set8(Enemy::VULNERABLE, 1);
        x.set8(Riyakaa::GAZE, 1);
        if x.u8(Riyakaa::POSE) != 1 {
            x.set8(Riyakaa::POSE, 1);
            return body(x, 0);
        }
    }
    if (countdown(x, Enemy::AI_WAIT) as i8) < 0 {
        x.set16(f::SUB2, 0);
    }
    x.anim_step();
}

// --- common helpers --------------------------------------------------------------------------

/// 0x700fe: return to walking; a nearby player triggers substate 10.
pub(super) fn stand_up<R: Bench>(x: &mut Cx<'_, R>) {
    walk(x);
    if lib::maybe_draw_6e(x) {
        x.set8(f::SUB, 0xa);
    }
}

/// 0x700ca: steps back, then chooses the next gesture in a table (substate 8).
pub(super) fn retreat<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::AI_SHOT, 0xff);
    let v = rank_byte(x, NEXT_MOVES);
    x.set8(f::SUB, 8);
    x.set8(f::SUB2, v);
    x.set8(Enemy::SUB_STEP, 0);
}

/// 0x70116: a player in the box in front of it and the decision to attack is favourable.
pub(super) fn want_attack<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let found = lib::target_in_box(x, 0, 0x70, 8);
    if !found {
        return false;
    }
    if !lib::decision_b(x) {
        return false;
    }
    if !lib::decision_a(x) {
        return false;
    }
    true
}

/// 0x70150: is the target behind it (it must turn around)?
pub(super) fn needs_turn<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u16(f::OWNER) == 0 {
        return false;
    }
    let target = x.linked(f::OWNER);
    let d = x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X));
    let facing = if (d as i16) < 0 { 0x20 } else { 0 };
    facing != x.u8(f::FLIP)
}

/// 0x70178: draws the next attack gesture in a table; if the game allows it, goes back to substate 4 with this
/// gesture.
pub(super) fn choose_attack<R: Bench>(x: &mut Cx<'_, R>) {
    let pick = rank_byte(x, NEXT_ATTACKS);
    if pick > 4 {
        return;
    }
    if !lib::chance_10_16(x) {
        x.set8(f::SUB, 4);
        x.set8(Riyakaa::RESUME_STEP, pick);
        x.set8(Enemy::SUB_STEP, 0);
        x.set8(f::SUB2, 0xc);
    } else if x.u8(Riyakaa::WALK_PENDING) != 0 {
        speed(x, 0xd);
        x.set8(Riyakaa::WALK_PENDING, 0);
        body(x, 2);
    }
}
