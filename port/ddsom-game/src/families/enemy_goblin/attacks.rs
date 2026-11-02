//! Type 15, substates 4, 6 and 8: the moves. Substate 4 contains the series of strikes, the charge and rest, 6 the side
//! steps, jumps and teleportations, 8 the choice that follows a series.

use crate::families::cx::Bench;
use super::stance::{approach_start, walk_off};
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match (x.u8(f::SUB), x.u8(f::SUB2)) {
        (4, 0) | (6, 0) => windup(x),
        (4, 2) | (6, 2) => turn_back(x),
        (4, 4) => series(x),
        (4, 6) | (4, 8) => side_hop(x),
        (4, 10) => maybe_series(x),
        (4, 12) => charge(x),
        (4, 14) => again(x, 0x26),
        (4, 16) => again(x, 0x1c),
        (4, 18) => rest(x),
        (4, 20) => walk_off(x),
        (6, 4) => done_attack(x),
        (6, 6) => depth_step(x),
        (6, 8) => dodge(x),
        (6, 10) => dodge_then_series(x),
        (6, 12) => teleport(x),
        (8, 0) => pause(x),
        (8, 2) => back_step(x),
        (8, 4) => lunge(x),
        (8, 6) => pick_again(x),
        _ => {}
    }
}

/// 0x6e228: the next step starts animation 4.
fn next_and_start<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    mv(x, 4);
}

/// 0x6e230: next step at the end of the animation.
fn next_when_done<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        x.add8(f::SUB3, 2);
    }
}

/// Common step "crouches" (0x6e27e, 0x6e32a, 0x6e502): end of animation, invulnerable dodge for `ticks` frames.
fn crouch<R: Bench>(x: &mut Cx<'_, R>, ticks: u8) {
    if !x.anim_step() {
        return;
    }
    x.add8(f::SUB3, 2);
    x.set8(0x3b, 0);
    x.set_bit(1, 1, true);
    x.set8(TIMER, ticks);
}

/// Common step "moves by `dist`" (0x6e100, 0x6e34a, 0x6e522): after the crouch, jumps sideways.
fn shift<R: Bench>(x: &mut Cx<'_, R>, dist: i16) {
    let n = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, n);
    if n != 0 {
        return;
    }
    x.add8(f::SUB3, 2);
    let d = if x.u8(f::FLIP) != 0 { -dist } else { dist };
    let at = x.u16(Obj::X).wrapping_add(d as u16);
    x.set16(Obj::X, at);
    x.set_bit(1, 1, false);
    mv(x, 3);
}

/// Common step "fires a strike" (0x6e086, 0x6e3a4): in progress, the animation key triggers the effect.
fn swing_key<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.anim_step() {
        return true;
    }
    if x.u8(f::KEY_FRAME) != 0 {
        x.set8(f::KEY_FRAME, 0);
        crate::families::eshl_creators::shot_0(&mut crate::families::effects_creators::leftovers::Pure, x.b, x.o);
    }
    false
}

// --- move 0: taunt, then chosen move --------------------------------------------------------

fn windup<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        lib::pick_byte(x);
        if x.u8(AI_LUNGE) == 0 {
            x.set8(f::SUB3, 0);
            let chosen = x.u8(CHOSEN);
            x.set8(f::SUB2, chosen);
        }
        return mv(x, 0);
    }
    x.anim_step();
    let n = x.u8(AI_LUNGE).wrapping_sub(1);
    x.set8(AI_LUNGE, n);
    if (n as i8) >= 0 {
        return;
    }
    x.set8(f::SUB3, 0);
    if faces_wrong(x) {
        x.add8(f::SUB2, 2);
    } else {
        let chosen = x.u8(CHOSEN);
        x.set8(f::SUB2, chosen);
    }
}

/// +6 = 2: turn-around then move 0xc (walk).
fn turn_back<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        return mv(x, 2);
    }
    if x.anim_step() {
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        x.set16(f::SUB2, 0xc00);
        mv(x, 0);
    }
}

