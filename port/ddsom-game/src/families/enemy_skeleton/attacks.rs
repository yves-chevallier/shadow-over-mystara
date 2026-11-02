//! Type 1, melee attacks (`+5 = 4`). `+6`: 0 thrust, 2 high thrust, 4 combo, 6 double attack, 8 leap attack, 10 slide,
//! 12 big leap, 14 and 16 leap with turn-around, 18 and 20 guard (series 1).
//! `+7` is the step: steps 0 to 4 (depending on the attack) are shared (`ready`, `settle`, `face`).

use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if special_hit(x) {
        return;
    }
    match x.u8(f::SUB2) {
        0 => thrust(x, 5, 5, 0),
        2 => thrust(x, 6, 4, 1),
        4 => combo(x),
        6 => double_cut(x),
        8 => leap(x, Leap::Short),
        10 => slide(x),
        12 => leap(x, Leap::Long),
        14 => leap(x, Leap::Turning),
        16 => leap(x, Leap::TurningCombo),
        18 | 20 => super::leaps::guard(x),
        _ => {}
    }
    zone_and_terrain(x);
}

// --- common steps ----------------------------------------------------------------------------

/// 0x59b98: the skeleton gets ready (rest animation).
pub(super) fn ready<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    body(x, 0);
}

/// 0x59b82: waits for delay `+0x77` to elapse.
pub(super) fn settle<R: Bench>(x: &mut Cx<'_, R>) {
    let wait = x.u8(COOLDOWN);
    if wait != 0 {
        let n = wait.wrapping_sub(1);
        x.set8(COOLDOWN, n);
        if (n as i8) > 0 {
            return;
        }
    }
    x.add8(f::SUB3, 2);
}

/// 0x591d6: turns toward the target if it is behind (`+0xa6` = 2).
pub(super) fn face<R: Bench>(x: &mut Cx<'_, R>) {
    let side = x.u8(SIDE) as i8;
    if side < 2 {
        return x.add8(f::SUB3, 2);
    }
    if side == 2 {
        x.set8(SIDE, 3);
        return body(x, 1);
    }
    if (x.u8(ANIM_STATE) as i8) < 0 {
        x.add8(f::SUB3, 2);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        body(x, 0);
    } else {
        step(x);
    }
}

/// 0x59222: saves whether the target is behind (`+0xa6` = 2, otherwise 0); true if a turn-around is needed.
fn needs_turn_around<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let mut side = 0;
    if x.u16(f::TARGET) != 0 {
        let target = load_target(x);
        let tx = target.u16(x.b, Obj::X);
        let mine = x.u16(Obj::X);
        if mine != tx {
            let want = if mine < tx { 0x20 } else { 0 };
            if want != x.u8(f::FLIP) as u32 {
                side = 2;
            }
        }
    }
    x.set8(SIDE, side);
    side != 0
}

/// 0x58370: end of an attack — waits for the end of the animation, then chooses what follows.
fn finish<R: Bench>(x: &mut Cx<'_, R>) {
    if (x.u16(ANIM_STATE) as i16) >= 0 {
        step(x);
    } else {
        x.set8(AI_KIND, 0xff);
        end_attack(x);
    }
}

// --- coups simples ------------------------------------------------------------------------------

/// Thrust (`+6` = 0 or 2): draws whether to take the idea up again, otherwise prepares then strikes (type `kind`,
/// strength `power`, animation `anim`).
fn thrust<R: Bench>(x: &mut Cx<'_, R>, kind: u8, power: u8, anim: u32) {
    match x.u8(f::SUB3) {
        0 => {
            let r = (x.random() as u32) & 0xf;
            if 0xffc0u32 >> r & 1 != 0 {
                set_state_long(x, STANCE_ALT);
            } else {
                x.add8(f::SUB3, 2);
            }
        }
        2 => ready(x),
        4 => settle(x),
        6 => face(x),
        8 => {
            x.add8(f::SUB3, 2);
            x.set8(STRIKE_KIND, kind);
            x.set8(STRIKE_POWER, power);
            strike(x, anim);
        }
        10 => finish(x),
        _ => {}
    }
}

