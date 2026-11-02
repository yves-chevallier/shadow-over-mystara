//! Enemy "Riyakaa" (ENEMY type 16, 0x6f1c0). See `docs/families/enemy.md`.
//!
//! Four-level state machine: `+4` main state (0 creation, 2 active, 4 hit, 6 release), `+5` substate (active state: 0
//! arrival according to the variant `+3`, 2 choice of an attack, 4 paw strikes, 6 special hits, 8 pursuit, 10 rest),
//! `+6` and `+7` sub-substates. Each function below is a substate of the original, named after its role; the original
//! address is noted when it helps to find the code.

use crate::families::cx::Bench;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Current;

mod attacks;
mod decide;
mod hit;

use attacks::*;
use decide::*;
use hit::*;

/// Animation sets of the body and of the hits.
const BODY: u32 = 0x1665d6;
const STRIKE: u32 = 0x1665f8;
/// Address tables of the reactions to hits (state 4: hit while standing / hit in the middle of an action).
const REACT_STANDING: u32 = 0x6fe22;
const REACT_ACTING: u32 = 0x70018;
/// Bytes for choosing the next gesture, 32 per difficulty level (`A5+0x7f92` / 2), 5-bit draw.
const NEXT_MOVES: u32 = 0x5ce72;
const NEXT_ATTACKS: u32 = 0x701dc;
/// Decision masks of the counter-attack (one long per difficulty level).
const MASKS: u32 = 0x6ff70;
/// Long state "active, substate 0": return to walking.
const WALK: u32 = 0x0202_0000;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    match x.u8(f::STATE) {
        st::INIT => {
            if paused(x) {
                return;
            }
            init(x);
            active(x);
        }
        st::ACTIVE => active(x),
        st::HIT => hit(x),
        st::DEAD => free(x),
        _ => {}
    }
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::STATE, 2);
    x.set8(f::BOX_SET, 0xf);
    lib::initial_hp(x);
    lib::set_immunities(x);
    // AI parameters read by the decision tables, and starting values
    for (off, v) in [
        (0x68, 2),
        (0x69, 2),
        (0x70, 0),
        (0x6a, 1),
        (0x6c, 2),
        (0x6d, 4),
        (0x72, 2),
        (0x73, 2),
        (0x74, 2),
        (0x75, 0),
        (0x76, 3),
        (0xa2, 0x20),
        (0xa1, 2),
        (0x67, 1),
        (0x8e, 0),
        (0x7f, 0),
        (0x7e, 0x44),
    ] {
        x.set8(off, v);
    }
    x.set32(0x8a, 0);
    x.set8(0xad, 0);
    x.set8(0x79, 6);
    x.set8(0x81, 6);
    x.set8(0x80, 7);
}

/// Plays animation `n` of the movement set.
fn body<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(BODY, n as u16);
}

/// Plays animation `n` of the hit set.
fn strike<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(STRIKE, n as u16);
}

/// Chooses speed table `n`.
fn speed<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
}

/// Return to the active state, substate 0.
fn walk<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, WALK);
}

fn toggle_flip<R: Bench>(x: &mut Cx<'_, R>) {
    let flip = x.u8(f::FLIP) ^ 0x20;
    x.set8(f::FLIP, flip);
}

/// Decrements a byte and returns its new value.
fn countdown<R: Bench>(x: &mut Cx<'_, R>, off: u32) -> u8 {
    let n = x.u8(off).wrapping_sub(1);
    x.set8(off, n);
    n
}

// --- state 2: active -----------------------------------------------------------------------

fn active<R: Bench>(x: &mut Cx<'_, R>) {
    active_loop(x, |x| {
        match x.u8(f::SUB) {
            0 => return arrive(x),
            2 => decide(x),
            4 => swipe(x),
            6 => special(x),
            8 => chase(x),
            10 => rest(x),
            _ => {}
        }
        false
    });
}

