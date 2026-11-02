//! Type 1, leaps and guards. `+5 = 6`: `+6` = 0 rest, 2 side leap, 4 and 6 forward leaps, 8 guard (series 1), 10 screen
//! exit. `+5 = 8`: 0 and 6 pause, 2 slide, 4 side step or depth leap. `+5 = 10`: arrival fall.

use crate::families::cx::Bench;
use super::attacks::{fall, land_wait, ready, rise, restart_slide, settle};
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

// --- substate 6 --------------------------------------------------------------------------------

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if special_hit(x) {
        return;
    }
    match x.u8(f::SUB2) {
        0 => rest(x),
        2 => hop_sideways(x),
        4 => hop_forward(x, false),
        6 => hop_forward(x, true),
        8 => guard_leap(x),
        10 => exit_screen(x),
        _ => {}
    }
    zone_and_terrain(x);
}

/// Rest of 16 frames, then standing.
fn rest<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set16(Skeleton::TIMER, 0x10);
        return body(x, 0);
    }
    let t = x.u16(Skeleton::TIMER).wrapping_sub(1);
    x.set16(Skeleton::TIMER, t);
    if t == 0 {
        set_state_long(x, STANCE);
    }
}

/// Side leap (`+6` = 2): lateral speed at random.
fn hop_sideways<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => ready(x),
        2 => settle(x),
        4 => {
            if x.u8(Enemy::GROUP) != 0 {
                return guard_windup(x);
            }
            x.add8(f::SUB3, 2);
            x.set16(Obj::VY, 0x200);
            if rand_and(x, 1) != 0 {
                negate(x, Obj::VY);
            }
            x.set16(Obj::AY, 0);
            x.set16(Obj::VZ, 0x400);
            x.set16(Obj::AZ, -0x80i16 as u16);
            body(x, 3);
        }
        6 => land_wait(x),
        8 => hop_rise(x),
        10 => hop_fall(x),
        12 => hop_end(x),
        _ => {}
    }
}

/// Forward leaps (`+6` = 4 with movement animation, 6 with attack animation).
fn hop_forward<R: Bench>(x: &mut Cx<'_, R>, striking: bool) {
    match x.u8(f::SUB3) {
        0 => ready(x),
        2 => settle(x),
        4 => {
            if !striking && x.u8(Enemy::GROUP) != 0 {
                x.set8(f::SUB2, 8);
                return guard_windup_random(x);
            }
            x.add8(f::SUB3, 2);
            x.set16(Obj::VX, 0x200);
            if x.u8(f::FLIP) != 0 {
                negate(x, Obj::VX);
            }
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, 0x800);
            x.set16(Obj::AZ, -0x80i16 as u16);
            if striking {
                strike(x, 2)
            } else {
                body(x, 3)
            }
        }
        6 => land_wait(x),
        8 if striking => rise(x),
        8 => {
            step(x);
            x.move_xz();
            if (x.u16(Obj::VZ) as i16) < 0 {
                x.add8(f::SUB3, 2);
                x.set16(Obj::ANIM_TIMER, 1);
            }
        }
        10 if striking => fall(x),
        10 => {
            step(x);
            x.move_xz();
            if (x.u32(Obj::Z) as i32) <= 0 {
                x.add8(f::SUB3, 2);
                x.set32(Obj::Z, 0);
                x.set16(Obj::ANIM_TIMER, 1);
            }
        }
        12 if striking => hop_end_byte(x),
        12 => hop_end(x),
        _ => {}
    }
}

/// Climb of a side leap (movement in depth and in height).
fn hop_rise<R: Bench>(x: &mut Cx<'_, R>) {
    step(x);
    x.move_yz();
    if (x.u16(Obj::VZ) as i16) < 0 {
        x.add8(f::SUB3, 2);
        x.set16(Obj::ANIM_TIMER, 1);
    }
}

fn hop_fall<R: Bench>(x: &mut Cx<'_, R>) {
    step(x);
    x.move_yz();
    if (x.u32(Obj::Z) as i32) <= 0 {
        x.add8(f::SUB3, 2);
        x.set32(Obj::Z, 0);
        x.set16(Obj::ANIM_TIMER, 1);
    }
}

