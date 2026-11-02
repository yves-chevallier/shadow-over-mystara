//! Boss 8, groups 6 and 8: dodges (disappearance and reappearance, sidestep, leap, retreat), started by
//! `aids::after_attack` and by hits taken. Group 6 draws two variants of step 2 according to `+6`, group 8 has
//! five.

use crate::families::cx::Bench;
use super::aids;
use super::attacks::{leap_rest, leap_start};
use super::*;
use crate::objects::Current;

fn key<R: Bench>(x: &mut Cx<'_, R>, kind: u8) {
    x.set8(ANIM_KIND, kind);
    x.set8(ANIM_LAST, kind);
}

fn step<R: Bench>(x: &mut Cx<'_, R>, table: u32) -> u32 {
    dispatch(x, table, f::SUB3)
}

/// 0x89030: the boss fades out (common start of the dodges).
fn fade_out<R: Bench>(x: &mut Cx<'_, R>) {
    addq8(x, f::SUB3, 2);
    x.set8(PHASE_TIMER, 0x10);
    key(x, 0x12);
    lib::sound(x, crate::sound::CMD_349);
    body(x, 4);
}

/// 0x8909a: the boss reappears when the countdown has elapsed.
fn fade_in<R: Bench>(x: &mut Cx<'_, R>) {
    if sub16(x, TIMER, 1).mi() {
        addq8(x, f::SUB3, 2);
        x.set_bit(0x1, 1, false);
        lib::sound(x, crate::sound::CMD_349);
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

/// 0x88f36 (`+5` = 6).
pub(super) fn group6<R: Bench>(x: &mut Cx<'_, R>) {
    let table = if dispatch(x, 0x88f44, f::SUB2) == 0x88f4c { 0x88f5a } else { 0x88f70 };
    match step(x, table) {
        0x89030 => fade_out(x),
        0x8905a => {
            if x.anim_step() {
                addq8(x, f::SUB3, 2);
                let mut d = (-0x80i16) as u16;
                if x.u8(f::FLIP) != 0 {
                    d = d.wrapping_neg();
                }
                x.add16(0x8, d);
                aids::clamp(x);
                reappear(x);
            }
        }
        0x890de => {
            if x.anim_step() {
                addq8(x, f::SUB3, 2);
                aids::dodge_spot(x);
                reappear(x);
            }
        }
        0x8909a => fade_in(x),
        _ => finish(x, 0x0202_0000),
    }
}

/// 0x88f78 (`+5` = 8).
pub(super) fn group8<R: Bench>(x: &mut Cx<'_, R>) {
    match dispatch(x, 0x88f86, f::SUB2) {
        0x88f90 => rest(x),
        0x8901a => match step(x, 0x89028) {
            0x89030 => fade_out(x),
            0x8905a => {
                if x.anim_step() {
                    addq8(x, f::SUB3, 2);
                    let mut d = (-0x80i16) as u16;
                    if x.u8(f::FLIP) != 0 {
                        d = d.wrapping_neg();
                    }
                    x.add16(0x8, d);
                    aids::clamp(x);
                    reappear(x);
                }
            }
            0x8909a => fade_in(x),
            _ => finish(x, 0x0204_0a00),
        },
        0x890c8 => match step(x, 0x890d6) {
            0x89030 => fade_out(x),
            0x890de => {
                if x.anim_step() {
                    addq8(x, f::SUB3, 2);
                    aids::dodge_spot(x);
                    reappear(x);
                }
            }
            0x8909a => fade_in(x),
            _ => finish(x, 0x0202_0000),
        },
        0x89120 => match step(x, 0x8912e) {
            0x8913a => leap_start(x, 0x400, 0x10, 9),
            0x8910c => finish(x, 0x0202_0000),
            t => leap_rest(x, t),
        },
        _ => match step(x, 0x89194) {
            0x89030 => fade_out(x),
            0x8919c => {
                if x.anim_step() {
                    addq8(x, f::SUB3, 2);
                    aids::random_spot(x);
                    aids::clamp(x);
                    x.set8(f::FLIP, 0);
                    if !Fl::sub16(x.u16(0x8), 0xc0).gt() {
                        x.set8(f::FLIP, 0x20);
                    }
                    reappear(x);
                }
            }
            0x8909a => fade_in(x),
            _ => finish(x, 0x0202_0000),
        },
    }
}

/// 0x88f90 (`+6` = 0 of group 8): standing wait, then return to combat.
fn rest<R: Bench>(x: &mut Cx<'_, R>) {
    match dispatch(x, 0x88f9e, f::SUB3) {
        0x88fe2 => {
            if sub16(x, TIMER, 1).mi() {
                x.set32(f::STATE, 0x0202_0000);
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
            if t == 0x88fac {
                addq8(x, f::SUB3, 2);
                x.set16(TIMER, 0x14);
            } else {
                sub8(x, f::SUB3, 2);
            }
            x.set8(VULNERABLE, 1);
            x.set8(ANIM_KIND, 0);
            let k = x.u8(ANIM_KIND);
            if k != x.u8(ANIM_LAST) {
                x.set8(ANIM_LAST, k);
            }
            body(x, 0);
        }
    }
}
