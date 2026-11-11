//! Type 0, substate 2: standing. `+6`: 0 and 8 approach and choice of a move, 2 turn-around, 4 wait, 6 run.

use crate::actors::families::cx::Bench;
use super::*;
use crate::world::objects::Current;
use crate::world::objects::Obj;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        return set_state_long(x, IDLE);
    }
    if matches!(x.u8(Obj::VARIANT), 0xa | 0xb) || grabbed(x) {
        return set_state_long(x, HELD);
    }
    match x.u8(f::SUB2) {
        0 | 8 => approach(x),
        2 => turn(x),
        4 => wait(x),
        6 => run_up(x),
        _ => {}
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        set_state_long(x, IDLE);
    }
}

// --- +6 = 0 and 8: approach -----------------------------------------------------------------------------

fn approach<R: Bench>(x: &mut Cx<'_, R>) {
    if target_far_in_line(x) {
        return set_state_long(x, 0x0202_0600);
    }
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(Enemy::VULNERABLE, 1);
        x.set8(Kobold::LOOK, 0xc);
        mv(x, 0xc);
        set_speed(x, 0xb);
        return lib::draw_stance(x);
    }
    let n = x.u8(Enemy::AI_STANCE).wrapping_sub(1);
    x.set8(Enemy::AI_STANCE, n);
    if (n as i8) < 0 {
        lib::draw_stance(x);
        pick_target(x);
        let kind = if x.u8(f::SUB2) == 0 { 0xe } else { 0xf };
        x.set8(Enemy::AI_POINT_SET, kind);
        lib::approach_target(x);
        let w = x.u8(Enemy::AI_WANDER).wrapping_sub(1);
        x.set8(Enemy::AI_WANDER, w);
        if w == 0 {
            lib::draw_6b(x);
            if lib::maybe_draw_6e(x) {
                return x.set16(f::SUB2, 0x400);
            }
        }
    }
    reach(x);
}

/// 0x555d4: on guard; a player very close? Immediate attack, otherwise range boxes, otherwise walking step.
pub(super) fn reach<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Kobold::GUARD, 2);
    reach_boxes(x);
}

/// 0x555da: successive range boxes.
pub(super) fn reach_boxes<R: Bench>(x: &mut Cx<'_, R>) {
    if lib::target_in_box(x, -0x28, 0x18, 4) {
        set_state_long(x, 0x0204_0200);
        return lib::pick_byte(x);
    }
    if front_attack(x) || far_attack(x) {
        return;
    }
    if lib::guard_in_front_strict(x, -0x30, 0x30, 0x20) {
        return lib::pick_byte(x);
    }
    x.anim_step();
    x.move_by_table_xy();
    if x.u16(f::TARGET) == 0 {
        return;
    }
    let target = x.linked(f::TARGET);
    let side = if (x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X)) as i16) < 0 { 0x20 } else { 0 };
    if side != x.u8(f::FLIP) {
        x.set32(Kobold::RESUME_STATE, STANCE);
        x.set8(Kobold::RESUME_SUB2, 0);
        set_state_long(x, 0x0202_0200);
    }
}

/// Attack decision according to the relative orientation of the target (0xa4c2 same direction, 0xa4b4 otherwise).
fn gate<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let target = x.linked(f::TARGET);
    let theirs = target.u8(x.b, f::FLIP);
    let same = theirs == x.u8(f::FLIP);
    lib::decision(x, same)
}

/// 0x5566e: a player in front, very close: attack of the short series or new step.
fn front_attack<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if !lib::target_in_box(x, -0x40, 0x20, 2) || x.u16(f::TARGET) == 0 || !gate(x) {
        return false;
    }
    let r = rand_and(x, 0xf);
    if (0xffc0u32 >> r) & 1 != 0 {
        set_state_long(x, 0x0202_0800);
    } else {
        set_state_long(x, 0x0204_0000);
        lib::pick_byte(x);
        pick_attack(x, 0);
    }
    true
}

/// 0x556e8: a player further in front: move from the far series.
fn far_attack<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if !lib::target_in_box(x, -0xb0, 0x20, 2) || x.u16(f::TARGET) == 0 || !gate(x) {
        return false;
    }
    let r = rand_and(x, 2);
    if r != 0 {
        set_state_long(x, 0x0202_0800);
    } else {
        set_state_long(x, 0x0204_0000);
        lib::pick_byte(x);
        pick_attack(x, 0x14);
    }
    true
}

// --- +6 = 2 : demi-tour --------------------------------------------------------------------------------------

fn turn<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(Enemy::VULNERABLE, 1);
        look(x, 5);
        return mv(x, 5);
    }
    if !x.anim_step() {
        return;
    }
    let s = x.u32(Kobold::RESUME_STATE);
    x.set32(f::STATE, s);
    let sub2 = x.u8(Kobold::RESUME_SUB2);
    x.set8(f::SUB2, sub2);
    let flip = x.u8(f::FLIP) ^ 0x20;
    x.set8(f::FLIP, flip);
    mv(x, 0);
}

// --- +6 = 4 : attente -----------------------------------------------------------------------------------------

fn wait<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(Enemy::VULNERABLE, 1);
        x.set8(Kobold::LOOK, 0);
        return mv(x, 0);
    }
    let n = x.u8(Enemy::AI_WAIT).wrapping_sub(1);
    x.set8(Enemy::AI_WAIT, n);
    if (n as i8) < 0 {
        return set_state_long(x, STANCE);
    }
    if must_turn(x) {
        x.set32(Kobold::RESUME_STATE, STANCE);
        x.set8(Kobold::RESUME_SUB2, 4);
        set_state_long(x, 0x0202_0200);
    }
    x.anim_step();
}

// --- +6 = 6 : course ------------------------------------------------------------------------------------------

fn run_up<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        4 => {
            x.add8(f::SUB3, 0xfe);
            run_begin(x);
        }
        0 => {
            x.add8(f::SUB3, 2);
            x.set16(Kobold::TIMER, 0x28);
            lib::draw_stance(x);
            run_begin(x);
        }
        2 => {
            let n = x.u16(Kobold::TIMER).wrapping_sub(1);
            x.set16(Kobold::TIMER, n);
            if (n as i16) < 0 {
                return set_state_long(x, STANCE);
            }
            let n = x.u8(Enemy::AI_STANCE).wrapping_sub(1);
            x.set8(Enemy::AI_STANCE, n);
            if (n as i8) < 0 {
                lib::draw_stance(x);
                pick_target(x);
                x.set8(Enemy::AI_POINT_SET, 0xe);
                lib::approach_target(x);
            }
            if must_turn(x) {
                x.set32(Kobold::RESUME_STATE, 0x0202_0004);
                let sub2 = x.u8(f::SUB2);
                x.set8(Kobold::RESUME_SUB2, sub2);
                set_state_long(x, 0x0202_0200);
            }
            x.anim_step();
            x.move_by_table_xy();
        }
        _ => {}
    }
}

/// 0x5583c: start of the run.
fn run_begin<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::VULNERABLE, 1);
    x.set8(Kobold::LOOK, 0x16);
    mv(x, 0x16);
    set_speed(x, 0x17);
}