// --- series of strikes ----------------------------------------------------------------------------------

/// 0x6e048: starts the series (+6 = 4).
fn series_begin<R: Bench>(x: &mut Cx<'_, R>) {
    x.set16(f::SUB2, 0x400);
    series(x);
}

/// +6 = 4: series of strikes of the duration drawn from the difficulty row, then fallback move.
fn series<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        let t = row_byte(x, SERIES_TABLE);
        x.set8(TIMER, t);
        return strike(x, 0);
    }
    if !swing_key(x) {
        return;
    }
    let n = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, n);
    if n != 0 {
        return strike(x, 0);
    }
    follow_up(x);
}

/// +6 = 10: at random, back to walking (0x1c) or series.
fn maybe_series<R: Bench>(x: &mut Cx<'_, R>) {
    if rand_and(x, 1) == 0 {
        return again(x, 0x1c);
    }
    series_begin(x);
}

/// +6 = 14, 16 (and 10 on a draw): back to standing and resume walking, `kind` = saved shot type.
fn again<R: Bench>(x: &mut Cx<'_, R>, kind: u8) {
    x.set8(SHOT_KIND, kind);
    set_state_long(x, STANCE);
    approach_start(x);
}

/// +6 = 6 and 8: invulnerable side step, then series (6) or back to walking (8).
fn side_hop<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 | 8 => next_when_done(x),
        4 => {
            x.add8(f::SUB3, 2);
            x.set_bit(1, 1, true);
            x.set8(0x3b, 0);
            x.set8(TIMER, 0x14);
        }
        6 => shift(x, 0x40),
        10 => {
            if x.u8(f::SUB2) != 8 {
                return series_begin(x);
            }
            set_state_long(x, STANCE);
            approach_start(x);
        }
        _ => {}
    }
}

/// +6 = 12: charge toward the target for `+0xa2` frames.
fn charge<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(TIMER, 0x40);
        let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
        x.set8(Obj::SPEED_INDEX, angle);
        set_speed(x, 0x12);
        return mv(x, 1);
    }
    x.move_by_table_xy();
    x.anim_step();
    let n = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, n);
    if (n as i8) < 0 {
        x.set16(f::SUB2, 0x400);
    }
}

/// +6 = 18: rest. The original leaves the rest from the second frame on (the timer-end test is inverted).
fn rest<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(TIMER, 0x1e);
        return mv(x, 0);
    }
    x.anim_step();
    let n = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, n);
    if n != 0 {
        follow_up(x);
    }
}

// --- substate 6 --------------------------------------------------------------------------------------

/// +6 = 6: step in depth.
fn depth_step<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 => next_when_done(x),
        4 => {
            let r = rand_and(x, 1);
            let d: i16 = if r != 0 { -0x18 } else { 0x18 };
            let at = x.u16(Obj::Y).wrapping_add(d as u16);
            x.set16(Obj::Y, at);
            x.add8(f::SUB3, 2);
            mv(x, 3);
        }
        6 => {
            if x.anim_step() {
                done_attack(x);
            }
        }
        _ => {}
    }
}

/// +6 = 8: dodge in place.
fn dodge<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 => crouch(x, 0xa),
        4 => {
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if n != 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set_bit(1, 1, false);
            mv(x, 3);
        }
        6 => {
            if x.anim_step() {
                done_attack(x);
            }
        }
        _ => {}
    }
}

/// +6 = 10: dodge with a side jump, then series of strikes ending with a return to standing.
fn dodge_then_series<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 => crouch(x, 0xa),
        4 => shift(x, 0x40),
        6 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
                let t = row_byte(x, SERIES_TABLE);
                x.set8(TIMER, t);
                strike(x, 0);
            }
        }
        8 => {
            if !swing_key(x) {
                return;
            }
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if n != 0 {
                return strike(x, 0);
            }
            done_attack(x);
        }
        _ => {}
    }
}

