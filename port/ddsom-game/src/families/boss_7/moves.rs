//! Boss 7, groups 6 and 8: dodges (disappearance and reappearance, backward leap, short walk), started by
//! `aids::after_attack` and by hits taken. Group 6 draws two variants of the reappearance according to `+6`, group 8
//! has four.

use crate::families::cx::Bench;
use super::aids;
use super::attacks::{leap_rest, leap_start};
use super::*;
use crate::objects::Current;

fn step<R: Bench>(x: &mut Cx<'_, R>, table: u32) -> u32 {
    dispatch(x, table, f::SUB3)
}

/// 0x8699e: the boss fades out (common start of the dodges).
fn fade_out<R: Bench>(x: &mut Cx<'_, R>) {
    addq8(x, f::SUB3, 2);
    x.set8(0, 2);
    key(x, 0x12);
    lib::sound(x, 0xecf7a);
    body(x, 4);
}

/// 0x86a06: the boss reappears when the countdown has elapsed.
fn fade_in<R: Bench>(x: &mut Cx<'_, R>) {
    if timer_done(x) {
        addq8(x, f::SUB3, 2);
        x.set_bit(0x1, 1, false);
        x.set8(0, 1);
        lib::sound(x, 0xecf7a);
    }
}

/// End of the dodges: plays the animation, then moves to `state`.
fn finish<R: Bench>(x: &mut Cx<'_, R>, state: u32) {
    if x.anim_step() {
        x.set32(f::STATE, state);
    }
}

/// Reappearance: the boss is invisible (`bset 1`) for 8 frames, animation 5.
fn reappear<R: Bench>(x: &mut Cx<'_, R>) {
    x.set_bit(0x1, 1, true);
    x.set16(TIMER, 8);
    body(x, 5);
}

/// 0x869c6: reappearance slightly behind (0x60 pixels), within the play area.
fn back_off<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        addq8(x, f::SUB3, 2);
        let mut d = (-0x60i16) as u16;
        if x.u8(f::FLIP) != 0 {
            d = d.wrapping_neg();
        }
        x.add16(0x8, d);
        aids::clamp(x);
        reappear(x);
    }
}

/// 0x86a4e: reappearance at a point drawn according to the screen quarter.
fn jump_away<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        addq8(x, f::SUB3, 2);
        aids::dodge_spot(x);
        reappear(x);
    }
}

/// 0x868a4 (`+5` = 6).
pub(super) fn group6<R: Bench>(x: &mut Cx<'_, R>) {
    let table = if dispatch(x, 0x868b2, f::SUB2) == 0x868ba { 0x868c8 } else { 0x868de };
    match step(x, table) {
        0x8699e => fade_out(x),
        0x869c6 => back_off(x),
        0x86a4e => jump_away(x),
        0x86a06 => fade_in(x),
        _ => finish(x, FIGHT),
    }
}

/// 0x868e6 (`+5` = 8).
pub(super) fn group8<R: Bench>(x: &mut Cx<'_, R>) {
    match dispatch(x, 0x868f4, f::SUB2) {
        0x868fe => short_walk(x),
        0x86988 => match step(x, 0x86996) {
            0x8699e => fade_out(x),
            0x869c6 => back_off(x),
            0x86a06 => fade_in(x),
            _ => finish(x, 0x0204_0a00),
        },
        0x86a38 => match step(x, 0x86a46) {
            0x8699e => fade_out(x),
            0x86a4e => jump_away(x),
            0x86a06 => fade_in(x),
            _ => finish(x, FIGHT),
        },
        // backward leap: the steps of the attack leap, but it ends on combat (0x86a7c)
        _ => match step(x, 0x86a9e) {
            0x86aaa => leap_start(x, 0x400),
            0x86a7c => finish(x, FIGHT),
            t => leap_rest(x, t),
        },
    }
}

/// 0x868fe (`+6` = 0): a few steps in place (0x14 frames) while watching the opponent.
fn short_walk<R: Bench>(x: &mut Cx<'_, R>) {
    match step(x, 0x8690c) {
        0x86950 => {
            if timer_done(x) {
                x.set32(f::STATE, FIGHT);
                return;
            }
            if aids::must_turn(x) {
                x.set32(RESUME_STATE, 0x0208_0004);
                let b6 = x.u8(f::SUB2);
                x.set8(RESUME_SUB, b6);
                x.set32(f::STATE, 0x0202_0200);
                return;
            }
            x.anim_step();
        }
        t => {
            if t == 0x8691a {
                addq8(x, f::SUB3, 2);
                x.set16(TIMER, 0x14);
            } else {
                x.add8(f::SUB3, 0xfe);
            }
            x.set8(VULNERABLE, 1);
            x.set8(ANIM_KIND, 0);
            setb(x, 2, 0);
            if x.u8(ANIM_LAST) != 0 {
                x.set8(ANIM_LAST, 0);
            }
            body(x, 0);
        }
    }
}
