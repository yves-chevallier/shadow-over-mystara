//! Type 11, substate 4: the attacks. `+6` = 0 punch, 2 to 6 series of strikes (preceded by a turn-around and a pause),
//! 8 and 12 leaps into a diving strike, 10 approach walk, 14 and 16 leaps with turn-around, 18 and 20 crouched.

use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    let step = x.u8(f::SUB3);
    match x.u8(f::SUB2) {
        0 => match step {
            0 => {
                x.add8(f::SUB3, 2);
                set_strike(x, 0xa, 5);
                strike(x, 0);
            }
            2 => until_end(x),
            _ => {}
        },
        2 => match step {
            0 => coin_flip(x),
            2 => next_anim(x),
            4 => wait(x),
            6 => turn(x),
            8 => {
                x.add8(f::SUB3, 2);
                set_strike(x, 0xc, 4);
                strike(x, 1);
            }
            10 => until_end(x),
            _ => {}
        },
        4 => match step {
            0 => turn(x),
            2 => next_anim(x),
            4 => wait(x),
            6 => {
                if x.u8(Enemy::GROUP) != 0 {
                    x.set8(f::SUB2, 0x12);
                    return squat_step(x);
                }
                set_strike(x, 0xa, 4);
                x.set8(Enemy::AI_SHOT, 3);
                let state = x.u32(f::STATE);
                x.set32(Enemy::RESUME_STATE, state);
                x.add8(f::SUB3, 2);
                strike(x, 3);
            }
            8 => {
                if !x.anim_step() {
                    return;
                }
                if rand_and(x, 1) == 0 {
                    return choose(x);
                }
                x.add8(f::SUB3, 2);
                strike(x, 0);
            }
            10 => until_end(x),
            _ => {}
        },
        6 => match step {
            0 => turn(x),
            2 => next_anim(x),
            4 => wait(x),
            6 => {
                x.add8(f::SUB3, 2);
                let state = x.u32(f::STATE);
                x.set32(Enemy::RESUME_STATE, state);
                set_strike(x, 0xc, 2);
                x.set8(Enemy::AI_SHOT, 3);
                let anim = rand_and(x, 1);
                strike(x, anim);
            }
            8 => {
                if x.anim_step() {
                    x.add8(f::SUB3, 2);
                    strike(x, 1);
                }
            }
            10 => until_end(x),
            _ => {}
        },
        8 => match step {
            0 => next_anim(x),
            2 => wait(x),
            4 => leap_start(x, Some(0x180), Some(0xe)),
            6 => wait_key(x),
            8 => rise(x),
            10 => fall(x),
            12 => end_choose(x),
            _ => {}
        },
        10 => match step {
            0 => next_anim(x),
            2 => wait(x),
            4 => {
                x.add8(f::SUB3, 2);
                x.set16(Golem::TIMER, 0x2a);
                restart_walk(x);
            }
            6 => approach(x),
            _ => {}
        },
        12 => match step {
            0 => next_anim(x),
            2 => wait(x),
            4 => {
                x.set8(Enemy::STRIKE_KIND, 0x10);
                leap_start(x, Some(0x480), None);
            }
            6 => wait_key(x),
            8 => rise(x),
            10 => fall(x),
            12 => end_choose(x),
            _ => {}
        },
        14 | 16 => hop_turn(x, step),
        18 | 20 => squat_step(x),
        _ => {}
    }
}

/// 0x67c74: as long as end flag `+0x32` is not raised, the animation advances; then standing.
fn until_end<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u16(Enemy::ANIM_END) as i16) >= 0 {
        x.anim_step();
        return;
    }
    set_state_long(x, STANCE);
    x.set8(Enemy::AI_SHOT, 0xff);
    mv(x, 0);
}

/// 0x67c32: on a draw (ten chances out of sixteen), back to walking (state 0x2020600), otherwise next step.
fn coin_flip<R: Bench>(x: &mut Cx<'_, R>) {
    let r = rand_and(x, 0xf);
    if (0xffc0u32 >> r) & 1 != 0 {
        return set_state_long(x, 0x0202_0600);
    }
    x.add8(f::SUB3, 2);
}

/// 0x67dce / 0x67dda / 0x67f5e: starts a leap: horizontal speed (inverted if the orientation is zero), upward thrust
/// 0x900, strike of strength 5; `speed` / `kind` absent when the caller has already set them.
fn leap_start<R: Bench>(x: &mut Cx<'_, R>, speed: Option<i16>, kind: Option<u8>) {
    if let Some(speed) = speed {
        x.set16(Obj::VX, speed as u16);
    }
    if let Some(kind) = kind {
        x.set8(Enemy::STRIKE_KIND, kind);
    }
    x.set8(Enemy::STRIKE_POWER, 5);
    if x.u8(f::FLIP) == 0 {
        negate(x, Obj::VX);
    }
    x.set16(Obj::AX, 0);
    x.set16(Obj::VZ, 0x900);
    x.set16(Obj::AZ, -0x80i16 as u16);
    x.add8(f::SUB3, 2);
    strike(x, 2);
}