/// End of a leap: once the landing animation is over, back to standing.
fn hop_end<R: Bench>(x: &mut Cx<'_, R>) {
    step(x);
    if (x.u16(Enemy::ANIM_END) as i16) < 0 {
        set_state_long(x, STANCE);
    }
}

/// End of a leap with attack animation: the test is on the high byte of the state.
fn hop_end_byte<R: Bench>(x: &mut Cx<'_, R>) {
    step(x);
    if (x.u8(Enemy::ANIM_END) as i8) < 0 {
        set_state_long(x, STANCE);
    }
}

// --- gardes -------------------------------------------------------------------------------------

/// Guard of attack 18/20 (`+5 = 4`): prepares the counter-attack `+0xa9 = 2`; ends with the choice of a new attack.
pub(super) fn guard<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => ready(x),
        2 => settle(x),
        4 => {
            x.set16(Skeleton::COUNTER, 0x100);
            x.set8(Skeleton::COUNTER_MOVE, 2);
            guard_start(x);
        }
        6 => guard_wait(x),
        8 => {
            if step(x) {
                end_attack(x);
            }
        }
        _ => {}
    }
}

/// Guard of the jump of series 1 (`+5 = 6`, `+6` = 8).
fn guard_leap<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => ready(x),
        2 => settle(x),
        4 => guard_windup_random(x),
        6 => guard_wait(x),
        8 => {
            step(x);
            if (x.u16(Enemy::ANIM_END) as i16) < 0 {
                set_state_long(x, STANCE);
            }
        }
        _ => {}
    }
}

/// 0x58966 / 0x58974: starts the guard from a leap (`+6` = 8, random counter-attack or none).
fn guard_windup<R: Bench>(x: &mut Cx<'_, R>) {
    x.set16(Skeleton::COUNTER, 0);
    x.set8(f::SUB2, 8);
    guard_start(x);
}

/// 0x5897a: counter-attack drawn at random (0 or 2), then the guard.
fn guard_windup_random<R: Bench>(x: &mut Cx<'_, R>) {
    let n = rand_and(x, 1) * 2;
    x.set8(Skeleton::COUNTER_MOVE, n as u8);
    x.set8(Skeleton::COUNTER, 0xff);
    guard_start(x);
}

/// 0x58990: the guard lasts `0x3c` frames, the posture changes to "on guard".
fn guard_start<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    x.set16(Skeleton::TIMER, 0x3c);
    x.set8(Enemy::AI_68, 9);
    body(x, 8);
}

/// 0x589ae: holds the guard, then releases it.
fn guard_wait<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::KEY_FRAME) != 0 {
        let t = x.u16(Skeleton::TIMER).wrapping_sub(1);
        x.set16(Skeleton::TIMER, t);
        if (t as i16) >= 0 {
            return;
        }
        x.add8(f::SUB3, 2);
        x.set16(Obj::ANIM_TIMER, 1);
        let series = x.u8(Enemy::GROUP) as i8 as i16 as u16 as u32;
        let v = x.rom8(0x589da + series);
        x.set8(Enemy::AI_68, v);
    }
    step(x);
}

// --- screen exit -----------------------------------------------------------------------------

/// The skeleton runs away toward the nearest edge (`+6` = 10).
fn exit_screen<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.set16(Obj::VX, -0x230i16 as u16);
        x.set8(f::FLIP, 0);
        let d = var::SCROLL_X.get(x.b).wrapping_sub(x.u16(Obj::X));
        if d as i16 >= 0xc0 {
            negate(x, Obj::VX);
            x.set8(f::FLIP, 0x20);
        }
        x.add8(f::SUB3, 2);
        return body(x, 0);
    }
    if (x.u8(f::KEY_FRAME) as i8) >= 0 {
        x.move_x();
    }
    step(x);
    let d = x.u16(Obj::X).wrapping_sub(var::SCROLL_X.get(x.b)).wrapping_add(0x10);
    if d > 0x1a0 {
        detach_from_host(x);
        set_state_long(x, 0x0600_0000);
    }
}

// --- substate 8: dodges ---------------------------------------------------------------------

