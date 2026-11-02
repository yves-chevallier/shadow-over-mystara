//! Type 13, substates 4, 6 and 8: the moves. Substate 4 contains the axe strikes, the jumps onto the target and rest, 6
//! the steps, jumps and teleportations, 8 the choice that follows a move.

use crate::families::cx::Bench;
use super::stance::{approach_restart, approach_start};
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

/// The four jumps onto the target of substate 4 (`+6` = 12, 16, 18 and 20): the last three land higher and further, the
/// last two still chain another strike (steps `+7` = 12 and up).
#[derive(Clone, Copy, PartialEq)]
enum Leap {
    Low,
    High,
    HighThenStrike,
    HighThenSeries,
}

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match (x.u8(f::SUB), x.u8(f::SUB2)) {
        (4, 0) | (6, 0) => windup(x),
        (4, 2) | (6, 2) => turn_back(x),
        (4, 4) => swing(x, 0, 0x1b),
        (4, 6) => swing(x, 2, 0x1c),
        (4, 8) => combo_a(x),
        (4, 10) => combo_b(x),
        (4, 12) => leap_on_target(x, Leap::Low),
        (4, 14) | (6, 8) => hop(x),
        (4, 16) => leap_on_target(x, Leap::High),
        (4, 18) => leap_on_target(x, Leap::HighThenStrike),
        (4, 20) => leap_on_target(x, Leap::HighThenSeries),
        (4, 22) => {
            set_state_long(x, STANCE);
            approach_start(x);
        }
        (4, 24) => {
            set_state_long(x, STANCE);
            x.set8(Enemy::AI_POINT_SET, 0x1c);
            approach_restart(x);
        }
        (6, 4) => done_attack(x),
        (6, 6) => depth_step(x),
        (6, 10) => jump_up(x),
        (6, 12) => teleport(x),
        (8, 0) => pause(x),
        (8, 2) => back_step(x),
        (8, 4) => lunge(x),
        (8, 6) => pick_again(x),
        _ => {}
    }
}

/// 0x6b742: the next step starts animation 4 (with sound).
fn next_and_start<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    wind_up(x);
}

/// 0x6b74a: next step at the end of the animation.
fn next_when_done<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        x.add8(f::SUB3, 2);
    }
}

/// 0x6b776: end of the animation, move finished.
fn finish<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        done_attack(x);
    }
}

/// 0x6b4c6 : fin de l'animation, geste de repli.
fn finish_follow<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        follow_up(x);
    }
}

// --- move 0: taunt, then chosen move --------------------------------------------------------

fn windup<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(Enemy::VULNERABLE, 1);
        lib::pick_byte(x);
        if x.u8(Enemy::AI_BYTE) == 0 {
            x.set8(f::SUB3, 0);
            let chosen = x.u8(Gnoll::CHOSEN);
            x.set8(f::SUB2, chosen);
        }
        let weapon = x.u8(Gnoll::WEAPON) as u32;
        return mv(x, weapon);
    }
    x.anim_step();
    let n = x.u8(Enemy::AI_BYTE).wrapping_sub(1);
    x.set8(Enemy::AI_BYTE, n);
    if (n as i8) >= 0 {
        return;
    }
    x.set8(f::SUB3, 0);
    if faces_wrong(x) {
        x.add8(f::SUB2, 2);
    } else {
        let chosen = x.u8(Gnoll::CHOSEN);
        x.set8(f::SUB2, chosen);
    }
}

/// +6 = 2: turn-around then move 0xc.
fn turn_back<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(Enemy::VULNERABLE, 1);
        return mv(x, 2);
    }
    if x.anim_step() {
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        x.set16(f::SUB2, 0xc00);
        let weapon = x.u8(Gnoll::WEAPON) as u32;
        mv(x, weapon);
    }
}

// --- coups de hache -----------------------------------------------------------------------------------

/// +6 = 4 and 6: an axe strike (animation `anim`, type `kind`), or, one time in ten to sixteen, back to walking.
fn swing<R: Bench>(x: &mut Cx<'_, R>, anim: u32, kind: u8) {
    if x.u8(f::SUB3) != 0 {
        if x.anim_step() {
            follow_up(x);
        }
        return;
    }
    let r = rand_and(x, 0xf);
    // `btst D0,D1` with D1 = 0xffc0: draws from 6 to 15 give up the strike
    if (0xffc0u32 >> r) & 1 != 0 {
        x.set8(Enemy::AI_POINT_SET, 0x1b);
        set_state_long(x, STANCE);
        return approach_restart(x);
    }
    x.add8(f::SUB3, 2);
    strike_with(x, anim, kind, 5);
}

