//! Type 18, substates 4 (attacks) and 6, 8, 10 (rest).
//!
//! Substate 4, `+6`: 0 roar, 2 and 4 the ogre flies up and throws arms (objects of type 0x2a, variant 1), 6 leap and
//! fall onto the target, 8 arm variant 2: the ogre rises following the arm, then comes back down.

use crate::families::cx::Bench;
use super::*;
use crate::objects::pools;
use crate::objects::Current;
use crate::objects::Obj;

/// Table of the ogre's climbs following its arm, indexed by the animation key (longs).
const RISE: u32 = 0x713e8;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => roar(x),
        2 => fly_up(x),
        4 => fly_up_b(x),
        6 => crash(x),
        8 => arm_swing(x),
        _ => {}
    }
}

/// Plus personne en vie → retraite ; sinon `then` (0x71094 : debout, 0x71cd0 : nouveau geste).
fn end_with<R: Bench>(x: &mut Cx<'_, R>, stand: bool) {
    if !lib::players_alive(x) {
        return go::retire(x);
    }
    if stand {
        go::stand(x);
    } else {
        go::wander(x);
    }
}

// --- +6 = 0 : rugissement -----------------------------------------------------------------------------

fn roar<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            lib::sound(x, crate::sound::CMD_30C_40C);
            look(x, 0x10);
            act(x, 0);
        }
        2 => {
            if lib::step(x) {
                end_with(x, false);
            }
        }
        _ => {}
    }
}

// --- +6 = 2 and 4: flight and arms ----------------------------------------------------------------------

/// 0x70eca: creates the arm (type 0x2a, variant `variant`) linked to the ogre; false if the allocation fails.
fn throw_arm<R: Bench>(x: &mut Cx<'_, R>, variant: Option<u8>) -> bool {
    let Some(arm) = x.alloc(&pools::ESHL) else { return false };
    x.set16(ARM, arm.0 as u16);
    arm.set_u8(x.b, 0, 1);
    arm.set_u8(x.b, 2, 0x2a);
    if let Some(v) = variant {
        arm.set_u8(x.b, 3, v);
    }
    arm.set_u16(x.b, f::CREATOR, x.o.0 as u16);
    let target = x.u16(f::TARGET);
    arm.set_u16(x.b, f::TARGET, target);
    let flip = x.u8(f::FLIP);
    arm.set_u8(x.b, f::FLIP, flip);
    true
}

/// 0x70ef8: the ogre rises.
fn take_off<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    x.set16(Obj::VZ, 0x400);
    x.set16(Obj::AZ, 0);
    act(x, 1);
}

/// `+6` = 2: the ogre throws an arm, climbs off the screen, comes back down above the player and lands.
fn fly_up<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            if !pick_target(x) {
                return go::retire(x);
            }
            look(x, 0x11);
            if !throw_arm(x, Some(1)) {
                return end_with(x, true);
            }
            take_off(x);
        }
        _ => fly_common(x),
    }
}

/// `+6` = 4: like 2, after an arrival leap from the top (0x710ba, 0x710d8..).
fn fly_up_b<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            if !pick_target(x) {
                return go::retire(x);
            }
            lib::sound(x, crate::sound::CMD_30C_40C);
            look(x, 0x12);
            if !throw_arm(x, Some(1)) {
                return end_with(x, true);
            }
            take_off(x);
        }
        2 => fly_loop(x),
        4 => {
            if !count_down(x, TIMER) {
                x.add8(f::SUB3, 2);
                x.set_bit(1, 1, false);
                let t = x.linked(f::TARGET);
                let tz = t.u16(x.b, Obj::Y);
                x.set16(Obj::Y, tz);
                x.set16(Obj::Z, 0x70);
                let mut d0 = 0u16;
                let mut vx = 0x700i16;
                if x.u8(f::FLIP) == 0 {
                    d0 = 0x180;
                    vx = -vx;
                }
                let px = d0.wrapping_add(var::CAMERA1_X.get(x.b));
                x.set16(Obj::X, px);
                x.set16(Obj::VX, vx as u16);
                x.set16(Obj::VZ, -0x680i16 as u16);
                x.set16(Obj::AX, 0);
                x.set16(Obj::AZ, 0);
                lib::sound(x, crate::sound::CMD_30C_40C);
                x.set8(INVULNERABLE, 0);
                act(x, 5);
            }
        }
        6 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set16(TIMER, 0x1e);
            x.set16(TIMER2, 0);
            x.set16(Obj::Z, 0);
            let vy = if x.u8(f::FLIP) != 0 { -0x38i16 } else { 0x38 };
            x.set16(Obj::AX, vy as u16);
            lib::shake_54(x, 0x1e);
            lib::sound(x, crate::sound::CMD_328_428);
            lib::sound(x, crate::sound::CMD_329_429);
            act(x, 6);
        }
        8 => {
            if !count_down(x, TIMER2) {
                x.set16(TIMER2, 4);
                spark(x, 3);
            }
            if count_down(x, TIMER) {
                x.move_x();
                lib::step(x);
            } else {
                go::stand(x);
            }
        }
        _ => {}
    }
}