/// Substate 0: arrival according to the variant `+3`. Returns true when the original abandons the frame without
/// finishing it (variant 8, `+6 = 2`: it pops the return address of the substate function).
fn arrive<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    match x.u8(0x3) {
        0 | 4 => charge_in(x),
        2 => rise(x),
        6 => leap_in(x),
        8 => return approach(x),
        10 => hover(x),
        _ => {}
    }
    false
}

/// Movement common to the arrivals: the duration `+0xa2` counted down, then return to walking.
fn fly_step<R: Bench>(x: &mut Cx<'_, R>) {
    if countdown(x, 0xa2) == 0 {
        walk(x);
        x.set8(0, 1);
    }
    x.move_by_table_xy();
    x.anim_step();
}

/// 0x6f2e0: enters running (variants 0 and 4).
fn charge_in<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) != 0 {
        return fly_step(x);
    }
    x.add8(f::SUB2, 2);
    x.set8(0x67, 1);
    x.set16(0xa0, 0);
    x.set8(0xa2, 0x18);
    x.set8(0x2d, 0x18);
    if x.u8(f::FLIP) != 0 {
        x.set8(0x2d, 8);
    }
    x.set16(0xa0, 0x202);
    speed(x, 0x17);
    let mut n = 3;
    if x.u8(0x3) == 0 {
        x.set16(0xa0, 0);
        speed(x, 0xd);
        n = 2;
    }
    body(x, n);
}

/// 0x6f35e: enters leaping (variant 6).
fn leap_in<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) != 0 {
        return fly_step(x);
    }
    x.add8(f::SUB2, 2);
    x.set8(0x67, 1);
    x.set8(0xa2, 0x20);
    x.set8(0xa1, 0xd);
    x.set8(0x2d, 0x10);
    speed(x, 0xd);
    body(x, 6);
}

/// 0x6f3aa: start of the fall (springs upward then falls back).
fn rise_start<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 2);
    x.set8(0x67, 0);
    x.set8(0xa1, 0xe);
    x.set16(0x18, 0xfd00);
    x.set16(0x1a, 0xff80);
    body(x, 0xb);
}

/// 0x6f3d2: falls back to the ground.
fn rise_fall<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_z();
    if (x.u16(0xc) as i16) >= 0 {
        return;
    }
    x.add8(f::SUB2, 2);
    x.set32(0xc, 0);
    body(x, 0x10);
}

/// 0x6f3f6: end of the landing animation.
fn rise_land<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        walk(x);
        x.set8(0, 1);
    }
}

/// Variant 2: falls from the top of the screen.
fn rise<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => rise_start(x),
        2 => rise_fall(x),
        4 => rise_land(x),
        _ => {}
    }
}

/// Variant 10: appears after 30 frames, then falls like variant 2.
fn hover<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.set8(0x90, 0x1e);
            rise_start(x);
        }
        2 => {
            if countdown(x, 0x90) == 0 {
                x.add8(f::SUB2, 2);
            }
        }
        4 => rise_fall(x),
        6 => rise_land(x),
        _ => {}
    }
}

/// Variant 8: approaches until it sees a player; at `+6 = 2` the original abandons the frame.
fn approach<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(0x67, 0);
            x.set8(0xa1, 0xb);
            body(x, 0xd);
        }
        2 => {
            x.anim_step();
            if lib::pick_target(x).is_some() && lib::target_in_box(x, 0, 0x80, 0x20) {
                    x.add8(f::SUB2, 2);
                }
            return true;
        }
        4 => {
            x.add8(f::SUB2, 2);
            x.set8(0x67, 1);
            x.set8(0xa1, 0xc);
            body(x, 0xa);
        }
        6 => {
            if x.anim_step() {
                x.set8(f::SUB, 0xa);
                x.set16(f::SUB2, 0);
            }
        }
        _ => {}
    }
    false
}

/// Substate 10: waits, standing, until a player is alive.
fn rest<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set8(0x67, 1);
        x.set8(0xa1, 1);
        return body(x, 0);
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        x.anim_step();
    } else {
        walk(x);
        lib::pick_target(x);
    }
}

handlers! {
    0x6f1c0 "enemy_16" run,
}
