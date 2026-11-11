//! Type 14, substates 4, 6 and 8: the moves. Substate 4 contains the throw, the jumps, the charge and rest, 6 the steps
//! in depth, jumps and teleportations, 8 the choice that follows a move.

use crate::actors::families::cx::Bench;
use super::stance::{approach_start, walk_off};
use crate::world::objects::Obj;
use super::*;
use crate::world::objects::Current;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match (x.u8(f::SUB), x.u8(f::SUB2)) {
        (4, 0) | (6, 0) => windup(x),
        (4, 2) | (6, 2) => turn_back(x),
        (4, 4) => throw(x),
        (4, 6) | (4, 8) => hop(x),
        (4, 10) => maybe_throw(x),
        (4, 12) => charge(x),
        (4, 14) => again(x, 0x26),
        (4, 16) => again(x, 0x1c),
        (4, 18) => rest(x),
        (4, 20) => walk_off(x),
        (6, 4) => done_attack(x),
        (6, 6) => depth_step(x),
        (6, 8) => jump_here(x, false),
        (6, 10) => jump_here(x, true),
        (6, 12) => teleport(x),
        (8, 0) => pause(x),
        (8, 2) => jump_in_place(x),
        (8, 4) => lunge(x),
        (8, 6) => pick_again(x),
        _ => {}
    }
}

/// 0x6cd5c: the next step starts animation 4.
fn next_and_start<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    mv(x, 4);
}

/// 0x6cd64: next step at the end of the animation.
fn next_when_done<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        x.add8(f::SUB3, 2);
    }
}

/// 0x6cd90: end of the animation, move finished.
fn finish<R: Bench>(x: &mut Cx<'_, R>) {
    if x.anim_step() {
        done_attack(x);
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
            let chosen = x.u8(Troll::CHOSEN);
            x.set8(f::SUB2, chosen);
        }
        return mv(x, 0);
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
        let chosen = x.u8(Troll::CHOSEN);
        x.set8(f::SUB2, chosen);
    }
}

/// +6 = 2: turn-around then move 0xc (walk).
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
        mv(x, 0);
    }
}

// --- lancer -------------------------------------------------------------------------------------------

/// 0x6ca46: starts the throwing move (+6 = 4).
fn throw_begin<R: Bench>(x: &mut Cx<'_, R>) {
    x.set16(f::SUB2, 0x400);
    throw(x);
}

/// +6 = 4: the Troll throws an object (as long as it has charges left `+0xab`), waits for it, then moves away.
fn throw<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 8);
            x.set8(Troll::TIMER, 0x30);
            if (x.u8(Troll::AMMO) as i8) > 0 {
                if let Some(obj) = lib::eshl_0a(x) {
                    x.set32(Troll::THROWN, obj.0);
                    x.add8(Troll::AMMO, 0xf0);
                    x.add8(f::SUB3, 0xfa);
                }
            }
            throw_anim(x, 0);
        }
        2 | 8 => {
            if x.u8(f::SUB3) == 8 {
                x.add8(Troll::NO_ZONE, 1);
            }
            let n = x.u8(Troll::TIMER).wrapping_sub(1);
            x.set8(Troll::TIMER, n);
            if (n as i8) >= 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set8(Troll::TIMER, 0x60);
            throw_anim(x, 4);
        }
        10 => {
            let n = x.u8(Troll::TIMER).wrapping_sub(1);
            x.set8(Troll::TIMER, n);
            if n == 0 {
                return throw_end(x);
            }
            throw_hold(x);
        }
        4 => throw_hold(x),
        6 => {
            x.anim_step();
            let n = x.u8(Troll::TIMER).wrapping_sub(1);
            x.set8(Troll::TIMER, n);
            if n != 0 {
                follow_up(x);
            }
        }
        12 => {
            x.anim_step();
            let n = x.u8(Troll::AFTER_THROW).wrapping_sub(1);
            x.set8(Troll::AFTER_THROW, n);
            if (n as i8) < 0 {
                set_state_long(x, 0x0202_0600);
            }
        }
        _ => {}
    }
}

/// 0x6cad2: the throw follows its animation; at the key, the thrown object moves on to the next step; waits until it
/// has gone (object state `+4` at 4 or more).
fn throw_hold<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
    let obj = Obj(x.u32(Troll::THROWN));
    if x.u8(f::KEY_FRAME) != 0 {
        x.set8(f::KEY_FRAME, 0);
        add8(x, obj, 5, 2);
    }
    if (obj.u8(x.b, Enemy::STATE) as i8) < 4 {
        return;
    }
    throw_end(x);
}

/// 0x6caf2: end of the throw.
fn throw_end<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    x.set16(Troll::AFTER_THROW, 0xb4);
    mv(x, 0);
}

/// +6 = 10: at random, back to walking (0x1c) or throw.
fn maybe_throw<R: Bench>(x: &mut Cx<'_, R>) {
    if rand_and(x, 1) == 0 {
        return again(x, 0x1c);
    }
    throw_begin(x);
}