/// Combo (`+6` = 4): shield strike, then, at random, a second strike; series 1 goes to guard.
fn combo<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => face(x),
        2 => ready(x),
        4 => settle(x),
        6 => {
            if x.u8(SERIES) != 0 {
                x.set16(f::SUB2, 0x1200);
                return super::leaps::guard(x);
            }
            x.set8(STRIKE_KIND, 5);
            x.set8(AI_KIND, 3);
            x.set8(STRIKE_POWER, 4);
            let state = x.u32(f::STATE);
            x.set32(Enemy::RESUME_STATE, state);
            x.add8(f::SUB3, 2);
            strike(x, 3);
        }
        8 => {
            if !step(x) {
                return;
            }
            if (x.random() as u32) & 1 == 0 {
                end_attack(x);
            } else {
                x.add8(f::SUB3, 2);
                strike(x, 0);
            }
        }
        10 => finish(x),
        _ => {}
    }
}

/// Double attack (`+6` = 6): a random strike among two animations, then the next one.
fn double_cut<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => face(x),
        2 => ready(x),
        4 => settle(x),
        6 => {
            x.add8(f::SUB3, 2);
            let state = x.u32(f::STATE);
            x.set32(Enemy::RESUME_STATE, state);
            x.set8(STRIKE_KIND, 5);
            x.set8(STRIKE_POWER, 2);
            x.set8(AI_KIND, 3);
            let n = rand_and(x, 1);
            let kind = x.u8(STRIKE_KIND).wrapping_add(n as u8);
            x.set8(STRIKE_KIND, kind);
            strike(x, n);
        }
        8 => {
            if step(x) {
                x.add8(f::SUB3, 2);
                strike(x, 1);
            }
        }
        10 => finish(x),
        _ => {}
    }
}

// --- bonds ----------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub(super) enum Leap {
    /// Bond court : coup de type 7.
    Short,
    /// Big leap: type 8, faster.
    Long,
    /// Leap with turn-around on landing.
    Turning,
    /// Same, followed by a second hit.
    TurningCombo,
}

fn leap<R: Bench>(x: &mut Cx<'_, R>, kind: Leap) {
    match x.u8(f::SUB3) {
        0 => ready(x),
        2 => settle(x),
        4 => jump_up(x, kind),
        6 => land_wait(x),
        8 => rise(x),
        10 => fall(x),
        12 => match kind {
            Leap::Short | Leap::Long => {
                step(x);
                if (x.u8(ANIM_STATE) as i8) < 0 {
                    end_attack(x);
                }
            }
            Leap::Turning | Leap::TurningCombo => {
                step(x);
                if (x.u8(ANIM_STATE) as i8) < 0 {
                    x.add8(f::SUB3, 2);
                    if !needs_turn_around(x) {
                        x.add8(f::SUB3, 2);
                    }
                }
            }
        },
        14 => face(x),
        16 => {
            x.add8(f::SUB3, 2);
            let n = if x.u8(f::SUB3) == 0x10 {
                0
            } else {
                rand_and(x, 1)
            };
            strike(x, n);
        }
        18 => match kind {
            Leap::TurningCombo => {
                if step(x) {
                    x.add8(f::SUB3, 2);
                    strike(x, 1);
                }
            }
            _ => finish(x),
        },
        20 => finish(x),
        _ => {}
    }
}

/// 0x584fe: waits for the key frame of the jump.
pub(super) fn land_wait<R: Bench>(x: &mut Cx<'_, R>) {
    step(x);
    if x.u8(f::KEY_FRAME) != 0 {
        x.add8(f::SUB3, 2);
    }
}

