//! Boss 21, group `+5` = 2: the combat. The boss waits, turns around, walks toward a player and, depending on the
//! distance, chooses an attack (`decide`); or rushes in a straight line.
//!
//! | `+6` | Role |
//! |---|---|
//! | 0 | standing, waits (animation 0) |
//! | 2 | about-face |
//! | 4, 6 | walk toward a player (4: fast steps, 6: slow) |
//! | 8, 10 | charge |

use crate::families::cx::Bench;
use super::decide;
use super::*;
use crate::objects::Current;

/// Walk animation sets, chosen by the mark `+0x33` (0x103e0): the first when the boss is not facing the
/// direction it is moving in, the second otherwise.
const WALK_SIDEWAYS: u32 = 0x18388c;
const WALK_FORWARD: u32 = 0x183930;

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
    let set = if facing_forward(x) { WALK_FORWARD } else { WALK_SIDEWAYS };
    lib::switch_keeping_time(x, set);
}

/// The host is dead: exit from the stage; no player left: wait. Returns true if either of the two took place.
fn abort<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if decide::host_dead(x) {
        decide::start_death(x);
        return true;
    }
    if !players_alive(x) {
        decide::wait(x);
        return true;
    }
    false
}

// --- +6 = 0 : debout --------------------------------------------------------------------------

fn idle<R: Bench>(x: &mut Cx<'_, R>) {
    if abort(x) {
        return;
    }
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        lib::draw_6e(x);
        x.set8(ANIM_KIND, 0);
        if x.u8(ANIM_LAST) != 0 {
            x.set8(ANIM_LAST, 0);
            body(x, 0);
        }
        return;
    }
    if sub8(x, APPROACH, 1).le() {
        return decide::chase(x);
    }
    if needs_turn(x) {
        return decide::interrupt(x);
    }
    lib::step(x);
}

// --- +6 = 2 : demi-tour ----------------------------------------------------------------------

fn turn<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        let side = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, side);
        x.set8(ANIM_KIND, 1);
        return body(x, 1);
    }
    if !lib::step(x) {
        return;
    }
    if abort(x) {
        return;
    }
    let resume = x.u32(SAVED_STATE);
    x.set32(f::STATE, resume);
    let last = x.u8(ANIM_LAST);
    if last == 2 || last == 3 {
        x.set8(ANIM_KIND, last);
        walk_anim(x);
    } else {
        body(x, 0);
    }
}

// --- +6 = 4 and 6: walk -----------------------------------------------------------------------

fn walk<R: Bench>(x: &mut Cx<'_, R>, slow: bool) {
    if abort(x) {
        return;
    }
    if x.u8(0x7) != 0 {
        return keep_walking(x);
    }
    if slow {
        decide::walk_steps_slow(x);
        decide::walk_delay_slow(x);
    } else {
        decide::walk_steps_fast(x);
        decide::walk_delay_fast(x);
    }
    x.set8(ANIM_KIND, if slow { 3 } else { 2 });
    x.add8(0x7, 2);
    x.set16(0x16, 0);
    x.set16(0x1e, 0);
    if !pick_target(x) {
        return decide::wait(x);
    }
    lib::approach_target(x);
    speed(x, 0xd);
    let last = x.u8(ANIM_LAST);
    let kind = x.u8(ANIM_KIND);
    x.set8(ANIM_LAST, kind);
    if last == 2 || last == 3 {
        walk_select(x);
    } else {
        walk_anim(x);
    }
}

/// 0x999f4: one more walk step; at the end of the steps, the walk stops (`chase_b`); as long as an
/// attack is possible, the boss triggers it.
fn keep_walking<R: Bench>(x: &mut Cx<'_, R>) {
    if sub8(x, WALK_DELAY, 1).mi() {
        if sub8(x, WALK_STEPS, 1).le() {
            return decide::chase_b(x);
        }
        lib::approach_target(x);
        speed(x, 0xd);
        if x.u8(ANIM_KIND) == 3 {
            decide::walk_delay_slow(x);
        } else {
            decide::walk_delay_fast(x);
        }
        return walk_select(x);
    }
    let side = decide::probe(x, -0xb0, 0x40, 0xc);
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
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(INVULNERABLE, 1);
            x.set16(0x1e, 0);
            x.set16(0x18, 0x500);
            x.set16(0x1a, (-0x30i16) as u16);
            let (angle, vz) = if x.u8(f::FLIP) == 0 { (8, -0x28i16) } else { (0x18, 0x28) };
            x.set8(Obj::SPEED_INDEX, angle);
            x.set16(0x16, vz as u16);
            lib::drift_heading(x);
            speed(x, 0x35);
            x.set8(ANIM_KIND, 5);
            x.set8(ANIM_LAST, 5);
            body(x, 6);
        }
        2 => {
            x.move_xyz();
            if var::FRAME_COUNTER.get(x.b) & 3 == 0 {
                dust_unless_slow(x);
            }
            if cmp16(x, 0x18, 0x100).le() {
                x.add8(0x7, 2);
                x.set8(INVULNERABLE, 0);
                let vz = (x.u16(0x16) as i16) >> 1;
                x.set16(0x16, vz as u16);
                x.set16(0x1a, (-0x1ci16) as u16);
            }
            lib::step(x);
        }
        4 => {
            x.move_xyz();
            if cmp16(x, 0x18, 0xff00).le() {
                x.add8(0x7, 2);
                x.set16(0x16, 0);
                body(x, 8);
            } else {
                lib::step(x);
            }
        }
        6 => {
            x.move_xz();
            if tst16(x, 0xc).gt() {
                lib::step(x);
            } else {
                x.add8(0x7, 2);
                x.set16(0xc, 0);
                body(x, 0xa);
            }
        }
        8 => {
            if lib::step(x) {
                decide::after_attack(x, decide::chase);
            }
        }
        _ => {}
    }
}
