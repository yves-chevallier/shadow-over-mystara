//! Type 8, substate 2: movements. `+6` = 0 slow pursuit, 2 turn-around, 4 and 6 pursuit flight (two paces), 8 dive
//! toward the target, 10 jump with landing.

use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => pursue(x),
        2 => turn(x),
        4 => glide(x, false),
        6 => glide(x, true),
        8 => plunge(x),
        10 => leap(x),
        _ => {}
    }
}

/// Stops the move if a player grabs the enemy or if nobody is alive any more; true if the state has changed.
fn interrupted<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if grabbed(x) {
        go::held(x);
        return true;
    }
    if !lib::players_alive(x) {
        go::retire(x);
        return true;
    }
    false
}

/// Forward (3) or backward (4) flight animation according to the direction of the speed (0x644f4).
fn fly_forward_or_back<R: Bench>(x: &mut Cx<'_, R>) {
    let n = if lib::moving_forward(x) { 3 } else { 4 };
    fly(x, n);
}

/// The flight animation set matching the direction of the speed, on the same frame (0x645b8).
fn switch_flight_set<R: Bench>(x: &mut Cx<'_, R>) {
    let table = if lib::moving_forward(x) { SET_FLY_ALONG.0 } else { SET_FLY_AGAINST.0 };
    lib::switch_keeping_time(x, table);
}

/// 0x6563c / 0x65648: pursuit pace read according to difficulty.
fn charge_a<R: Bench>(x: &mut Cx<'_, R>) {
    let level = var::RANK.get(x.b) as u32;
    let v = x.rom8(0x65654 + level);
    x.set8(Enemy::AI_STANCE, v);
}

fn charge_b<R: Bench>(x: &mut Cx<'_, R>) {
    let level = var::RANK.get(x.b) as u32;
    let v = x.rom8(0x65674 + level);
    x.set8(Enemy::AI_STANCE, v);
}

/// 0x65694 / 0x656a6: number of passes before changing gesture.
fn wander_a<R: Bench>(x: &mut Cx<'_, R>) {
    let r = rand_and(x, 0x1f);
    let v = x.rom8(0x656b8 + r);
    x.set8(Enemy::AI_WANDER, v);
}

fn wander_b<R: Bench>(x: &mut Cx<'_, R>) {
    let r = rand_and(x, 0x1f);
    let v = x.rom8(0x656d8 + r);
    x.set8(Enemy::AI_WANDER, v);
}

// --- +6 = 0 : poursuite lente ---------------------------------------------------------------------

fn pursue<R: Bench>(x: &mut Cx<'_, R>) {
    if interrupted(x) {
        return;
    }
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        lib::draw_6e(x);
        x.set8(Gargoyle::LOOK, 0);
        if x.u8(Gargoyle::LOOK_PREV) != 0 {
            x.set8(Gargoyle::LOOK_PREV, 0);
            fly(x, 0);
        }
        return;
    }
    if lib::outside_band(x, -0x40, 0x1c0) {
        return go::dive(x);
    }
    let n = x.u8(Enemy::AI_WAIT).wrapping_sub(1);
    x.set8(Enemy::AI_WAIT, n);
    if (n as i8) <= 0 {
        return go::wander(x);
    }
    if !lib::facing_target(x) {
        return go::turn_around(x);
    }
    lib::step(x);
}

// --- +6 = 2 : demi-tour ---------------------------------------------------------------------------

fn turn<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        x.set8(Gargoyle::LOOK, 1);
        return fly(x, 2);
    }
    if !lib::step(x) {
        return;
    }
    if interrupted(x) {
        return;
    }
    let saved = x.u32(Gargoyle::SAVED_STATE);
    x.set32(f::STATE, saved);
    let prev = x.u8(Gargoyle::LOOK_PREV);
    if prev == 2 || prev == 3 {
        x.set8(Gargoyle::LOOK, prev);
        return fly_forward_or_back(x);
    }
    fly(x, 0);
}

// --- +6 = 4 and 6: pursuit flight -----------------------------------------------------------------

/// `second`: variant 6 (`+0xa0 = 3`, pace B), otherwise variant 4 (`+0xa0 = 2`, pace A).
fn glide<R: Bench>(x: &mut Cx<'_, R>, second: bool) {
    if interrupted(x) {
        return;
    }
    if x.u8(f::SUB3) == 0 {
        if second {
            wander_b(x);
            charge_b(x);
            x.set8(Gargoyle::LOOK, 3);
        } else {
            wander_a(x);
            charge_a(x);
            x.set8(Gargoyle::LOOK, 2);
        }
        return begin_glide(x);
    }
    glide_step(x);
}

