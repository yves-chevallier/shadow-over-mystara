//! Type 19, the beast's attacks (substate `+5 = 4`, 0x72faa): `+6` chooses the attack, `+7` its steps.
//!
//! All attacks start the same way: `+7 = 0` the beast gets ready (rest animation), `+7 = 2` it follows its target with
//! its gaze (turns around if the target is behind); the following steps are specific to the attack. When the attack is
//! over, `next_move` chains on or puts the beast back on its feet.

use crate::families::cx::Bench;
use crate::families::eshl_creators;
use super::parts::{bound_land, bound_wait};
use super::stance::{engage, next_move};
use super::*;
use crate::objects::Current;

/// Long state to resume after a turn-around: substate 4 (here) or 6 (`attacks_b`).
pub(super) const RESUME_ATTACK: u32 = 0x0204_0000;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => jab(x),
        2 => jab_b(x),
        4 => lunge(x),
        6 => double_strike(x),
        8 => claw_pair(x),
        10 => leap_strike(x),
        12 => jump_in(x),
        14 | 34 => pounce(x),
        16 | 36 => backflip(x),
        18 => turn_run(x),
        20 => leap_strike_b(x),
        22 => claw(x),
        24 => claw_b(x),
        26 => claw_series(x),
        28 => claw_series_b(x),
        30 => claw_leap(x),
        32 => crouch(x),
        _ => {}
    }
}

// --- common steps ----------------------------------------------------------------------------

/// +7 = 0 (0x72e80): the beast gets ready (rest animation).
pub(super) fn ready<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Enemy::SUB_STEP, 2);
    x.set8(Enemy::VULNERABLE, 1);
    x.set8(DisplacerBeast::WANT_ANIM, 0);
    if x.u8(DisplacerBeast::WANT_ANIM) != x.u8(DisplacerBeast::CUR_ANIM) {
        x.set8(DisplacerBeast::CUR_ANIM, 0);
        play(x, 0);
    }
}

/// +7 = 2 (0x72ffc / 0x72ff0): it follows its target with its gaze for `+0x77` frames (turns around if needed, then
/// resumes the attack `resume`).
pub(super) fn track<R: Bench>(x: &mut Cx<'_, R>, resume: u32) {
    x.set32(DisplacerBeast::RESUME_STATE, resume);
    let n = x.u8(Enemy::AI_BYTE).wrapping_sub(1);
    x.set8(Enemy::AI_BYTE, n);
    if (n as i8) < 0 {
        return x.add8(Enemy::SUB_STEP, 2);
    }
    if player_ahead(x) {
        let sub2 = x.u8(f::SUB2);
        x.set8(DisplacerBeast::RESUME_SUB2, sub2);
        set_state_long(x, 0x0202_0200);
    }
    step(x);
}

/// The first three common steps, then `later(step)` for the others.
pub(super) fn phased<'a, R: Bench>(x: &mut Cx<'a, R>, resume: u32, later: impl FnOnce(&mut Cx<'a, R>, u8)) {
    match x.u8(Enemy::SUB_STEP) {
        0 => ready(x),
        2 => track(x, resume),
        n => later(x, n),
    }
}

/// What an attack must do when it wants to play its animation `+0xa5`.
pub(super) enum Replay {
    /// Another animation was running: start it.
    New,
    /// The same one was already running: one repetition fewer (`+0xae`), some remain.
    Again,
    /// No more repetitions: the beast is put back on its feet.
    End,
}

pub(super) fn replay<R: Bench>(x: &mut Cx<'_, R>) -> Replay {
    let want = x.u8(DisplacerBeast::WANT_ANIM);
    if want != x.u8(DisplacerBeast::CUR_ANIM) {
        x.set8(DisplacerBeast::CUR_ANIM, want);
        return Replay::New;
    }
    let left = x.u8(DisplacerBeast::REPEAT_LEFT).wrapping_sub(1);
    x.set8(DisplacerBeast::REPEAT_LEFT, left);
    if left == 0 {
        set_state_long(x, ACTIVE_STANCE);
        return Replay::End;
    }
    Replay::Again
}

/// Start of an attack: next step, vulnerability, wanted animation, strength of the attack.
fn strike_begin<R: Bench>(x: &mut Cx<'_, R>, vulnerable: u8, want: u8, power: u8, kind: u8) {
    x.add8(Enemy::SUB_STEP, 2);
    x.set8(Enemy::VULNERABLE, vulnerable);
    x.set8(Enemy::STRIKE_POWER, power);
    x.set8(Enemy::STRIKE_KIND, kind);
    x.set8(DisplacerBeast::WANT_ANIM, want);
}

