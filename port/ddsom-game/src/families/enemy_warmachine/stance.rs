//! Type 27, substate 2: the machine rolls toward the player. `+6` = 0 wait, 2 turn-around, 4 and 6 walk (with or
//! without charge delay), 8 charge.

use crate::families::cx::Bench;
use super::arrival::hop_loop;
use super::decide::*;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

/// Delay tables (bytes) read by `charge_delay` and `wander_delay`.
const CHARGE_A: u32 = 0x78e8c;
const CHARGE_B: u32 = 0x78eac;
const WANDER_A: u32 = 0x78ef0;
const WANDER_B: u32 = 0x78f10;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => wait(x),
        2 => turn(x),
        4 => march(x, true),
        6 => march(x, false),
        8 => charge(x),
        _ => {}
    }
}

/// Common start of moves 0 to 6: grabbed or no player left → fallback states (true if we have left).
fn gate<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if grabbed(x) {
        go::held(x);
        return true;
    }
    if !lib::players_alive(x) {
        go::idle(x);
        return true;
    }
    false
}

/// 0x780d4 : attente.
fn wait<R: Bench>(x: &mut Cx<'_, R>) {
    if gate(x) {
        return;
    }
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        pick_wait(x);
        x.set8(LOOK, 0);
        if x.u8(LOOK_PREV) == 0 {
            return;
        }
        x.set8(LOOK_PREV, 0);
        return mv(x, 4);
    }
    if lib::outside_band(x, -0x30, 0x1b0) {
        return go::charge(x, 0);
    }
    let n = x.u8(AI_WAIT).wrapping_sub(1);
    x.set8(AI_WAIT, n);
    if (n as i8) <= 0 {
        return go::fire_a(x);
    }
    if !lib::facing_target(x) {
        return go::turn(x);
    }
    lib::step(x);
}

/// 0x79f6: sets the wait delay `+0x6e` from the difficulty and returns control (D0 = 1).
fn pick_wait<R: Bench>(x: &mut Cx<'_, R>) {
    if !x.in_arena(0) {
        x.set8(AI_WAIT, 1);
        return;
    }
    let group = x.u8(0x6d) as u32;
    let list = x.data32(0x7a50 + group * 4);
    let r = rand_and(x, 0x1f);
    let v = x.data8(list + r);
    x.set8(AI_WAIT, v);
}

/// 0x78142 : demi-tour.
fn turn<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        x.set8(LOOK, 1);
        return mv(x, 5);
    }
    if !lib::step(x) {
        return;
    }
    if gate(x) {
        return;
    }
    let state = x.u32(RESUME_STATE);
    x.set32(f::STATE, state);
    let prev = x.u8(LOOK_PREV);
    if prev == 2 || prev == 3 {
        x.set8(LOOK, prev);
        return walk_anim(x);
    }
    mv(x, 4);
}

/// 0x78244: walking animation (6 if the speed follows the orientation, 7 otherwise).
fn walk_anim<R: Bench>(x: &mut Cx<'_, R>) {
    let n = if lib::moving_forward(x) { 6 } else { 7 };
    mv(x, n);
}

/// 0x7831e: switches to the flight animation with the same key, according to the direction of the walk.
fn fly_anim<R: Bench>(x: &mut Cx<'_, R>) {
    let set = if lib::moving_forward(x) { SET_FLY_B } else { SET_FLY_A };
    lib::switch_keeping_time(x, set);
}

/// +6 = 4 (`first`) and 6: walks toward the player; each variant draws its delay and its look (2 or 3).
fn march<R: Bench>(x: &mut Cx<'_, R>, first: bool) {
    if gate(x) {
        return;
    }
    if x.u8(f::SUB3) != 0 {
        return march_step(x);
    }
    if first {
        wander_delay(x, WANDER_A);
        charge_delay(x, CHARGE_A);
        x.set8(LOOK, 2);
    } else {
        wander_delay(x, WANDER_B);
        charge_delay(x, CHARGE_B);
        x.set8(LOOK, 3);
    }
    x.add8(f::SUB3, 2);
    x.set16(Obj::AX, 0);
    x.set16(Obj::AY, 0);
    if !pick_target(x) {
        return go::idle(x);
    }
    lib::approach_target(x);
    set_speed(x, 8);
    if matches!(x.u8(LOOK_PREV), 2 | 3) {
        let look = x.u8(LOOK);
        x.set8(LOOK_PREV, look);
        return fly_anim(x);
    }
    let look = x.u8(LOOK);
    x.set8(LOOK_PREV, look);
    walk_anim(x);
}