/// Steps 2 to 8 shared by `+6` = 2 and 4: the flight (70f14), the landing (70fd6, 7102e, 7107e).
fn fly_common<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        2 => fly_loop(x),
        4 => fly_down_from_above(x),
        6 => land_heavily(x),
        8 => {
            if (x.u16(TIMER) as i16) > 0 {
                x.add16(TIMER, 0xffff);
                return;
            }
            if x.anim_step() {
                end_with(x, true);
            }
        }
        _ => {}
    }
}

/// 0x70f14: climbing flight with the arms; over when the height exceeds the top of the screen.
fn fly_loop<R: Bench>(x: &mut Cx<'_, R>) {
    let top = var::CAMERA1_Y.get(x.b).wrapping_add(0xf0);
    let here = x.u16(Obj::Y).wrapping_add(x.u16(Obj::Z));
    if (here as i16) >= (top as i16) {
        x.add8(f::SUB3, 2);
        x.set16(TIMER, 0x28);
        x.set8(INVULNERABLE, 1);
        x.set_bit(1, 1, true);
        let arm = x.linked(ARM);
        arm.set_u8(x.b, 0x58, 1);
        return;
    }
    lib::push_players(x);
    lib::step(x);
    let key = x.u8(f::KEY_FRAME) & 0x7f;
    if key < 3 {
        flash_key(x);
        let v = x.u16(Obj::VZ).wrapping_add(-0x28i16 as u16);
        x.set16(Obj::VZ, v);
        let d = ((v as u32) << 16) as i32 >> 8;
        x.add32(Obj::Z, d as u32);
        let v = x.u16(Obj::VZ);
        x.set16(Obj::AZ, v);
    } else {
        flash_key(x);
        let v = x.u16(Obj::AZ).wrapping_add(-0x30i16 as u16);
        x.set16(Obj::AZ, v);
        let d = ((v as u32) << 16) as i32 >> 8;
        x.add32(Obj::Z, d as u32);
        x.set16(Obj::VZ, 0x400);
    }
}

/// 0x70fd6: the ogre disappears off the top, then reappears above the player and dives.
fn fly_down_from_above<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.u16(TIMER).wrapping_sub(1);
    x.set16(TIMER, n);
    if (n as i16) <= 0 {
        x.add8(f::SUB3, 2);
        x.set8(INVULNERABLE, 0);
        return act(x, 2);
    }
    if n != 0xa {
        return;
    }
    x.set_bit(1, 1, false);
    let t = x.linked(f::TARGET);
    let (tx, tz) = (t.u16(x.b, Obj::X), t.u16(x.b, Obj::Y));
    x.set16(Obj::X, tx);
    x.set16(Obj::Y, tz);
    let h = var::CAMERA1_Y.get(x.b).wrapping_add(0xf0).wrapping_sub(tz);
    x.set16(Obj::Z, h);
    x.set16(Obj::VZ, -0x500i16 as u16);
    x.set16(Obj::AZ, -0x10i16 as u16);
}

/// 0x7102e: free fall to the ground, shake and impact animation.
fn land_heavily<R: Bench>(x: &mut Cx<'_, R>) {
    let g = x.u16(Obj::AZ);
    x.add16(Obj::VZ, g);
    let d = ((x.u16(Obj::VZ) as u32) << 16) as i32 >> 8;
    x.add32(Obj::Z, d as u32);
    if (x.u16(Obj::Z) as i16) > 0 {
        return;
    }
    x.add8(f::SUB3, 2);
    x.set16(TIMER, 0x1e);
    x.set16(Obj::Z, 0);
    lib::shake_58(x, 0x1e);
    spark(x, 5);
    lib::sound(x, crate::sound::CMD_328_428_B);
    act(x, 3);
}

// --- +6 = 6: leap and fall onto the target ---------------------------------------------------------------------