/// Arrival positions (X then depth) of the teleportation, relative to the target.
const TELEPORT_X: u32 = 0x6e44c;
const TELEPORT_Y: u32 = 0x6e43e;

/// +6 = 12: the Goblin reappears near its target.
fn teleport<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 | 6 => next_when_done(x),
        4 => {
            x.add8(f::SUB3, 2);
            let r = rand_and(x, 3) * 2;
            let target = load_target(x);
            let off = x.rom16(TELEPORT_X + r);
            let at = off.wrapping_add(target.u16(x.b, Obj::X));
            x.set16(Obj::X, at);
            let r = rand_and(x, 7) * 2;
            let target = load_target(x);
            let off = x.rom16(TELEPORT_Y + r);
            let at = off.wrapping_add(target.u16(x.b, Obj::Y));
            x.set16(Obj::Y, at);
            x.set8(f::FLIP, 0x20);
            let dx = x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X));
            if (dx as i16) < 0 {
                x.set8(f::FLIP, 0);
            }
            mv(x, 3);
        }
        8 => {
            if x.anim_step() {
                done_attack(x);
            }
        }
        _ => {}
    }
}

// --- substate 8 --------------------------------------------------------------------------------------

/// +6 = 0 : courte pause debout.
fn pause<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        x.set8(TIMER, 0x10);
        return mv(x, 0);
    }
    let n = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, n);
    if (n as i8) < 0 {
        set_state_long(x, STANCE);
    }
    x.anim_step();
}

/// +6 = 2: step backward.
fn back_step<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 => crouch(x, 0xa),
        4 => shift(x, 0x30),
        6 => {
            if x.anim_step() {
                done_attack(x);
            }
        }
        _ => {}
    }
}

/// +6 = 4: leap (or run) toward the target.
fn lunge<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            if rand_and(x, 1) != 0 {
                return next_and_start(x);
            }
            x.add8(f::SUB3, 0xc);
            x.set8(TIMER, 0x10);
            set_speed(x, 0xd);
            x.set8(Obj::SPEED_INDEX, 0);
            let target = load_target(x);
            let d = x.u16(Obj::Y).wrapping_sub(target.u16(x.b, Obj::Y));
            if (d as i16) < 0 {
                x.set8(Obj::SPEED_INDEX, 0x10);
            }
            mv(x, 1);
        }
        2 | 6 => next_when_done(x),
        4 => {
            x.add8(f::SUB3, 2);
            let target = load_target(x);
            let d = x.u16(Obj::Y).wrapping_sub(target.u16(x.b, Obj::Y));
            let step: i16 = if (d as i16) < 0 { -0x32 } else { 0x32 };
            let at = x.u16(Obj::Y).wrapping_add(step as u16);
            x.set16(Obj::Y, at);
            mv(x, 3);
        }
        8 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set32(Obj::Z, 0);
            mv(x, 5);
        }
        10 => {
            if x.anim_step() {
                done_attack(x);
            }
        }
        12 => {
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if (n as i8) < 0 {
                set_state_long(x, STANCE);
            }
            x.move_by_table_xy();
            x.anim_step();
        }
        _ => {}
    }
}

/// +6 = 6: chooses a move again; changes move only if the new one differs from the previous one or after a delay.
fn pick_again<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(VULNERABLE, 1);
    set_state_long(x, IDLE);
    let table = if near_box(x) {
        NEAR_TABLE
    } else if far_box(x) {
        FAR_TABLE
    } else {
        return;
    };
    let b = pick_attack(x, table);
    if b != x.u8(LOOK_PREV) {
        x.set8(LOOK_PREV, b);
        x.set8(COOLDOWN, 2);
        return;
    }
    let n = x.u8(COOLDOWN).wrapping_sub(1);
    x.set8(COOLDOWN, n);
    if (n as i8) < 0 {
        x.set8(COOLDOWN, 2);
    }
}