/// +6 = 8: a strike, then a low strike repeated `+0xa2` times.
fn combo_a<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set8(Enemy::AI_SHOT, 3);
            let r = rand_and(x, 1);
            x.set8(Gnoll::TIMER, r as u8);
            strike_with(x, 0, 0x1b, 5);
        }
        2 => {
            if x.anim_step() {
                x.add8(f::SUB3, 2);
                strike_with(x, 3, 0x1b, 4);
            }
        }
        4 => {
            if !x.anim_step() {
                return;
            }
            let n = x.u8(Gnoll::TIMER).wrapping_sub(1);
            x.set8(Gnoll::TIMER, n);
            if (n as i8) < 0 {
                return follow_up(x);
            }
            strike_with(x, 0, 0x1b, 4);
        }
        _ => {}
    }
}

/// +6 = 10: a strike drawn at random between two, then a second one.
fn combo_b<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(Enemy::AI_SHOT, 3);
        x.set8(Gnoll::TIMER, 1);
        if rand_and(x, 1) != 0 {
            strike_with(x, 2, 0x1c, 5);
        } else {
            strike_with(x, 0, 0x1b, 5);
        }
        return;
    }
    if !x.anim_step() {
        return;
    }
    let n = x.u8(Gnoll::TIMER).wrapping_sub(1);
    x.set8(Gnoll::TIMER, n);
    if (n as i8) < 0 {
        return follow_up(x);
    }
    strike_with(x, 2, 0x1c, 4);
}

// --- jumps onto the target -------------------------------------------------------------------------------

/// 0x6b3bc / 0x6b4fa: the Gnoll places itself on the target (position copied), at height `height`, and leaps `shift`
/// forward.
fn leap_start<R: Bench>(x: &mut Cx<'_, R>, height: u32, shift: i16) {
    x.add8(f::SUB3, 2);
    let target = load_target(x);
    let at = target.u32(x.b, Obj::X);
    x.set32(Obj::X, at);
    let depth = target.u32(x.b, Obj::Y);
    x.set32(Obj::Y, depth);
    x.set32(Obj::Z, height << 16);
    x.set16(Obj::VZ, 0);
    x.set16(Obj::AZ, -0x40i16 as u16);
    let (mut d, mut v) = (shift, -0x400i16);
    if x.u8(f::FLIP) != 0 {
        d = -d;
        v = -v;
    }
    let pos = x.u16(Obj::X).wrapping_add(d as u16);
    x.set16(Obj::X, pos);
    x.set16(Obj::VX, v as u16);
    x.set16(Obj::AX, 0);
    x.set_bit(Enemy::DISPLAY, 1, false);
    mv(x, 3);
}

/// 0x6b8a8: landing of the jump.
fn land<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xz();
    if (x.u16(Obj::Z) as i16) > 0 {
        return;
    }
    x.add8(f::SUB3, 2);
    x.set32(Obj::Z, 0);
    mv(x, 5);
}

fn leap_on_target<R: Bench>(x: &mut Cx<'_, R>, kind: Leap) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 => {
            if x.anim_step() {
                x.add8(f::SUB3, 2);
                x.set_bit(Enemy::DISPLAY, 1, true);
                x.set8(0x3b, 0);
                x.set8(Gnoll::TIMER, 0x1e);
            }
        }
        4 => {
            let n = x.u8(Gnoll::TIMER).wrapping_sub(1);
            x.set8(Gnoll::TIMER, n);
            if n != 0 {
                return;
            }
            if kind == Leap::Low {
                leap_start(x, 0x30, 0x40);
            } else {
                leap_start(x, 0x60, 0x60);
            }
        }
        6 => {
            if x.anim_step() {
                x.add8(f::SUB3, 2);
                strike_with(x, 1, 0x1d, 5);
            }
        }
        8 => {
            if (x.u16(Obj::Z) as i16) <= 0x30 {
                x.anim_step();
            }
            land(x);
        }
        10 => finish_follow(x),
        12 if kind == Leap::HighThenStrike => {
            x.add8(f::SUB3, 2);
            x.set8(Enemy::STRIKE_KIND, 0x1b);
            x.set8(Enemy::STRIKE_POWER, 5);
            let anim = if rand_and(x, 1) != 0 { 2 } else { 0 };
            strike(x, anim);
        }
        12 if kind == Leap::HighThenSeries => {
            x.add8(f::SUB3, 4);
            strike_with(x, 0, 0x1b, 5);
        }
        14 if kind == Leap::HighThenStrike => finish_follow(x),
        14 if kind == Leap::HighThenSeries => {
            if x.anim_step() {
                x.add8(f::SUB3, 4);
                strike_with(x, 2, 0x1b, 5);
            }
        }
        16 if kind == Leap::HighThenSeries => finish_follow(x),
        _ => {}
    }
}

/// +6 = 14 (substate 4) and 8 (substate 6): jump in place.
fn hop<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 | 6 => next_when_done(x),
        4 => {
            x.add8(f::SUB3, 2);
            mv(x, 3);
        }
        8 => finish(x),
        _ => {}
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
        6 => finish(x),
        _ => {}
    }
}