fn crash<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set16(Obj::AX, 0);
            let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            x.set8(Obj::SPEED_INDEX, angle);
            set_speed(x, 0x40);
            // vertical speed: the number of frames to cover the distance, in steps of 0x10
            let target = load_target(x);
            let mut dist = x.u32(Obj::X).wrapping_sub(target.u32(x.b, Obj::X)) as i32;
            if dist < 0 {
                dist = dist.wrapping_neg();
            }
            let mut speed = ((x.u16(Obj::VX) as u32) << 16) as i32 >> 8;
            if speed < 0 {
                speed = speed.wrapping_neg();
            }
            let mut d1 = 0x200u16;
            let mut guard = 0;
            loop {
                dist = dist.wrapping_sub(speed);
                if dist <= 0 || guard > 0x10000 {
                    break;
                }
                d1 = d1.wrapping_add(0x10);
                guard += 1;
            }
            x.set16(Obj::VZ, d1);
            x.set16(Obj::AZ, -0x38i16 as u16);
            lib::sound(x, crate::sound::CMD_30C_40C);
            look(x, 0x13);
            act(x, 7);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) != 0 {
                return;
            }
            x.move_xz();
            if (x.u16(Obj::VZ) as i16) <= 0x100 {
                x.add8(f::SUB3, 2);
                act(x, 8);
            }
        }
        4 => {
            x.move_xz();
            if (x.u16(Obj::VZ) as i16) <= -0x100 {
                x.add8(f::SUB3, 2);
                act(x, 9);
            }
        }
        6 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set16(Obj::Z, 0);
            x.set16(TIMER, 0x14);
            x.set16(TIMER2, 0);
            x.set8(INVULNERABLE, 0);
            let vy = if x.u8(f::FLIP) != 0 { -0x5ci16 } else { 0x5c };
            x.set16(Obj::AX, vy as u16);
            lib::shake_54(x, 0x14);
            lib::sound(x, crate::sound::CMD_328_428);
            lib::sound(x, crate::sound::CMD_329_429);
            act(x, 0xa);
        }
        8 => {
            if !count_down(x, TIMER2) {
                x.set16(TIMER2, 4);
                spark(x, 3);
            }
            if count_down(x, TIMER) {
                x.move_x();
                lib::step(x);
            } else {
                end_with(x, false);
            }
        }
        _ => {}
    }
}

// --- +6 = 8 : bras variante 2 ---------------------------------------------------------------------------

fn arm_swing<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => {
            if !pick_target(x) {
                return go::retire(x);
            }
            look(x, 0x14);
            if !throw_arm(x, Some(2)) {
                return end_with(x, false);
            }
            take_off(x);
        }
        2 => {
            lib::push_players(x);
            flash_key(x);
            if (x.u16(Obj::Z) as i16) >= 0x20 && x.u8(f::KEY_FRAME) == 4 {
                x.add8(f::SUB3, 2);
                x.set8(INVULNERABLE, 0);
                x.set16(TIMER, 0xa);
                x.set16(TIMER2, 0x48);
            } else {
                let idx = (x.u8(f::KEY_FRAME) as u32) << 2;
                let d = x.prog32(RISE + idx);
                x.add32(Obj::Z, d);
            }
            lib::step(x);
        }
        4 => {
            lib::step(x);
            bob(x);
            lib::push_players(x);
            flash_key(x);
            if !count_down(x, TIMER) {
                x.add8(f::SUB3, 2);
                let arm = x.linked(ARM);
                arm.set_u8(x.b, 0x58, 1);
            }
        }
        6 => {
            lib::step(x);
            bob(x);
            lib::push_players(x);
            flash_key(x);
            if x.u8(ANIM_FLAGS) != 0 {
                x.add8(f::SUB3, 2);
                let arm = x.linked(ARM);
                arm.set_u8(x.b, 0x59, 1);
            }
        }
        8 => {
            flash_key(x);
            lib::step(x);
            bob(x);
            lib::push_players(x);
            let arm = x.linked(ARM);
            if arm.u8(x.b, 0) == 0 || (arm.u8(x.b, 0x5a) != 0 && x.u8(f::KEY_FRAME) == 4) {
                x.add8(f::SUB3, 2);
            }
        }
        10 => {
            flash_key(x);
            let h = x.u32(Obj::Z).wrapping_sub(0x18000);
            x.set32(Obj::Z, h);
            if (h as i32) > 0 {
                lib::step(x);
                return;
            }
            x.add8(f::SUB3, 2);
            x.set16(Obj::Z, 0);
            x.set16(TIMER, 0x10);
            mv(x, 0);
        }
        12 => {
            if count_down(x, TIMER) {
                lib::step(x);
            } else {
                end_with(x, false);
            }
        }
        _ => {}
    }
}

// --- substates 6, 8, 10: rest --------------------------------------------------------------------------

pub(super) fn rest<R: Bench>(x: &mut Cx<'_, R>) {
    if (var::PLAYERS_ALIVE.get(x.b) as i8) > 0 {
        return go::wander(x);
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(LOOK_PREV, 0xff);
            mv(x, 0);
        }
        2 => {
            lib::step(x);
        }
        _ => {}
    }
}