pub(super) fn run_dash<R: Bench>(x: &mut Cx<'_, R>) {
    if special_hit(x) {
        return;
    }
    match x.u8(f::SUB2) {
        0 | 6 => pause(x),
        2 => run_aside(x),
        4 => dodge(x),
        _ => {}
    }
    zone_and_terrain(x);
}

/// Pause de 30 images.
fn pause<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set16(Skeleton::TIMER, 0x1e);
        return body(x, 0);
    }
    let t = x.u16(Skeleton::TIMER).wrapping_sub(1);
    x.set16(Skeleton::TIMER, t);
    if t == 0 {
        set_state_long(x, STANCE);
    }
}

/// Sideways run: same animation as the slide, for 0x20 frames.
fn run_aside<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        restart_slide(x);
        x.set16(Skeleton::TIMER, 0x20);
        return;
    }
    step(x);
    if (x.u8(f::KEY_FRAME) as i8) < 0 {
        return;
    }
    let t = x.u16(Skeleton::TIMER).wrapping_sub(1);
    x.set16(Skeleton::TIMER, t);
    if t != 0 {
        move_xy(x);
        let d = x.u16(Skeleton::SAVED_POS).wrapping_sub(x.u16(Obj::X)).wrapping_add(0x30);
        if d < 0x60 {
            return;
        }
    }
    set_state_long(x, STANCE);
}

/// Dodge in depth (`+6` = 4): run forward or leap according to a draw.
fn dodge<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            let mut angle = 0;
            if x.u16(f::TARGET) != 0 {
                let target = load_target(x);
                let depth = x.u16(Obj::Y);
                if depth < target.u16(x.b, Obj::Y) {
                    angle = 0x10;
                }
            }
            x.set8(Obj::SPEED_INDEX, angle);
            if (x.random() as u32) & 1 != 0 {
                x.set8(f::SUB3, 4);
                x.set16(Obj::VY, 0x300);
                if angle != 0 {
                    negate(x, Obj::VY);
                }
                x.set16(Obj::AY, 0);
                x.set16(Obj::VZ, 0x400);
                x.set16(Obj::AZ, -0x80i16 as u16);
                body(x, 3);
            } else {
                x.set16(Skeleton::TIMER, 0x16);
                let y = x.u16(Obj::Y);
                x.set16(Skeleton::SAVED_POS, y);
                x.set32(Obj::SPEED_TABLE, 0x1c74b6);
                body(x, 0);
            }
        }
        2 => {
            step(x);
            if (x.u8(f::KEY_FRAME) as i8) < 0 {
                return;
            }
            let t = x.u16(Skeleton::TIMER).wrapping_sub(1);
            x.set16(Skeleton::TIMER, t);
            if t != 0 {
                move_xy(x);
                let d = x.u16(Skeleton::SAVED_POS).wrapping_sub(x.u16(Obj::Y)).wrapping_add(0x20);
                if d < 0x40 {
                    return;
                }
            }
            set_state_long(x, STANCE);
        }
        4 => land_wait(x),
        6 => hop_rise(x),
        8 => hop_fall(x),
        10 => hop_end(x),
        _ => {}
    }
}

// --- substate 10: arrival fall -------------------------------------------------------------

pub(super) fn run_land<R: Bench>(x: &mut Cx<'_, R>) {
    land(x);
    zone_and_terrain(x);
}

fn land<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set32(Obj::VX, 0);
            x.set32(Obj::VY, 0);
            x.set16(Obj::VZ, 0x400);
            x.set16(Obj::AZ, -0x40i16 as u16);
            body(x, 3);
        }
        2 => {
            step(x);
            x.move_xz();
            if (x.u16(Obj::VZ) as i16) <= 0 {
                x.add8(f::SUB2, 2);
                x.set16(Obj::ANIM_TIMER, 1);
            }
        }
        4 => {
            step(x);
            x.move_xz();
            if (x.u16(Obj::Z) as i16) <= 0 {
                x.set32(Obj::Z, 0);
                x.add8(f::SUB2, 2);
                x.set16(Obj::ANIM_TIMER, 1);
            }
        }
        6 => {
            if step(x) {
                x.set16(f::SUB2, 0);
                if var::PLAYERS_ALIVE.get(x.b) != 0 {
                    set_state_long(x, STANCE);
                    pick_target(x);
                }
            }
        }
        _ => {}
    }
}
