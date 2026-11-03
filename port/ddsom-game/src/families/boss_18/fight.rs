//! Boss 18, group `+5` = 2: the combat. The Lich waits, turns around, walks toward a player and, depending on the
//! distance, chooses an attack (`decide`); or rushes in a straight line.
//!
//! | `+6` | Role |
//! |---|---|
//! | 0 | standing, waits (animation 2) |
//! | 2 | about-face (keeps advancing if it was walking) |
//! | 4, 6 | walk toward a player (4: fast steps, 6: slow) |
//! | 8 | charge |
//! | 10 | recovery gesture (animation 6) |

use crate::families::cx::Bench;
use super::decide;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => idle(x),
        2 => turn(x),
        4 => walk(x, false),
        6 => walk(x, true),
        8 => dash(x),
        10 => resume(x),
        _ => {}
    }
}

fn players_alive<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    lib::players_alive(x)
}

fn needs_turn<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    !lib::facing_target(x)
}

// --- +6 = 0 : debout --------------------------------------------------------------------------

fn idle<R: Bench>(x: &mut Cx<'_, R>) {
    if !players_alive(x) {
        return decide::wait(x);
    }
    if x.u8(Enemy::SUB_STEP) == 0 {
        x.add8(Enemy::SUB_STEP, 2);
        lib::draw_6e(x);
        x.set8(Boss::ANIM_KIND, 0);
        if x.u8(Boss::ANIM_LAST) != 0 {
            x.set8(Boss::ANIM_LAST, 0);
            body(x, 2);
        }
        return;
    }
    if sub8(x, Boss::IDLE_COUNT, 1).le() {
        return decide::chase(x);
    }
    if needs_turn(x) {
        return decide::interrupt(x);
    }
    lib::step(x);
}

// --- +6 = 2 : demi-tour ------------------------------------------------------------------------

/// The boss was walking (saved state `0x0202_04xx` or `06xx`): it advances during the about-face.
fn was_walking<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    x.u16(Boss::SAVED_STATE) == 0x0202 && matches!(x.u8(Boss::SAVED_STATE + 2), 4 | 6)
}

fn turn<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::SUB_STEP) == 0 {
        x.add8(Enemy::SUB_STEP, 2);
        if was_walking(x) {
            x.move_by_table_xy();
        }
        let side = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, side);
        x.set8(Boss::ANIM_KIND, 1);
        x.set8(Boss::ANIM_LAST, 1);
        let n = if x.u8(Boss18::AIM_ON) != 0 { 4 } else { 3 };
        return body(x, n);
    }
    if (x.u16(Boss18::CHASE_TIME) as i16) > 0 {
        sub16(x, Boss18::CHASE_TIME, 1);
    }
    if !lib::step(x) {
        if was_walking(x) {
            x.move_by_table_xy();
        }
        return;
    }
    if !players_alive(x) {
        return decide::wait(x);
    }
    let resume = x.u32(Boss::SAVED_STATE);
    x.set32(f::STATE, resume);
    body(x, 2);
}

// --- +6 = 4 and 6: walk -----------------------------------------------------------------------

fn walk<R: Bench>(x: &mut Cx<'_, R>, slow: bool) {
    if !players_alive(x) {
        return decide::wait(x);
    }
    if x.u8(Enemy::SUB_STEP) != 0 {
        return keep_walking(x);
    }
    if slow {
        decide::walk_steps_slow(x);
        decide::walk_delay_slow(x);
    } else {
        decide::walk_steps_fast(x);
        decide::walk_delay_fast(x);
    }
    x.set8(Boss::ANIM_KIND, if slow { 3 } else { 2 });
    x.add8(Enemy::SUB_STEP, 2);
    x.set16(Enemy::AX, 0);
    x.set16(Enemy::AY, 0);
    if !lib::pick_target_any(x) {
        return decide::wait(x);
    }
    lib::approach_target(x);
    speed(x, 0xd);
    x.move_by_table_xy();
    let kind = x.u8(Boss::ANIM_KIND);
    if kind != x.u8(Boss::ANIM_LAST) {
        x.set8(Boss::ANIM_LAST, kind);
        body(x, 5);
    }
}

/// 0x97a0a: one more walk step; at the end of the steps, the walk stops (`chase_b`); as long as an
/// attack is possible, the boss triggers it.
fn keep_walking<R: Bench>(x: &mut Cx<'_, R>) {
    if !sub8(x, Boss::WALK_DELAY, 1).pl() {
        if sub8(x, Boss::WALK_STEPS, 1).le() {
            return decide::chase_b(x);
        }
        lib::approach_target(x);
        speed(x, 0xd);
        if x.u8(Boss::ANIM_KIND) == 3 {
            decide::walk_delay_slow(x);
        } else {
            decide::walk_delay_fast(x);
        }
    } else {
        let side = decide::probe(x, -0xe0, 0x68, 8);
        if side != 0 {
            return decide::choose_far(x, side);
        }
        if x.u8(Boss::ANIM_KIND) == 3 {
            let side = decide::probe(x, -0x60, 0x20, 6);
            if side != 0 {
                return decide::choose_mid(x, side);
            }
        }
        if lib::target_in_box(x, 0, 0x2c, 0xc) {
            return decide::choose_front(x);
        }
        if lib::guard_in_front(x, -0x20, 0x40, 8) {
            return decide::choose_close(x);
        }
        if needs_turn(x) {
            return decide::interrupt(x);
        }
    }
    x.move_by_table_xy();
    lib::step(x);
}

// --- +6 = 8 : charge ----------------------------------------------------------------------------

fn dash<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Enemy::AY, 0);
            x.set16(Enemy::VZ, 0x300);
            x.set16(Enemy::GRAVITY, (-0x30i16) as u16);
            let (angle, vz) = if x.u8(f::FLIP) == 0 { (8, -0x50i16) } else { (0x18, 0x50) };
            x.set8(Obj::SPEED_INDEX, angle);
            x.set16(Enemy::AX, vz as u16);
            lib::drift_heading(x);
            speed(x, 0x3f);
            x.set8(Boss::ANIM_KIND, 4);
            x.set8(Boss::ANIM_LAST, 4);
            body(x, 5);
        }
        2 => {
            x.move_xyz();
            if !tst16(x, Enemy::VZ).gt() {
                x.add8(Enemy::SUB_STEP, 2);
                x.set16(Enemy::AX, 0);
            }
            lib::step(x);
        }
        4 => {
            x.move_xyz();
            if !tst16(x, Obj::Z).gt() {
                x.add8(Enemy::SUB_STEP, 2);
                x.set16(Boss::TIMER, 0xc);
                let mut v: i16 = -0x1c;
                if x.u8(f::FLIP) != 0 {
                    v = -v;
                }
                x.set16(Enemy::AX, v as u16);
            }
            lib::step(x);
        }
        6 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                x.move_xy();
                lib::step(x);
                return;
            }
            if !players_alive(x) {
                return decide::wait(x);
            }
            decide::chase(x);
        }
        _ => {}
    }
}

// --- +6 = 10 : reprise -------------------------------------------------------------------------------

fn resume<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::SUB_STEP) == 0 {
        x.add8(Enemy::SUB_STEP, 2);
        x.set8(Boss::ANIM_KIND, 5);
        x.set8(Boss::ANIM_LAST, 5);
        return body(x, 6);
    }
    if !lib::step(x) {
        return;
    }
    if !players_alive(x) {
        return decide::wait(x);
    }
    let resume = x.u32(Boss::SAVED_STATE);
    x.set32(f::STATE, resume);
}
