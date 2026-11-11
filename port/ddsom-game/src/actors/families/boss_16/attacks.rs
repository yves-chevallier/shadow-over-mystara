//! Boss 16, group 4: the eight attacks (`+6` = 0 to 14). Each ends with `next_attack` (0x94e24).

use crate::actors::families::cx::Bench;
use super::*;
use crate::actors::families::effects_creators::{self, leftovers::Pure};
use crate::world::objects::pools;
use crate::world::objects::Current;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => crawl_leap(x),
        2 => charge(x),
        4 => tail_flame(x),
        6 => sweep_a(x),
        8 | 10 => sweep_b(x),
        12 => roar(x),
        14 => double_jump(x),
        _ => {}
    }
    if var::PLAYERS_ALIVE.get(x.b) == 0 {
        x.set32(f::STATE, state::IDLE);
    }
}

/// 0xb59a6 / 0xb59c8: an EFFCT 0x30 (variant 1 or 2) linked to the boss (`+0x46`).
fn spawn_trail<R: Bench>(x: &mut Cx<'_, R>, variant: u8) {
    effects_creators::type_48(&mut Pure, x.b, x.o, variant);
}

/// 0xa3b4a / 0xa3b66: an ESHL 0x2d (variant 0 or, with `index` in `+0x2f`, 1) linked to the boss (`+0x46`).
fn spawn_flame<R: Bench>(x: &mut Cx<'_, R>, variant: u8, index: u8) {
    if let Some(s) = x.alloc(&pools::ESHL) {
        s.set_u8(x.b, Obj::ACTIVE, 1);
        s.set_u8(x.b, Obj::KIND, 0x2d);
        s.set_u8(x.b, Obj::VARIANT, variant);
        if variant != 0 {
            s.set_u8(x.b, Base::SETTING, index);
        }
        s.set_u16(x.b, f::HOST, x.o.0 as u16);
    }
}

/// 0x93950: looks for a player to grab (0xfc44) and grabs them (0xfd64); true if grabbed.
fn try_grab<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let Some(player) = lib::attack_hits_a_player(x, 2) else {
        return false;
    };
    lib::seize_player(x, player, 0);
    true
}

/// 0x93884: the boss crouches down trembling, leaps onto the player and tries to grab them during the jump.
fn crawl_leap<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            x.set8(Boss16::TIMER, 1);
            x.add16(Obj::Z, 1);
            anim_b(x, 4);
        }
        2 => {
            let d = x.u8(Boss16::TIMER) as i8 as i16;
            x.add16(Obj::Z, d as u16);
            x.set8(Boss16::TIMER, (d as u8).wrapping_neg());
            if !lib::step(x) {
                return;
            }
            next7(x);
            launch(x);
        }
        4 => launch(x),
        6 => in_flight(x),
        8 => {
            x.move_xy();
            if sub8(x, Boss16::TIMER, 1).gt() {
                return;
            }
            next_attack(x);
        }
        _ => {}
    }
}

/// 0x938ce: forward takeoff, then the rest of the flight in the same frame.
fn launch<R: Bench>(x: &mut Cx<'_, R>) {
    next7(x);
    x.set8(Boss16::TIMER, 3);
    stop(x);
    x.set16(Enemy::VZ, 0x200);
    x.set16(Enemy::GRAVITY, (-0x40i16) as u16);
    let v = if x.u8(f::FLIP) != 0 { 0x800u16 } else { (-0x800i16) as u16 };
    x.set16(Enemy::VX, v);
    anim_b(x, 1);
    in_flight(x);
}

/// 0x938fe: in flight; after three frames the boss tries to grab a player, and on landing it brakes.
fn in_flight<R: Bench>(x: &mut Cx<'_, R>) {
    if tst8(x, Boss16::TIMER).gt() {
        sub8(x, Boss16::TIMER, 1);
    } else if try_grab(x) {
        return;
    }
    x.move_xz();
    if (x.u16(Obj::Z) as i16) > 0 {
        return;
    }
    x.set16(Obj::Z, 0);
    next7(x);
    x.set32(Enemy::VZ_LONG, 0);
    let v = (x.u16(Enemy::VX) as i16 >> 2) as u16;
    x.set16(Enemy::VX, v);
    x.set8(Boss16::TIMER, 0x10);
}

/// 0x9396a: charges at full speed, trembles then goes back.
fn charge<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            x.set8(Enemy::STRIKE_POWER, 0x10);
            stop(x);
            let (vx, vz) = if x.u8(f::FLIP) != 0 { (-0x400i16, 0xe0i16) } else { (0x400, -0xe0) };
            x.set16(Enemy::VX, vx as u16);
            x.set16(Enemy::AX, vz as u16);
            anim_b(x, 0);
        }
        2 => {
            x.move_xy();
            let mut v = x.u16(Enemy::VX);
            if x.u8(f::FLIP) == 0 {
                v = v.wrapping_neg();
            }
            if (v as i16) < 0 {
                return;
            }
            next7(x);
            stop(x);
            x.set8(Boss16::TIMER, 8);
        }
        4 => {
            if sub8(x, Boss16::TIMER, 1).le() {
                next7(x);
                let v = if x.u8(f::FLIP) != 0 { 0x700u16 } else { (-0x700i16) as u16 };
                x.set16(Enemy::VX, v);
                x.set8(Boss16::TIMER, 0x20);
                return;
            }
            let d: u16 = if x.u8(Boss16::TIMER) & 1 != 0 { 1 } else { 0xffff };
            x.add16(Obj::X, d);
        }
        6 => {
            lib::step(x);
            x.move_xy();
            if sub8(x, Boss16::TIMER, 1).gt() {
                return;
            }
            next_attack(x);
        }
        _ => {}
    }
}