/// 0x644b8: start of the flight.
fn begin_glide<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    x.set16(Obj::AX, 0);
    x.set16(Obj::AY, 0);
    if !pick_target(x) {
        return go::retire(x);
    }
    lib::approach_target(x);
    set_speed(x, 0xd);
    let prev = x.u8(Gargoyle::LOOK_PREV);
    let look = x.u8(Gargoyle::LOOK);
    x.set8(Gargoyle::LOOK_PREV, look);
    if prev == 2 || prev == 3 {
        switch_flight_set(x);
    } else {
        fly_forward_or_back(x);
    }
}

/// 0x6452c: one flight step; changes move when the counters are exhausted or a player is in range.
fn glide_step<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.u8(Enemy::AI_STANCE).wrapping_sub(1);
    x.set8(Enemy::AI_STANCE, n);
    if (n as i8) < 0 {
        let w = x.u8(Enemy::AI_WANDER).wrapping_sub(1);
        x.set8(Enemy::AI_WANDER, w);
        if (w as i8) <= 0 {
            return go::wander_far(x);
        }
        lib::approach_target(x);
        set_speed(x, 0xd);
        if x.u8(Gargoyle::LOOK) == 3 {
            charge_b(x);
        } else {
            charge_a(x);
        }
        return switch_flight_set(x);
    }
    if lib::outside_band(x, -0x40, 0x1c0) {
        return go::dive(x);
    }
    let near = near_box(x, -0x90, 0x10, 6);
    if near != 0 {
        return pick_b(x, near);
    }
    if x.u8(Gargoyle::LOOK) == 3 {
        let close = near_box(x, -0x3a, 0x12, 3);
        if close != 0 {
            return pick_a2(x, close);
        }
    }
    if lib::target_in_box(x, -0x10, 0x14, 8) {
        return pick_a(x);
    }
    if !lib::facing_target(x) {
        return go::turn_around(x);
    }
    if (x.u8(f::KEY_FRAME) as i8) >= 0 {
        x.move_by_table_xy();
    }
    lib::step(x);
}

/// 0x656f8 / 0x65716: a player in the box, then the AI decision (0, 1 or 2).
fn near_box<R: Bench>(x: &mut Cx<'_, R>, d0: i16, d1: u16, d2: u16) -> u32 {
    if !lib::target_in_box(x, d0, d1, d2) {
        return 0;
    }
    lib::turn_decision(x) as u32
}

/// 0x6575a: chooses the next attack move (table by HP threshold `+0xaa`).
fn pick_a<R: Bench>(x: &mut Cx<'_, R>) {
    let r = rand_and(x, 0xf);
    let table = [0x65792, 0x657a2, 0x657b2][x.u8(Gargoyle::PHASE).min(2) as usize];
    let v = x.rom8(table + r);
    match v {
        0 => go::strike_0(x, false),
        2 => go::strike_0(x, true),
        4 => go::strike_8(x),
        _ => {}
    }
}

/// 0x657c8: like `pick_a`, but according to the `gate` result of the AI decision (1: in front, 2: behind).
fn pick_a2<R: Bench>(x: &mut Cx<'_, R>, gate: u32) {
    *x.c.d2() = *x.c.d2() & !0xffff | gate;
    let r = rand_and(x, 0xf);
    let table = if gate == 1 { 0x65822 } else { 0x65832 };
    let v = x.rom8(table + r);
    match v {
        0 => go::strike_2(x, false),
        2 => go::strike_2(x, true),
        4 => go::strike_4(x, false),
        _ => {}
    }
}

/// 0x65848: draws a fallback move (table by threshold `+0xaa`).
fn pick_b<R: Bench>(x: &mut Cx<'_, R>, gate: u32) {
    *x.c.d2() = *x.c.d2() & !0xffff | gate;
    let r = rand_and(x, 0xf);
    let table = 0x658a2 + 0x10 * x.u8(Gargoyle::PHASE).min(2) as u32;
    let v = x.rom8(table + r);
    match v {
        0 => go::strike_10(x),
        2 => go::strike_6(x),
        4 => go::strike_12(x),
        _ => {}
    }
}

// --- +6 = 8 : plongeon -------------------------------------------------------------------------------