/// Starts animation `n` of the attack series, unless the series of repetitions is over.
fn strike_play<R: Bench>(x: &mut Cx<'_, R>, n: u8) {
    if !matches!(replay(x), Replay::End) {
        play_b(x, n);
    }
}

/// End of an attack animation, back to rest then choice of the next attack.
fn strike_end<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        play(x, 0);
        next_move(x);
    }
}

/// End of an attack animation, immediate choice of the next attack (0x73108).
fn strike_end_quick<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        next_move(x);
    }
}

/// A launched shot (0x9dda8 or 0x9e062), when the key frame is reached.
fn key_throw<R: Bench>(x: &mut Cx<'_, R>, throw: fn(&mut cps2::Cps2Bus, crate::objects::Obj)) {
    if x.u8(f::KEY_FRAME) != 0 {
        x.set8(f::KEY_FRAME, 0);
        throw(x.b, x.o);
    }
}

/// Walking direction: to the right (8) or the left (0x18) according to the orientation.
fn walk_angle<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Obj::SPEED_INDEX, 8);
    if x.u8(f::FLIP) == 0 {
        x.set8(Obj::SPEED_INDEX, 0x18);
    }
}

fn random_bit<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    (x.random() as u32) & 1 != 0
}

// --- the attacks ----------------------------------------------------------------------------------

/// 0x72fdc: fast paw strike.
fn jab<R: Bench>(x: &mut Cx<'_, R>) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => {
            strike_begin(x, 0, 0x2d, 5, 0);
            strike_play(x, 3);
        }
        6 => strike_end(x),
        _ => {}
    });
}

/// 0x730a2 : second coup de patte.
fn jab_b<R: Bench>(x: &mut Cx<'_, R>) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => {
            strike_begin(x, 0, 0x28, 5, 1);
            strike_play(x, 0);
        }
        6 => strike_end_quick(x),
        _ => {}
    });
}

/// 0x73118: advances then leaps on a player, clawing.
fn lunge<R: Bench>(x: &mut Cx<'_, R>) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(DisplacerBeast::TIMER, 0x14);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(DisplacerBeast::WANT_ANIM, 0x16);
            walk_angle(x);
            play(x, 0xd);
            speed(x, 0x17);
        }
        6 => {
            let t = x.u8(DisplacerBeast::TIMER).wrapping_sub(1);
            x.set8(DisplacerBeast::TIMER, t);
            if (t as i8) >= 0 {
                step(x);
                return move_xy(x);
            }
            x.add8(Enemy::SUB_STEP, 2);
            let flip = x.u8(f::FLIP) != 0;
            x.set16(Enemy::VX, if flip { -0x180i16 } else { 0x180 } as u16);
            x.set16(Enemy::AX, if flip { 0x10 } else { -0x10i16 } as u16);
            play_b(x, 0);
        }
        8 => {
            if x.u8(Enemy::VX) != 0 {
                x.move_x();
            }
            if x.anim_step() {
                x.set32(Enemy::VX_LONG, 0);
                next_move(x);
            }
        }
        _ => {}
    });
}

/// 0x731da: two or three paw strikes in a row.
fn double_strike<R: Bench>(x: &mut Cx<'_, R>) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => {
            let count = if random_bit(x) { 3 } else { 2 };
            x.set8(DisplacerBeast::REPEAT, count);
            strike_begin(x, 0, 0x2d, 5, 0);
            match replay(x) {
                Replay::New => play_b(x, 3),
                Replay::Again => {
                    let a = if random_bit(x) { 3 } else { 0 };
                    play_b(x, a);
                }
                Replay::End => {}
            }
        }
        6 => {
            if x.anim_step() {
                play(x, 0);
                let left = x.u8(DisplacerBeast::REPEAT).wrapping_sub(1);
                x.set8(DisplacerBeast::REPEAT, left);
                if left == 0 {
                    return next_move(x);
                }
                let a = if random_bit(x) { 3 } else { 0 };
                play_b(x, a);
            }
        }
        _ => {}
    });
}

/// 0x732b4: paw strike followed, at random, by a second one.
fn claw_pair<R: Bench>(x: &mut Cx<'_, R>) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => {
            strike_begin(x, 0, 0x2a, 5, 1);
            play_b(x, 1);
            if random_bit(x) {
                x.add8(Enemy::SUB_STEP, 2);
                x.set8(DisplacerBeast::WANT_ANIM, 0x2b);
                play_b(x, 2);
            }
            replay(x);
        }
        6 => strike_end_quick(x),
        8 => strike_end(x),
        _ => {}
    });
}