/// 0x93a26: spits a flame (ESHL 0x2d variant 0; the EFFCT 0x30 accompany it).
fn tail_flame<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            x.set8(Boss16::TIMER, 0x40);
            anim_b(x, 0xa);
        }
        2 => {
            if !lib::step(x) {
                return;
            }
            next7(x);
            spawn_trail(x, 1);
            x.set8(Boss16::ATTACK_CUE, 1);
            anim_b(x, 0xb);
            lib::sound(x, crate::services::sound::CMD_33F);
        }
        4 => {
            x.set8(Boss16::ATTACK_CUE, 1);
            if !lib::step(x) {
                return;
            }
            next7(x);
            anim_b(x, 0xc);
        }
        6 => {
            if lib::step(x) {
                next_attack(x);
            }
        }
        _ => {}
    }
}

/// 0x93a9a: long breath (variant 1 of the flame every sixteen frames).
fn sweep_a<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            x.set8(Boss16::TIMER, 0x40);
            anim_b(x, 0x14);
        }
        2 => {
            if !lib::step(x) {
                return;
            }
            next7(x);
            anim_b(x, 0x15);
        }
        4 => {
            if var::FRAME_COUNTER.get(x.b) & 0xf == 0 {
                spawn_flame(x, 0, 0);
                lib::sound(x, crate::services::sound::CMD_33E);
            }
            if !lib::step(x) {
                return;
            }
            next7(x);
            anim_b(x, 0x16);
        }
        6 => {
            if lib::step(x) {
                next_attack(x);
            }
        }
        _ => {}
    }
}

/// 0x93b0c and 0x93b80: fire breath that leaves a trail (EFFCT 0x30 variant 2).
fn sweep_b<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            x.set8(Boss16::TIMER, 0x40);
            anim_b(x, 0x14);
        }
        2 => {
            if !lib::step(x) {
                return;
            }
            next7(x);
            spawn_trail(x, 2);
            x.set8(Boss16::ATTACK_CUE, 3);
            anim_b(x, 0x15);
            lib::sound(x, crate::services::sound::CMD_340);
        }
        4 => {
            x.set8(Boss16::ATTACK_CUE, 3);
            if !lib::step(x) {
                return;
            }
            next7(x);
            anim_b(x, 0x16);
        }
        6 => {
            if lib::step(x) {
                next_attack(x);
            }
        }
        _ => {}
    }
}

/// 0x93bf4: roar (ESHL 0x2d variant 1 at the key frame).
fn roar<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            x.set8(Enemy::ACTIVE, 1);
            x.set8(Boss16::TIMER, 0x40);
            anim_b(x, 0x1c);
        }
        2 => {
            x.set8(Boss16::ATTACK_CUE, 4);
            if x.anim_step() {
                next7(x);
                return anim_b(x, 0x1d);
            }
            let key = x.u8(Enemy::STRIKE_KEY);
            if (key as i8) >= 0 {
                return;
            }
            x.set8(Enemy::STRIKE_KEY, 0);
            let index = key & 0x7f;
            spawn_flame(x, 1, index);
            lib::sound(x, crate::services::sound::CMD_33D);
        }
        4 => {
            if x.anim_step() {
                next_attack(x);
            }
        }
        _ => {}
    }
}

/// 0x93c60: two successive jumps, the second longer.
fn double_jump<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(Enemy::SUB_STEP) {
        0 => {
            next7(x);
            x.set8(Enemy::STRIKE_POWER, 0x10);
            stop(x);
            let v = if x.u8(f::FLIP) != 0 { -0x400i16 } else { 0x400 };
            x.set16(Enemy::VX, v as u16);
            x.set16(Enemy::VZ, 0x800);
            x.set16(Enemy::GRAVITY, (-0x60i16) as u16);
            anim_b(x, 0);
        }
        2 => {
            lib::step(x);
            x.move_xz();
            if (x.u16(Obj::Z) as i16) < 0 {
                x.set16(Obj::Z, 0);
                next7(x);
                let v = if x.u8(f::FLIP) != 0 { 0x600i16 } else { -0x600 };
                x.set16(Enemy::VX, v as u16);
                x.set16(Enemy::VZ, 0x900);
                x.set16(Enemy::GRAVITY, (-0x80i16) as u16);
            }
        }
        4 => {
            lib::step(x);
            x.move_xz();
            if (x.u16(Obj::Z) as i16) < 0 {
                x.set16(Obj::Z, 0);
                next_attack(x);
            }
        }
        _ => {}
    }
}
