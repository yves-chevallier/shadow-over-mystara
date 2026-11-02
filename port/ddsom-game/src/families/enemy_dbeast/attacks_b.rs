//! Type 19, second series of the beast's attacks (substate `+5 = 6`, 0x739b2): leaps and jumps. Same common steps as
//! `attacks` (preparation, gaze), resume after a turn-around in substate 6.

use crate::families::cx::Bench;
use super::attacks::{phased, pounce_repeat, roll_pounces};
use super::parts::{bound_land, bound_wait, bound_y_land, bound_y_wait};
use super::stance::pursue_tail;
use super::*;

const RESUME_ATTACK_B: u32 = 0x0206_0000;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => hop_or_pounce(x),
        2 => roar(x),
        4 => side_leap(x),
        6 => short_leap(x),
        8 => leap_and_claw(x),
        10 => long_leap(x),
        12 => retreat(x),
        _ => {}
    }
}

/// Back to the standing beast, then continue the pursuit (0x73a4a).
fn end_and_pursue<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        set_state_long(x, ACTIVE_STANCE);
        x.set8(DisplacerBeast::WANT_ANIM, 0);
        play(x, 0);
        pursue_tail(x);
    }
}

/// 0x739cc: at random a small jump forward, otherwise a series of jumps in place.
fn hop_or_pounce<R: Bench>(x: &mut Cx<'_, R>) {
    phased(x, RESUME_ATTACK_B, |x, n| match n {
        4 => {
            if rand_and(x, 1) != 0 {
                set_state_long(x, 0x0202_0002);
                x.set8(DisplacerBeast::WANT_ANIM, 0xc);
                play(x, 8);
                return pursue_tail(x);
            }
            x.add8(Enemy::SUB_STEP, 4);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(DisplacerBeast::WANT_ANIM, 7);
            roll_pounces(x);
            play(x, 7);
        }
        6 => end_and_pursue(x),
        8 => pounce_repeat(x),
        _ => {}
    });
}

/// 0x73a78 : rugit.
fn roar<R: Bench>(x: &mut Cx<'_, R>) {
    phased(x, RESUME_ATTACK_B, |x, n| match n {
        4 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(DisplacerBeast::WANT_ANIM, 0xf);
            play(x, 0xb);
        }
        6 => end_and_pursue(x),
        _ => {}
    });
}

/// 0x73aae: sideways leap (in depth), up or down at random.
fn side_leap<R: Bench>(x: &mut Cx<'_, R>) {
    phased(x, RESUME_ATTACK_B, |x, n| match n {
        4 => {
            x.add8(Enemy::SUB_STEP, 2);
            let vy = if rand_and(x, 1) != 0 { -0x100i16 } else { 0x100 };
            x.set16(Enemy::VY, vy as u16);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(DisplacerBeast::WANT_ANIM, 6);
            x.set8(DisplacerBeast::CUR_ANIM, 6);
            x.set16(Enemy::AY, 0);
            x.set16(Enemy::VZ, 0x400);
            x.set16(Enemy::GRAVITY, (-0x38i16) as u16);
            play(x, 6);
        }
        6 => bound_y_wait(x),
        8 => bound_y_land(x),
        10 => end_and_pursue(x),
        _ => {}
    });
}

/// Start of an X leap of the second series (0x73b30): speed `vx`, height `vz`.
fn leap_start<R: Bench>(x: &mut Cx<'_, R>, vx: i16, vz: u16, anim: Option<u8>) {
    x.add8(Enemy::SUB_STEP, 2);
    x.set8(Enemy::VULNERABLE, 1);
    x.set8(DisplacerBeast::WANT_ANIM, 0x1c);
    x.set8(DisplacerBeast::CUR_ANIM, 0x1c);
    let vx = if x.u8(f::FLIP) != 0 { vx.wrapping_neg() } else { vx };
    set_velocity(x, vx, vz, -0x38);
    match anim {
        None => play(x, 0x11),
        Some(n) => {
            x.set8(Enemy::STRIKE_POWER, 6);
            x.set8(Enemy::STRIKE_KIND, 2);
            play_b(x, n);
        }
    }
}

/// 0x73b18: small leap forward.
fn short_leap<R: Bench>(x: &mut Cx<'_, R>) {
    phased(x, RESUME_ATTACK_B, |x, n| match n {
        4 => leap_start(x, 0x80, 0x2c0, None),
        6 => bound_wait(x),
        8 => bound_land(x),
        10 => end_and_pursue(x),
        _ => {}
    });
}

/// 0x73b7a: small leap then claw strike.
fn leap_and_claw<R: Bench>(x: &mut Cx<'_, R>) {
    phased(x, RESUME_ATTACK_B, |x, n| match n {
        4 => leap_start(x, 0x80, 0x2c0, None),
        6 => bound_wait(x),
        8 => bound_land(x),
        10 => {
            if x.anim_step() {
                x.add8(Enemy::SUB_STEP, 2);
                x.set8(Enemy::STRIKE_POWER, 5);
                x.set8(Enemy::STRIKE_KIND, 3);
                x.set8(DisplacerBeast::WANT_ANIM, 0x28);
                play_b(x, 0);
            }
        }
        12 => end_and_pursue(x),
        _ => {}
    });
}

/// 0x73bec : grand bond griffu.
fn long_leap<R: Bench>(x: &mut Cx<'_, R>) {
    phased(x, RESUME_ATTACK_B, |x, n| match n {
        4 => leap_start(x, 0x280, 0x2c0, Some(5)),
        6 => bound_wait(x),
        8 => bound_land(x),
        10 => end_and_pursue(x),
        _ => {}
    });
}

/// 0x73c5a: backs off roaring then turns around, on the ground.
fn retreat<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Enemy::SUB_STEP) == 0 {
        x.add8(Enemy::SUB_STEP, 2);
        return play(x, 0x18);
    }
    if x.anim_step() {
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        set_state_long(x, KNOCKED);
    }
}