/// 0x7827c: walk: charge delay, then shot decisions.
fn march_step<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.u8(AI_CHARGE).wrapping_sub(1);
    x.set8(AI_CHARGE, n);
    if (n as i8) < 0 {
        let m = x.u8(AI_WANDER).wrapping_sub(1);
        x.set8(AI_WANDER, m);
        if (m as i8) <= 0 {
            return go::fire_b(x);
        }
        lib::approach_target(x);
        set_speed(x, 8);
        let table = if x.u8(LOOK) == 3 { CHARGE_B } else { CHARGE_A };
        charge_delay(x, table);
        return fly_anim(x);
    }
    if lib::outside_band(x, -0x30, 0x1b0) {
        return go::charge(x, 0);
    }
    if x.u8(LOOK) == 3 {
        let g = gate_near(x);
        if g != 0 {
            return near_decision(x, g);
        }
    }
    let g = gate_wide(x);
    if g != 0 {
        return wide_decision(x, g);
    }
    if lib::target_in_box(x, -0x10, 0x10, 8) {
        return pick_by_phase(x);
    }
    if lib::guard_in_front(x, -0x30, 0x18, 8) {
        return point_blank(x);
    }
    if !lib::facing_target(x) {
        return go::turn(x);
    }
    if (x.u8(f::KEY_FRAME) as i8) >= 0 {
        x.move_by_table_xy();
    }
    lib::step(x);
}

/// +6 = 8: charge toward the player (or toward `+0xbc` if `+0xb2`), then leap.
fn charge<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set16(Obj::AX, 0);
            let mut angle = 0x18;
            if x.u8(AIMED) == 0 {
                let cam = camera_x(x);
                x.set8(f::FLIP, 0);
                if (x.u16(Obj::X) as i16) <= cam as i16 {
                    x.set8(f::FLIP, 0x20);
                    angle = 8;
                }
            } else {
                let target = load_target(x);
                let goal = target.u16(x.b, Obj::X);
                x.set16(GOAL_X, goal);
                if x.u8(f::FLIP) != 0 {
                    angle = 8;
                }
            }
            x.set8(Obj::SPEED_INDEX, angle);
            set_speed(x, 0x21);
            spark(x, 1);
            lib::sound(x, crate::sound::CMD_376_476);
            look(x, 4);
            mv(x, 8);
        }
        2 => {
            x.anim_step();
            x.move_x();
            if x.u8(f::KEY_FRAME) == 2 {
                x.set8(f::KEY_FRAME, 0);
                spark(x, 1);
                lib::shake_54(x, 0x10);
                lib::sound(x, crate::sound::CMD_376_476);
            }
            let (mut d0, mut d1);
            if x.u8(AIMED) == 0 {
                let cam = camera_x(x);
                let reach = if (x.u16(Obj::VX) as i16) < 0 { 0xe0 } else { 0xa0 };
                d0 = cam.wrapping_add(reach);
                d1 = x.u16(Obj::X);
            } else {
                d0 = x.u16(GOAL_X);
                d1 = x.u16(Obj::X);
            }
            if (x.u16(Obj::VX) as i16) < 0 {
                std::mem::swap(&mut d0, &mut d1);
            }
            if (d1 as i16) < (d0 as i16) || x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            super::arrival::leap_begin(x);
        }
        4 => hop_loop(x),
        6 => {
            if lib::step(x) {
                if gate(x) {
                    return;
                }
                go::fire_a(x);
            }
        }
        _ => {}
    }
}