/// Leap with claws out, initial X speed `vx` (0x7337a and its three entry points).
fn leap_strike_start<R: Bench>(x: &mut Cx<'_, R>, vx: i16) {
    strike_begin(x, 1, 0x2f, 6, 2);
    let vx = if x.u8(f::FLIP) != 0 { vx.wrapping_neg() } else { vx };
    set_velocity(x, vx, 0x4c0, -0x38);
    strike_play(x, 5);
}

/// Landing of a clawing leap: wait for the key frame, land, finish the animation.
fn leap_landing<R: Bench>(x: &mut Cx<'_, R>, n: u8, first: u8) {
    match n.wrapping_sub(first) {
        0 => bound_wait(x),
        2 => bound_land(x),
        _ => strike_end_quick(x),
    }
}

/// 0x73348 : bond en avant griffes dehors.
fn leap_strike<R: Bench>(x: &mut Cx<'_, R>) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => leap_strike_start(x, -0x1c0),
        n => leap_landing(x, n, 6),
    });
}

/// 0x73754: the same leap, faster.
fn leap_strike_b<R: Bench>(x: &mut Cx<'_, R>) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => leap_strike_start(x, -0x300),
        n => leap_landing(x, n, 6),
    });
}

/// 0x733e8: jumps towards a player.
fn jump_in<R: Bench>(x: &mut Cx<'_, R>) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => super::moves::jump_toward(x),
        6 => super::moves::jump_wait(x),
        8 => bound_wait(x),
        10 => bound_land(x),
        12 => super::moves::jump_end(x),
        _ => {}
    });
}

/// 0x7346a: takes a run-up.
fn crouch<R: Bench>(x: &mut Cx<'_, R>) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(DisplacerBeast::TIMER, 0xe);
            play(x, 8);
            speed(x, 0x11);
        }
        6 => super::moves::jump_wait(x),
        _ => {}
    });
}

/// 0x734a8: jumps in place several times (the number is drawn from a table).
fn pounce<R: Bench>(x: &mut Cx<'_, R>) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => pounce_start(x),
        6 => pounce_repeat(x),
        _ => {}
    });
}

/// Draws the number of jumps in place (table `pounce_counts`).
pub(super) fn roll_pounces<R: Bench>(x: &mut Cx<'_, R>) {
    let r = (x.random() as u32) & 0x1f;
    let count = tables::table("dbeast/pounce_counts").at8(x.b, r);
    x.set8(DisplacerBeast::REPEAT, count);
}

/// 0x734bc: start of a series of jumps in place.
fn pounce_start<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Enemy::SUB_STEP, 2);
    x.set8(Enemy::VULNERABLE, 1);
    x.set8(DisplacerBeast::WANT_ANIM, 7);
    roll_pounces(x);
    if !matches!(replay(x), Replay::End) {
        play(x, 7);
    }
}

/// 0x73536: a jump in place is over; at random a cry accompanies the key frame.
pub(super) fn pounce_repeat<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::KEY_FRAME) != 0 {
        x.set8(f::KEY_FRAME, 0);
        let r = (x.random() as u32) & 0xf;
        let sound = tables::table("dbeast/pounce_cries").at8(x.b, r);
        if sound != 0 {
            lib::sound(x, crate::sound::CMD_31B_41B);
        }
    }
    if x.anim_step() {
        let left = x.u8(DisplacerBeast::REPEAT).wrapping_sub(1);
        x.set8(DisplacerBeast::REPEAT, left);
        if left == 0 {
            return next_move(x);
        }
        play(x, 7);
    }
}

/// 0x73594: somersault.
fn backflip<R: Bench>(x: &mut Cx<'_, R>) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(Enemy::STRIKE_POWER, 6);
            x.set8(Enemy::STRIKE_KIND, 2);
            x.set32(Enemy::VX_LONG, 0);
            x.set16(Enemy::VZ, 0x380);
            x.set16(Enemy::GRAVITY, (-0x38i16) as u16);
            play(x, 6);
        }
        6 | 10 => bound_wait(x),
        8 => {
            if x.move_xz().gt() {
                return step(x);
            }
            x.set32(Obj::Z, 0);
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(DisplacerBeast::WANT_ANIM, 0x18);
            x.set8(DisplacerBeast::CUR_ANIM, 0x18);
            x.set32(Enemy::VX_LONG, 0);
            x.set16(Enemy::VZ, 0x380);
            x.set16(Enemy::GRAVITY, (-0x38i16) as u16);
            play(x, 6);
        }
        12 => bound_land(x),
        14 => strike_end(x),
        _ => {}
    });
}

