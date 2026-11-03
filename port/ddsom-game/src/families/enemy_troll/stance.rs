//! Type 14, substate 2 (standing: approach, turn-around, wait, screen exit) and substate 10 (stroll as long as no
//! player is alive).

use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Troll::NO_ZONE) != 0 || grabbed(x) {
        set_state_long(x, HELD);
        return grab(x);
    }
    match x.u8(f::SUB2) {
        0 => approach(x),
        2 => turn(x),
        4 => wait(x),
        6 => walk_off(x),
        _ => {}
    }
    // no player alive any more: the Troll goes to wait (the four state bytes of the players)
    let alive = [0x5de, 0x7de, 0x9de, 0xbde].iter().fold(0u8, |sum, &off| sum.wrapping_add(x.a5_u8(off)));
    if alive == 0 {
        set_state_long(x, IDLE);
    }
}

/// 0x6c81c: (re)starts the walk toward the target.
pub(super) fn approach_start<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
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
        x.set8(Enemy::AI_POINT_SET, 0x1a);
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
    // player in contact: immediate attack (throw or jump), at random
    if lib::target_in_box(x, -0x20, 0x20, 0xc) {
        x.set16(f::STATE, 0x0204);
        let r = rand_and(x, 4);
        let chosen = if r != 0 { 4 } else { 6 };
        x.set8(Troll::CHOSEN, chosen);
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
        x.set8(Troll::CHOSEN, chosen);
        x.set16(f::SUB2, 0);
        return lib::pick_byte(x);
    }
    x.set8(Troll::LOOK_PREV, 0x80);
    x.anim_step();
    x.move_by_table_xy();
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

/// +6 = 6 (and substate 4, +6 = 20): the Troll leaves the screen walking. The original returns control to the loop with
/// a re-queuing (`jmp 0x1ee1a`) in several places: it is replayed.
pub(super) fn walk_off<R: Bench>(x: &mut Cx<'_, R>) {
    let step = x.u8(f::SUB3);
    if step == 2 {
        if x.in_arena_default() {
            return x.add8(f::STATE, 4);
        }
        x.move_by_table_xy();
        x.anim_step();
        return x.reinsert();
    }
    if step > 2 {
        if x.anim_step() {
            set_speed(x, 0x21);
            x.add8(f::SUB3, 0xfe);
            let flip = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
        }
        return x.reinsert();
    }
    x.add8(f::SUB3, 2);
    x.set8(Obj::SPEED_INDEX, 8);
    let mut want = 0x20u8;
    let base = if var::CAMERA_SELECT.get(x.b) != 0 { 0x55e } else { 0x50e };
    let d = x.u16(Obj::X).wrapping_sub(x.a5_u16(base));
    if (d as i16) <= 0xc0 {
        x.set8(Obj::SPEED_INDEX, 8);
        want = 0;
    }
    if want != x.u8(f::FLIP) {
        x.add8(f::SUB3, 4);
        mv(x, 2);
    } else {
        set_speed(x, 0x21);
        mv(x, 1);
    }
    x.reinsert();
}

/// Substate 10: strolls in place as long as no player is alive, then back to standing.
pub(super) fn idle<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set8(Enemy::VULNERABLE, 1);
        return mv(x, 0);
    }
    let n = x.u8(Enemy::AI_WAIT).wrapping_sub(1);
    x.set8(Enemy::AI_WAIT, n);
    if (n as i8) < 0 {
        x.set8(f::SUB, 2);
        x.set16(f::SUB2, 0);
    }
    x.anim_step();
}