fn plunge<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set16(Obj::AX, 0);
            let mut angle = 0x18;
            if x.u8(Gargoyle::SIDE_TURN) != 0 {
                let target = load_target(x);
                let px = target.u16(x.b, Obj::X);
                x.set16(Gargoyle::GOAL_X, px);
                if x.u8(f::FLIP) != 0 {
                    angle = 8;
                }
            } else {
                x.set8(f::FLIP, 0);
                let cam = camera_x(x);
                if (x.u16(Obj::X) as i16) <= (cam as i16) {
                    x.set8(f::FLIP, 0x20);
                    angle = 8;
                }
            }
            x.set8(Obj::SPEED_INDEX, angle);
            set_speed(x, 0x2b);
            look(x, 4);
            fly(x, 1);
        }
        2 => {
            if (x.u16(Obj::Z) as i16) >= 0x14 && x.u8(f::KEY_FRAME) == 4 {
                x.add8(f::SUB3, 2);
                x.set16(Gargoyle::TIMER, 2);
            } else {
                rise(x);
            }
            lib::step(x);
        }
        4 => {
            lib::step(x);
            bob(x);
            if count_down(x, Gargoyle::TIMER) {
                return;
            }
            x.add8(f::SUB3, 2);
            lib::sound(x, crate::sound::CMD_373_473);
            fly(x, 5);
        }
        6 => {
            x.move_x();
            let (mut d0, mut d1);
            let leftward = (x.u16(Obj::VX) as i16) < 0;
            if x.u8(Gargoyle::SIDE_TURN) == 0 {
                let cam = camera_x(x);
                d0 = cam.wrapping_add(if leftward { 0xf0 } else { 0x90 });
                d1 = x.u16(Obj::X);
            } else {
                d0 = x.u16(Gargoyle::GOAL_X);
                d1 = x.u16(Obj::X);
            }
            if leftward {
                std::mem::swap(&mut d0, &mut d1);
            }
            if (d1 as i16) < (d0 as i16) {
                return;
            }
            x.add8(f::SUB3, 2);
            let vy = if x.u8(f::FLIP) != 0 { -0x10i16 } else { 0x10 };
            x.set16(Obj::AX, vy as u16);
            x.set16(Obj::VZ, 0);
            x.set16(Obj::AZ, -0x40i16 as u16);
        }
        8 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set16(Gargoyle::TIMER, 0x14);
            x.set16(Gargoyle::TIMER2, 0);
            x.set16(Obj::Z, 0);
            let vy = if x.u8(f::FLIP) != 0 { -0x10i16 } else { 0x10 };
            x.set16(Obj::AX, vy as u16);
            fly(x, 6);
        }
        10 => {
            if !count_down(x, Gargoyle::TIMER2) {
                x.set16(Gargoyle::TIMER2, 4);
                spark(x, 8);
            }
            x.move_x();
            if count_down(x, Gargoyle::TIMER) {
                lib::step(x);
            } else if !interrupted(x) {
                go::wander(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 10 : saut ----------------------------------------------------------------------------------

fn leap<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            look(x, 5);
            fly(x, 9);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            let (angle, vy) = if x.u8(f::FLIP) != 0 { (0x18, 0x40i16) } else { (8, -0x40i16) };
            x.set8(Obj::SPEED_INDEX, angle);
            x.set16(Obj::AX, vy as u16);
            lib::drift_heading(x);
            set_speed(x, 0x3f);
            x.set16(Obj::VZ, 0x380);
            x.set16(Obj::AZ, -0x28i16 as u16);
            x.move_xz();
        }
        4 => {
            x.move_xz();
            if (x.u16(Obj::VZ) as i16) > 0xc0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set8(Gargoyle::INVULNERABLE, 0);
            x.set16(Obj::AX, 0);
        }
        6 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0 {
                lib::step(x);
                return;
            }
            x.add8(f::SUB3, 2);
            x.set16(Gargoyle::TIMER, 0x12);
            x.set16(Gargoyle::TIMER2, 0);
            x.set16(Obj::Z, 0);
            let vy = if x.u8(f::FLIP) != 0 { 0x20i16 } else { -0x20 };
            x.set16(Obj::AX, vy as u16);
            x.move_x();
            fly(x, 0xa);
        }
        8 => {
            if !count_down(x, Gargoyle::TIMER2) {
                x.set16(Gargoyle::TIMER2, 4);
                spark(x, 8);
            }
            if count_down(x, Gargoyle::TIMER) {
                x.move_x();
                lib::step(x);
            } else if !interrupted(x) {
                go::wander(x);
            }
        }
        _ => {}
    }
}