/// 0x7365e: turns around while running, heads back toward the player then stops.
fn turn_run<R: Bench>(x: &mut Cx<'_, R>) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Enemy::VULNERABLE, 1);
            let ahead = target(x).is_some_and(|t| (t.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X)) as i16) >= 0);
            x.set8(Obj::SPEED_INDEX, if ahead { 0x18 } else { 8 });
            play(x, 5);
        }
        6 => {
            if x.anim_step() {
                x.add8(Enemy::SUB_STEP, 2);
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                x.set8(DisplacerBeast::WANT_ANIM, 0x16);
                x.set8(DisplacerBeast::TIMER, 0x2a);
                play(x, 0xd);
                speed(x, 0x17);
            }
        }
        8 => {
            let t = x.u8(DisplacerBeast::TIMER).wrapping_sub(1);
            x.set8(DisplacerBeast::TIMER, t);
            if (t as i8) >= 0 {
                step(x);
                return move_xy(x);
            }
            x.add8(Enemy::SUB_STEP, 2);
            play(x, 5);
        }
        10 => {
            if x.anim_step() {
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                set_state_long(x, 0x0202_0002);
                x.set8(DisplacerBeast::WANT_ANIM, 0xc);
                play(x, 8);
                engage(x);
            }
        }
        _ => {}
    });
}

/// Start of a claw (0x73786, 0x7382c): animation 4 of the attack series.
fn claw_begin<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Enemy::SUB_STEP, 2);
    x.set8(DisplacerBeast::WANT_ANIM, 0x2e);
    x.set8(Enemy::VULNERABLE, 1);
    strike_play(x, 4);
}

/// End of a simple claw: impact sound, then rest and next attack.
fn claw_end<R: Bench>(x: &mut Cx<'_, R>, throw: fn(&mut cps2::Cps2Bus, crate::objects::Obj)) {
    key_throw(x, throw);
    strike_end(x);
}

/// 0x7376c : griffe.
fn claw<R: Bench>(x: &mut Cx<'_, R>) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => claw_begin(x),
        6 => claw_end(x, eshl_creators::throw_1),
        _ => {}
    });
}

/// 0x737fe: claw, other sound.
fn claw_b<R: Bench>(x: &mut Cx<'_, R>) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => claw_begin(x),
        6 => claw_end(x, eshl_creators::throw_2),
        _ => {}
    });
}

/// End of a claw in a series of `+0xab`: the last one brings it back to rest.
fn series_end<R: Bench>(x: &mut Cx<'_, R>, throw: fn(&mut cps2::Cps2Bus, crate::objects::Obj)) {
    key_throw(x, throw);
    if x.anim_step() {
        play_b(x, 4);
        let left = x.u8(DisplacerBeast::REPEAT).wrapping_sub(1);
        x.set8(DisplacerBeast::REPEAT, left);
        if left == 0 {
            play(x, 0);
        }
        next_move(x);
    }
}

/// 0x738a4: one claw, then next attack.
fn claw_series<R: Bench>(x: &mut Cx<'_, R>) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => {
            x.set8(DisplacerBeast::REPEAT, 1);
            claw_begin(x);
        }
        6 => series_end(x, eshl_creators::throw_1),
        _ => {}
    });
}

/// 0x73904: one or two claws.
fn claw_series_b<R: Bench>(x: &mut Cx<'_, R>) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => {
            let count = if random_bit(x) { 2 } else { 1 };
            x.set8(DisplacerBeast::REPEAT, count);
            claw_begin(x);
        }
        6 => series_end(x, eshl_creators::throw_2),
        _ => {}
    });
}

/// 0x73964: claw then clawing leap.
fn claw_leap<R: Bench>(x: &mut Cx<'_, R>) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => claw_begin(x),
        6 => {
            key_throw(x, eshl_creators::throw_2);
            if x.anim_step() {
                x.add8(Enemy::SUB_STEP, 2);
                play_b(x, 5);
            }
        }
        8 => leap_strike_start(x, -0x200),
        n => leap_landing(x, n, 10),
    });
}