/// +6 = 10 : bond en l'air, coup plongeant.
fn jump_up<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 | 6 => next_when_done(x),
        4 => {
            x.add8(f::SUB3, 2);
            let h = x.u16(Obj::Z).wrapping_add(0x40);
            x.set16(Obj::Z, h);
            mv(x, 3);
        }
        8 => {
            x.add8(f::SUB3, 2);
            x.set8(Enemy::STRIKE_KIND, 0x1d);
            x.set8(Enemy::STRIKE_POWER, 5);
            x.set16(Obj::VZ, 0);
            x.set16(Obj::AX, 0);
            x.set16(Obj::AZ, -0x40i16 as u16);
            let v = if x.u8(f::FLIP) != 0 { 0x400 } else { -0x400i16 as u16 };
            x.set16(Obj::VX, v);
            strike(x, 1);
        }
        10 => {
            x.anim_step();
            land(x);
        }
        12 => finish(x),
        _ => {}
    }
}

/// Arrival positions (X then depth) of the teleportation, relative to the target.
const TELEPORT_X: &str = "gnoll/teleport_x";
const TELEPORT_Y: &str = "gnoll/teleport_y";

/// +6 = 12: the Gnoll reappears near its target.
fn teleport<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 | 6 => next_when_done(x),
        4 => {
            x.add8(f::SUB3, 2);
            let r = rand_and(x, 3) * 2;
            let target = load_target(x);
            let off = tables::table(TELEPORT_X).at16(x.b, r);
            let at = off.wrapping_add(target.u16(x.b, Obj::X));
            x.set16(Obj::X, at);
            let r = rand_and(x, 3) * 2;
            let target = load_target(x);
            let off = tables::table(TELEPORT_Y).at16(x.b, r);
            let at = off.wrapping_add(target.u16(x.b, Obj::Y));
            x.set16(Obj::Y, at);
            x.set8(f::FLIP, 0x20);
            let dx = x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X));
            if (dx as i16) < 0 {
                x.set8(f::FLIP, 0);
            }
            mv(x, 3);
        }
        8 => finish(x),
        _ => {}
    }
}

// --- substate 8 --------------------------------------------------------------------------------------

/// +6 = 0 : courte pause debout.
fn pause<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(Enemy::VULNERABLE, 1);
        x.set8(Gnoll::TIMER, 0x10);
        return mv(x, 0);
    }
    let n = x.u8(Gnoll::TIMER).wrapping_sub(1);
    x.set8(Gnoll::TIMER, n);
    if (n as i8) < 0 {
        set_state_long(x, STANCE);
    }
    x.anim_step();
}

/// +6 = 2: step backward (steps 8 and up are a jump in place, never reached from step 6).
fn back_step<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 | 8 => next_and_start(x),
        2 => {
            if x.anim_step() {
                x.add8(f::SUB3, 2);
                x.set8(Gnoll::TIMER, 0x1e);
                x.set_bit(Enemy::DISPLAY, 1, true);
                x.set8(0x3b, 0);
            }
        }
        4 => {
            let n = x.u8(Gnoll::TIMER).wrapping_sub(1);
            x.set8(Gnoll::TIMER, n);
            if n != 0 {
                return;
            }
            let d: i16 = if x.u8(f::FLIP) != 0 { -0x30 } else { 0x30 };
            let at = x.u16(Obj::X).wrapping_add(d as u16);
            x.set16(Obj::X, at);
            x.set_bit(Enemy::DISPLAY, 1, false);
            x.add8(f::SUB3, 2);
            mv(x, 3);
        }
        6 | 18 => finish(x),
        10 | 14 => next_when_done(x),
        12 => {
            x.add8(f::SUB3, 2);
            x.set16(Obj::Z, 0x40);
            x.set16(Obj::VZ, 0);
            x.set16(Obj::AZ, -0x40i16 as u16);
            x.set32(Obj::VX, 0x0300_0000);
            if x.u8(f::FLIP) != 0 {
                negate(x, Obj::VX);
            }
            mv(x, 3);
        }
        16 => land(x),
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
            x.set8(Gnoll::TIMER, 0x10);
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
        8 => land(x),
        10 => finish(x),
        12 => {
            let n = x.u8(Gnoll::TIMER).wrapping_sub(1);
            x.set8(Gnoll::TIMER, n);
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
    x.set8(Enemy::VULNERABLE, 1);
    set_state_long(x, IDLE);
    let b = if lib::target_in_box(x, -0x20, 0x20, 0xc) {
        set_state_long(x, 0x0204_0000);
        rand_and(x, 2);
        x.set16(f::SUB2, 0);
        x.u8(Gnoll::CHOSEN)
    } else if near_box(x) {
        pick_attack(x, NEAR_TABLE)
    } else if far_box(x) {
        pick_attack(x, FAR_TABLE)
    } else {
        return;
    };
    if b != x.u8(Gnoll::LOOK_PREV) {
        x.set8(Gnoll::LOOK_PREV, b);
        x.set8(Gnoll::COOLDOWN, 2);
        return;
    }
    let n = x.u8(Gnoll::COOLDOWN).wrapping_sub(1);
    x.set8(Gnoll::COOLDOWN, n);
    if (n as i8) < 0 {
        x.set8(Gnoll::COOLDOWN, 2);
    }
}