/// 0x67e0e: waits for the animation key.
fn wait_key<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
    if x.u8(f::KEY_FRAME) == 0 {
        return;
    }
    x.add8(f::SUB3, 2);
}

/// 0x67e22: climb, up to the top (`+0x18` negative).
fn rise<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xz();
    x.anim_step();
    if (x.u16(Obj::VZ) as i16) < 0 {
        x.add8(f::SUB3, 2);
        x.set16(Golem::ANIM_TIME, 1);
        x.set8(f::KEY_FRAME, 0);
    }
}

/// 0x67e46: descent down to the ground.
fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xz();
    x.anim_step();
    if x.u32(Obj::Z) as i32 > 0 {
        return;
    }
    x.set32(Obj::Z, 0);
    x.set16(Golem::ANIM_TIME, 1);
    x.add8(f::SUB3, 2);
}

/// 0x67e6a: end of the leap: choice of the next move at the end of the animation.
fn end_choose<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
    if (x.u8(Enemy::ANIM_END) as i8) < 0 {
        choose(x);
    }
}

/// 0x67f00: approach walk: advances as long as the key is not negative, then chooses when the timer has elapsed or the
/// Golem has moved too much.
fn approach<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
    if (x.u8(f::KEY_FRAME) as i8) >= 0 {
        let n = x.u16(Golem::TIMER).wrapping_sub(1);
        x.set16(Golem::TIMER, n);
        if n == 0 {
            return choose(x);
        }
        x.move_by_table_xy();
    }
    let d = x.u16(Golem::TIMER2).wrapping_sub(x.u16(Obj::X)).wrapping_add(0x40);
    if d >= 0x80 {
        choose(x);
    }
}

/// +6 = 14 and 16: leap (speed 0x480) then turn-around if the target is behind, and a strike (16).
fn hop_turn<R: Bench>(x: &mut Cx<'_, R>, step: u8) {
    match step {
        0 => next_anim(x),
        2 => wait(x),
        4 => leap_start(x, Some(0x480), None),
        6 => wait_key(x),
        8 => rise(x),
        10 => fall(x),
        12 => {
            x.anim_step();
            if (x.u8(Enemy::ANIM_END) as i8) < 0 {
                x.add8(f::SUB3, 2);
                if !target_behind(x) {
                    x.add8(f::SUB3, 2);
                }
            }
        }
        14 => turn(x),
        16 => {
            x.add8(f::SUB3, 2);
            let anim = if x.u8(f::SUB3) == 0x10 { 0 } else { rand_and(x, 1) };
            strike(x, anim);
        }
        18 if x.u8(f::SUB2) == 14 => until_end(x),
        18 => {
            if x.anim_step() {
                x.add8(f::SUB3, 2);
                strike(x, 1);
            }
        }
        20 => until_end(x),
        _ => {}
    }
}

/// +6 = 18 and 20 (and 0x12 set by profile `+0x2f`): freezes for a while (animation 8), then choice.
fn squat_step<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => next_anim(x),
        2 => wait(x),
        4 => {
            x.set16(0xa8, 0x100);
            // (the test is on step `+7`, not on `+6`)
            if x.u8(f::SUB3) != 0x12 {
                x.set8(0xa9, 1);
            }
            squat_begin(x);
        }
        6 => {
            if x.u8(f::KEY_FRAME) != 0 {
                let n = x.u16(Golem::TIMER).wrapping_sub(1);
                x.set16(Golem::TIMER, n);
                if (n as i16) >= 0 {
                    return;
                }
                x.add8(f::SUB3, 2);
                x.set16(Golem::ANIM_TIME, 1);
                let group = x.u8(Enemy::GROUP) as i8 as i32 as u32;
                let v = x.rom8(0x682e8u32.wrapping_add(group));
                x.set8(Enemy::AI_68, v);
            }
            x.anim_step();
        }
        8 => {
            if x.anim_step() {
                choose(x);
            }
        }
        _ => {}
    }
}

/// 0x6829e: crouches: next step, timer 0x3c, animation 8.
pub(super) fn squat_begin<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    x.set16(Golem::TIMER, 0x3c);
    x.set8(Enemy::AI_68, 9);
    mv(x, 8);
}
