//! Type 13, substate 2 (standing: approach, turn-around, wait) and substate 10 (stroll as long as no player is alive).

use crate::actors::families::cx::Bench;
use super::*;
use crate::world::objects::Current;
use crate::world::objects::Obj;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    // variant 10 is grabbed as soon as it stands up
    if x.u8(Obj::VARIANT) == 0xa || grabbed(x) {
        x.set16(f::STATE, 0x0408);
        x.set16(f::SUB2, 0);
        return grab(x);
    }
    match x.u8(f::SUB2) {
        0 => approach(x),
        2 => turn(x),
        4 => wait(x),
        _ => {}
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        set_state_long(x, IDLE);
    }
}

/// 0x6affa: (re)starts the walk toward the target.
pub(super) fn approach_start<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    x.set8(Gnoll::LOOK_PREV, 0xf0);
    x.set8(Enemy::AI_POINT_SET, 0x1a);
    approach_restart(x);
}

/// 0x6b00a: continuation of `approach_start` without advancing the step (called directly after an attack).
pub(super) fn approach_restart<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 1);
    set_speed(x, 0x12);
    mv(x, 1);
    pick_target(x);
    lib::draw_stance(x);
    lib::draw_6b(x);
}

/// +6 = 0: walks toward the player; chooses a move as soon as it is in range.
fn approach<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        return approach_start(x);
    }
    lib::approach_target(x);
    let n = x.u8(Enemy::AI_STANCE).wrapping_sub(1);
    x.set8(Enemy::AI_STANCE, n);
    if (n as i8) < 0 {
        lib::draw_stance(x);
        if !pick_target(x) {
            return;
        }
        let m = x.u8(Enemy::AI_WANDER).wrapping_sub(1);
        x.set8(Enemy::AI_WANDER, m);
        if m == 0 {
            lib::draw_6b(x);
            if lib::maybe_draw_6e(x) {
                x.set16(f::SUB2, 0x400);
                return;
            }
        }
    }
    // player in contact: immediate attack (4 or 6), at random
    if lib::target_in_box(x, -0x20, 0x20, 0xc) {
        x.set16(f::STATE, 0x0204);
        let r = rand_and(x, 1);
        let chosen = (r * 2 + 4) as u8;
        x.set8(Gnoll::CHOSEN, chosen);
        x.set16(f::SUB2, 0);
        return;
    }
    if near_box(x) {
        pick_attack(x, NEAR_TABLE);
        return;
    }
    if far_box(x) {
        pick_attack(x, FAR_TABLE);
        return;
    }
    if lib::guard_in_front_strict(x, -0x38, 0x38, 0xc) {
        let chosen = x.u8(f::SUB2);
        x.set8(Gnoll::CHOSEN, chosen);
        x.set16(f::SUB2, 0);
        return lib::pick_byte(x);
    }
    x.anim_step();
    if x.u8(f::KEY_FRAME) & 1 == 0 {
        x.move_by_table_xy();
    }
    if faces_wrong(x) {
        x.add8(f::SUB2, 2);
        x.set8(f::SUB3, 0);
    }
}

/// +6 = 2 : demi-tour.
fn turn<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(Enemy::VULNERABLE, 1);
        return mv(x, 2);
    }
    if x.anim_step() {
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        x.set8(f::SUB2, 0);
        mv(x, 1);
    }
}

/// +6 = 4: waits in place.
fn wait<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(Enemy::VULNERABLE, 1);
        return mv(x, 0);
    }
    let n = x.u8(Enemy::AI_WAIT).wrapping_sub(1);
    x.set8(Enemy::AI_WAIT, n);
    if (n as i8) < 0 {
        x.set16(f::SUB2, 0);
    }
    x.anim_step();
}

/// Substate 10: strolls in place as long as no player is alive, then back to standing.
pub(super) fn idle<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set8(Enemy::VULNERABLE, 1);
        return mv(x, 0);
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        x.anim_step();
        return;
    }
    set_state_long(x, STANCE);
    pick_target(x);
}