/// +6 = 14, 16 (and 10 on a draw): back to standing and resume walking, `kind` = saved shot type.
fn again<R: Bench>(x: &mut Cx<'_, R>, kind: u8) {
    x.set8(Enemy::AI_POINT_SET, kind);
    set_state_long(x, STANCE);
    approach_start(x);
}

// --- sauts ---------------------------------------------------------------------------------------------

/// 0x6cdb6: side jump (horizontal speed 0x300, upward impulse).
fn jump_start<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    x.set32(Obj::VX, 0x0300_0000);
    let mut shift = 0x20i16;
    if x.u8(f::FLIP) != 0 {
        shift = -shift;
        negate(x, Obj::VX);
    }
    let at = x.u16(Obj::X).wrapping_add(shift as u16);
    x.set16(Obj::X, at);
    x.set16(Obj::VZ, 0);
    x.set16(Obj::AZ, -0x40i16 as u16);
    x.set16(Obj::Z, 0x50);
    mv(x, 3);
}

/// 0x6cdea: landing of the jump.
fn land<R: Bench>(x: &mut Cx<'_, R>) {
    x.move_xz();
    if (x.u16(Obj::Z) as i16) >= 0 {
        return;
    }
    x.add8(f::SUB3, 2);
    x.set32(Obj::Z, 0);
    mv(x, 5);
}

/// +6 = 6 and 8 (substate 4): side jump, then throw (6) or back to walking (8).
fn hop<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 | 6 => next_when_done(x),
        4 => jump_start(x),
        8 => land(x),
        10 => {
            if x.u8(f::SUB2) != 8 {
                return throw_begin(x);
            }
            set_state_long(x, STANCE);
            approach_start(x);
        }
        _ => {}
    }
}

/// +6 = 8 and 10 (substate 6): side jump; `then_throw` (10): then throw.
fn jump_here<R: Bench>(x: &mut Cx<'_, R>, then_throw: bool) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 | 6 => next_when_done(x),
        4 => jump_start(x),
        8 => land(x),
        10 => finish(x),
        12 if then_throw => {
            x.set8(f::SUB, 4);
            throw_begin(x);
        }
        _ => {}
    }
}

/// +6 = 12: charge toward the target for `+0xa2` frames.
fn charge<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(Troll::TIMER, 0x40);
        let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
        x.set8(Obj::SPEED_INDEX, angle);
        set_speed(x, 0x12);
        return mv(x, 1);
    }
    x.anim_step();
    let n = x.u8(Troll::TIMER).wrapping_sub(1);
    x.set8(Troll::TIMER, n);
    if (n as i8) < 0 {
        x.set16(f::SUB2, 0x400);
    }
}

/// +6 = 18: rest. The original leaves the rest from the second frame on (the timer-end test is inverted).
fn rest<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(Troll::TIMER, 0x1e);
        return mv(x, 0);
    }
    x.anim_step();
    let n = x.u8(Troll::TIMER).wrapping_sub(1);
    x.set8(Troll::TIMER, n);
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
        6 => finish(x),
        _ => {}
    }
}

/// Arrival positions (X then depth) of the teleportation, relative to the target.
const TELEPORT_X: &str = "troll/teleport_x";
const TELEPORT_Y: &str = "troll/teleport_y";

/// +6 = 12: the Troll reappears near its target.
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
            let r = rand_and(x, 7) * 2;
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
        x.set8(Troll::TIMER, 0x10);
        return mv(x, 0);
    }
    let n = x.u8(Troll::TIMER).wrapping_sub(1);
    x.set8(Troll::TIMER, n);
    if (n as i8) < 0 {
        set_state_long(x, STANCE);
    }
    x.anim_step();
}

/// +6 = 2: jump in place (without horizontal movement).
fn jump_in_place<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 | 6 => next_when_done(x),
        4 => {
            x.add8(f::SUB3, 2);
            x.set16(Obj::Z, 0x40);
            x.set16(Obj::VZ, 0);
            x.set16(Obj::AZ, -0x40i16 as u16);
            x.set32(Obj::VX, 0x0300_0000);
            mv(x, 3);
        }
        8 => land(x),
        10 => finish(x),
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
            x.set8(Troll::TIMER, 0x10);
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
            let n = x.u8(Troll::TIMER).wrapping_sub(1);
            x.set8(Troll::TIMER, n);
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
        x.u8(Troll::CHOSEN)
    } else if near_box(x) {
        pick_attack(x, NEAR_TABLE)
    } else if far_box(x) {
        pick_attack(x, FAR_TABLE)
    } else {
        return;
    };
    if b != x.u8(Troll::LOOK_PREV) {
        x.set8(Troll::LOOK_PREV, b);
        x.set8(Troll::COOLDOWN, 2);
        return;
    }
    let n = x.u8(Troll::COOLDOWN).wrapping_sub(1);
    x.set8(Troll::COOLDOWN, n);
    if (n as i8) < 0 {
        x.set8(Troll::COOLDOWN, 2);
    }
}