/// 0x58512: climb of the leap (movement in X and in height).
pub(super) fn rise<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xz();
    step(x);
    if (x.u16(Obj::VZ) as i16) < 0 {
        x.add8(f::SUB3, 2);
        x.set16(ANIM_COUNT, 1);
        x.set8(f::KEY_FRAME, 0);
    }
}

/// 0x58536: descent down to the ground.
pub(super) fn fall<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xz();
    step(x);
    if (x.u32(Obj::Z) as i32) <= 0 {
        x.set32(Obj::Z, 0);
        x.set16(ANIM_COUNT, 1);
        x.add8(f::SUB3, 2);
    }
}

/// Start of the leap: horizontal speed according to the leap type and the orientation, then take-off.
fn jump_up<R: Bench>(x: &mut Cx<'_, R>, kind: Leap) {
    match kind {
        Leap::Short => {
            x.set16(Obj::VX, 0x180);
            x.set8(STRIKE_KIND, 7);
        }
        Leap::Long => {
            x.set8(STRIKE_KIND, 8);
            x.set16(Obj::VX, 0x480);
        }
        Leap::Turning | Leap::TurningCombo => x.set16(Obj::VX, 0x480),
    }
    x.set8(STRIKE_POWER, 5);
    if x.u8(f::FLIP) == 0 {
        negate(x, Obj::VX);
    }
    x.set16(Obj::AX, 0);
    x.set16(Obj::VZ, 0x900);
    x.set16(Obj::AZ, -0x80i16 as u16);
    x.add8(f::SUB3, 2);
    strike(x, 2);
}

/// Slide (`+6` = 10): the skeleton rushes at the target while shifting its animation.
fn slide<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => ready(x),
        2 => settle(x),
        4 => begin_slide(x),
        6 => slide_move(x),
        _ => {}
    }
}

/// 0x58582: starts animation 2 bringing it back to the frame it had, saves the position.
fn begin_slide<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    x.set16(TIMER, 0x2a);
    restart_slide(x);
}

/// 0x5858c: animation 2 replayed until the same frame is found again (at most eight steps).
pub(super) fn restart_slide<R: Bench>(x: &mut Cx<'_, R>) {
    let (count, state) = (x.u16(ANIM_COUNT), x.u16(ANIM_STATE));
    // D2 (low word) = the sought frame, D3 = step counter: leftovers re-read afterwards (DDSOM_POISON:
    // skeleton_scene13, boss17_scene13).
    *x.c.d2() = *x.c.d2() & 0xffff_0000 | state as u32;
    x.anim_start(SET_BODY.0, 2);
    *x.c.d3() = 7;
    loop {
        if x.u16(ANIM_STATE) == state {
            x.set16(ANIM_COUNT, count);
            break;
        }
        x.set16(ANIM_STATE, 1);
        step(x);
        let n = (*x.c.d3() as u16).wrapping_sub(1);
        *x.c.d3() = *x.c.d3() & 0xffff_0000 | n as u32;
        if n == 0xffff {
            break;
        }
    }
    finish_restart(x);
}

fn finish_restart<R: Bench>(x: &mut Cx<'_, R>) {
    x.set32(MOVE_TABLE, 0x1c7336);
    let angle = if x.u8(f::FLIP) != 0 { 8 + 0x10 } else { 8 };
    x.set8(Obj::SPEED_INDEX, angle);
    let pos = x.u16(Obj::X);
    x.set16(SAVED_POS, pos);
}

fn slide_move<R: Bench>(x: &mut Cx<'_, R>) {
    step(x);
    if (x.u8(f::KEY_FRAME) as i8) >= 0 {
        let t = x.u16(TIMER).wrapping_sub(1);
        x.set16(TIMER, t);
        if t == 0 {
            return end_attack(x);
        }
        move_xy(x);
    }
    let d = x.u16(SAVED_POS).wrapping_sub(x.u16(Obj::X)).wrapping_add(0x40);
    if d >= 0x80 {
        end_attack(x);
    }
}
