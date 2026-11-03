//! Boss 17, group `+5` = 2: the combat. The boss waits, turns around, walks toward a player and, depending on the
//! distance, chooses an attack (`decide`); or rushes in a straight line.
//!
//! | `+6` | Role |
//! |---|---|
//! | 0 | standing, waits (animation 0) |
//! | 2 | about-face |
//! | 4, 6 | walk toward a player (4: fast steps, 6: slow) |
//! | 8, 10 | charge |

use crate::families::cx::Bench;
use crate::data::RomAddr;
use super::decide;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

/// Walk animation sets, chosen by the mark `+0x33` (0x103e0): the first when the boss is not facing the
/// direction it is moving in, the second otherwise.
const WALK_SIDEWAYS: RomAddr = RomAddr(0x180a46);
const WALK_FORWARD: RomAddr = RomAddr(0x180aea);

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => idle(x),
        2 => turn(x),
        4 => walk(x, false),
        6 => walk(x, true),
        8 | 10 => dash(x),
        _ => {}
    }
}

fn players_alive<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    var::PLAYERS_ALIVE.get(x.b) != 0
}

/// Must the boss turn around toward its target (0x1050c, Z set = no)?
fn needs_turn<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    !lib::facing_target(x)
}

/// Is the boss facing the direction it is moving in (0x10560, Z set = yes)?
fn facing_forward<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    lib::moving_forward(x)
}

/// Walk animation according to the direction (2 or 3).
fn walk_anim<R: Bench>(x: &mut Cx<'_, R>) {
    let n = if facing_forward(x) { 2 } else { 3 };
    body(x, n);
}

/// Walk animation chosen by the mark `+0x33` in the walk direction's set.
fn walk_select<R: Bench>(x: &mut Cx<'_, R>) {
    let set = if facing_forward(x) { WALK_FORWARD.0 } else { WALK_SIDEWAYS.0 };
    lib::switch_keeping_time(x, set);
}

// --- +6 = 0: standing -------------------------------------------------------------------------

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
            body(x, 0);
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

// --- +6 = 2: about-face ----------------------------------------------------------------------

fn turn<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::SUB_STEP) == 0 {
        x.add8(Enemy::SUB_STEP, 2);
        let side = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, side);
        x.set8(Boss::ANIM_KIND, 1);
        return body(x, 1);
    }
    if !lib::step(x) {
        return;
    }
    if !players_alive(x) {
        return decide::wait(x);
    }
    let resume = x.u32(Boss::SAVED_STATE);
    x.set32(f::STATE, resume);
    let last = x.u8(Boss::ANIM_LAST);
    if last == 2 || last == 3 {
        x.set8(Boss::ANIM_KIND, last);
        walk_anim(x);
    } else {
        body(x, 0);
    }
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
    if !pick_target(x) {
        return decide::wait(x);
    }
    lib::approach_target(x);
    speed(x, 0xd);
    let last = x.u8(Boss::ANIM_LAST);
    let kind = x.u8(Boss::ANIM_KIND);
    x.set8(Boss::ANIM_LAST, kind);
    if last == 2 || last == 3 {
        walk_select(x);
    } else {
        walk_anim(x);
    }
}

/// 0x95ff8: one more walk step; at the end of the steps, the walk stops (`chase_b`); as long as an attack is
/// possible, the boss triggers it.
fn keep_walking<R: Bench>(x: &mut Cx<'_, R>) {
    if sub8(x, Boss::WALK_DELAY, 1).mi() {
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
        return walk_select(x);
    }
    let side = decide::probe(x, -0xc0, 0x40, 0xc);
    if side != 0 {
        return decide::choose_far(x, side);
    }
    if decide::probe(x, -0x40, 0x30, 6) != 0 {
        return decide::choose_mid(x);
    }
    if lib::target_in_box(x, -0x10, 0x10, 8) {
        return decide::choose_front(x);
    }
    if lib::guard_in_front(x, -0x40, 0x20, 8) {
        return decide::choose_close(x);
    }
    if needs_turn(x) {
        return decide::interrupt(x);
    }
    x.move_by_table_xy();
    lib::step(x);
}

// --- +6 = 8 and 10: charge ----------------------------------------------------------------------

fn dash<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Boss::INVULNERABLE, 1);
            x.set16(Enemy::AY, 0);
            x.set16(Enemy::VZ, 0x500);
            x.set16(Enemy::GRAVITY, (-0x30i16) as u16);
            let (angle, vz) = if x.u8(f::FLIP) == 0 { (8, -0x28i16) } else { (0x18, 0x28) };
            x.set8(Obj::SPEED_INDEX, angle);
            x.set16(Enemy::AX, vz as u16);
            lib::drift_heading(x);
            speed(x, 0x35);
            x.set8(Boss::ANIM_KIND, 5);
            x.set8(Boss::ANIM_LAST, 5);
            body(x, 6);
        }
        2 => {
            x.move_xyz();
            if var::FRAME_COUNTER.get(x.b) & 3 == 0 {
                attacks::dust_unless_slow(x);
            }
            if cmp16(x, Enemy::VZ, 0x100).le() {
                x.add8(Enemy::SUB_STEP, 2);
                x.set8(Boss::INVULNERABLE, 0);
                let vz = (x.u16(Enemy::AX) as i16) >> 1;
                x.set16(Enemy::AX, vz as u16);
                x.set16(Enemy::GRAVITY, (-0x1ci16) as u16);
            }
            lib::step(x);
        }
        4 => {
            x.move_xyz();
            if cmp16(x, Enemy::VZ, 0xff00).le() {
                x.add8(Enemy::SUB_STEP, 2);
                x.set16(Enemy::AX, 0);
                body(x, 8);
            } else {
                lib::step(x);
            }
        }
        6 => {
            x.move_xz();
            if tst16(x, Obj::Z).gt() {
                lib::step(x);
            } else {
                x.add8(Enemy::SUB_STEP, 2);
                x.set16(Obj::Z, 0);
                lib::sound(x, crate::sound::CMD_362);
                body(x, 0xa);
            }
        }
        8 => {
            if lib::step(x) {
                if !players_alive(x) {
                    return decide::wait(x);
                }
                decide::chase(x);
            }
        }
        _ => {}
    }
}
