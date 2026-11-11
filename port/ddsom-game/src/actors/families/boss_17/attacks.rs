//! Boss 17, group `+5` = 4: the attacks (`+6` = 0 to 16), and their dust effects.
//!
//! | `+6` | Attack (h) |
//! |---|---|
//! | 0 | simple hit |
//! | 2 | hit with a wave |
//! | 4 | double hit with a run-up |
//! | 6 | leap toward the target |
//! | 8, 10 | ESHL 0x20 shot (throw) |
//! | 12 | ESHL 0x21 shot (breath) |
//! | 14 | cry that drops objects around the camera |
//! | 16 | final cry, slow-motion mode |

use crate::actors::families::cx::Bench;
use super::decide;
use super::*;
use crate::world::objects::pools;
use crate::world::objects::Current;
use crate::world::objects::Obj;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => strike_0(x),
        2 => strike_2(x),
        4 => strike_4(x),
        6 => leap(x),
        8 | 10 => throw(x),
        12 => breath(x),
        14 => roar(x),
        16 => final_roar(x),
        _ => {}
    }
}

fn players_alive<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    var::PLAYERS_ALIVE.get(x.b) != 0
}

fn state7<R: Bench>(x: &mut Cx<'_, R>) -> u8 {
    x.u8(Enemy::SUB_STEP)
}

/// End of an attack: return to combat if the animation is finished.
fn finish<R: Bench>(x: &mut Cx<'_, R>, to: fn(&mut Cx<'_, R>)) {
    if lib::step(x) {
        if !players_alive(x) {
            return decide::wait(x);
        }
        to(x);
    }
}

fn strike_anim<R: Bench>(x: &mut Cx<'_, R>, kind: u8, n: u32) {
    x.set8(Boss::ANIM_KIND, kind);
    x.set8(Boss::ANIM_LAST, kind);
    lib::start(x, SET_STRIKE.0, n);
}

/// Dust: an EFFCT 0x32 of variant `v` linked to the boss, without insisting (0x2318).
fn puff<R: Bench>(x: &mut Cx<'_, R>, v: u8) {
    if let Some(e) = x.alloc(&pools::EFFCT) {
        e.set_u8(x.b, Obj::ACTIVE, 1);
        e.set_u8(x.b, Obj::KIND, 0x32);
        e.set_u8(x.b, Obj::VARIANT, v);
        e.set_u16(x.b, Enemy::CREATOR, x.o.0 as u16);
        let side = x.u8(f::FLIP);
        e.set_u8(x.b, f::FLIP, side);
    }
}

/// 0x97580: dust at regular intervals during the slow motion.
pub(super) fn slow_tick<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Boss17::SLOW) == 0 {
        return;
    }
    x.add8(Boss17::SLOW_TICK, 1);
    let every = if x.u8(Boss17::SLOW) == 1 { 0xb } else { 7 };
    if Fl::sub8(every, x.u8(Boss17::SLOW_TICK)).gt() {
        return;
    }
    x.set8(Boss17::SLOW_TICK, 0);
    puff(x, 4);
}

/// 0x975a2: dust except during the slow motion.
pub(super) fn dust_unless_slow<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Boss17::SLOW) == 0 {
        puff(x, 4);
    }
}

fn spark<R: Bench>(x: &mut Cx<'_, R>) {
    lib::sound(x, crate::services::sound::CMD_80_80);
}

// --- +6 = 0 -----------------------------------------------------------------------------------

fn strike_0<R: Bench>(x: &mut Cx<'_, R>) {
    match state7(x) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            strike_anim(x, 0x10, 0);
        }
        2 => {
            if lib::step(x) {
                if !players_alive(x) {
                    return decide::wait(x);
                }
                return decide::chase(x);
            }
            if x.u8(f::KEY_FRAME) != 0 {
                x.set8(f::KEY_FRAME, 0);
                spark(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 2 -----------------------------------------------------------------------------------

fn strike_2<R: Bench>(x: &mut Cx<'_, R>) {
    match state7(x) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            strike_anim(x, 0x11, 1);
        }
        2 => {
            if lib::step(x) {
                if !players_alive(x) {
                    return decide::wait(x);
                }
                return decide::chase(x);
            }
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.set8(f::KEY_FRAME, 0);
            puff(x, 0);
            spark(x);
        }
        _ => {}
    }
}

// --- +6 = 4: double hit with a run-up ---------------------------------------------------------------

fn strike_4<R: Bench>(x: &mut Cx<'_, R>) {
    match state7(x) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            strike_anim(x, 0x12, 2);
        }
        2 => {
            lib::step(x);
            if !tst8(x, f::KEY_FRAME).mi() {
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(f::KEY_FRAME, 0);
            x.set16(Boss::TIMER, 0x1e);
            puff(x, 3);
            let (vx, vz) = if x.u8(f::FLIP) == 0 { (-0x900i16, 0x4ci16) } else { (0x900, -0x4c) };
            x.set16(Enemy::VX, vx as u16);
            x.set16(Enemy::AX, vz as u16);
            lib::sound(x, crate::services::sound::CMD_363_363);
            x.move_x();
        }
        4 => {
            if sub16(x, Boss::TIMER, 1).le() {
                x.add8(Enemy::SUB_STEP, 2);
                x.set16(Boss::TIMER, 8);
                x.set8(Boss::INVULNERABLE, 0);
                return;
            }
            x.move_x();
            lib::step(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.set8(f::KEY_FRAME, 0);
            puff(x, 1);
            lib::sound(x, crate::services::sound::CMD_81_81);
        }
        6 => {
            if sub16(x, Boss::TIMER, 1).le() {
                x.add8(Enemy::SUB_STEP, 2);
                lib::start(x, SET_STRIKE.0, 3);
            }
        }
        8 => finish(x, decide::chase),
        _ => {}
    }
}

// --- +6 = 6: leap -------------------------------------------------------------------------------

fn leap<R: Bench>(x: &mut Cx<'_, R>) {
    match state7(x) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Enemy::AX, 0);
            let angle = if x.u8(f::FLIP) == 0 { 0x18 } else { 8 };
            x.set8(Obj::SPEED_INDEX, angle);
            speed(x, 0x17);
            // depth speed large enough to reach the target in one leap
            let target = x.linked(f::TARGET);
            let mut gap = (x.u32(Obj::X) as i32).wrapping_sub(target.u32(x.b, Obj::X) as i32);
            if gap < 0 {
                gap = gap.wrapping_neg();
            }
            let per_frame = ((x.u16(Enemy::VX) as i16 as i32) << 8).wrapping_abs();
            let mut vz: u16 = 0x100;
            // the original's loop does not end if the speed is zero
            for _ in 0..0x10000 {
                gap = gap.wrapping_sub(per_frame);
                if gap <= 0 {
                    break;
                }
                vz = vz.wrapping_add(0xe);
            }
            x.set16(Enemy::VZ, vz);
            x.set16(Enemy::GRAVITY, (-0x20i16) as u16);
            x.set8(Boss::ANIM_KIND, 0x13);
            x.set8(Boss::ANIM_LAST, 0x13);
            body(x, 5);
        }
        2 => {
            x.anim_step();
            if x.u8(f::KEY_FRAME) != 0 {
                return;
            }
            x.move_xz();
            if cmp16(x, Enemy::VZ, 0x120).le() {
                x.add8(Enemy::SUB_STEP, 2);
                strike(x, 4);
            }
        }
        4 => {
            x.move_xz();
            if cmp16(x, Enemy::VZ, 0xff60).le() {
                x.add8(Enemy::SUB_STEP, 2);
                x.set16(Enemy::GRAVITY, (-0x30i16) as u16);
                strike(x, 5);
            } else {
                lib::step(x);
            }
        }
        6 => {
            x.move_xz();
            if tst16(x, Obj::Z).le() {
                x.add8(Enemy::SUB_STEP, 2);
                x.set16(Obj::Z, 0);
                lib::sound(x, crate::services::sound::CMD_362);
                return body(x, 0xa);
            }
            lib::step(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.set8(f::KEY_FRAME, 0);
            puff(x, 2);
            lib::sound(x, crate::services::sound::CMD_361);
            lib::sound(x, crate::services::sound::CMD_81_81);
        }
        8 => finish(x, decide::chase),
        _ => {}
    }
}

// --- +6 = 8, 10: throw ---------------------------------------------------------------------------

fn throw<R: Bench>(x: &mut Cx<'_, R>) {
    match state7(x) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            var::INPUT_MASK.set(x.b, 0x7f);
            x.set8(Boss::INVULNERABLE, 1);
            x.add8(Boss17::BUDGET_A, 0xff);
            let target = x.linked(f::TARGET);
            let (tx, ty) = (target.u16(x.b, Obj::X), target.u16(x.b, Obj::Y));
            x.set16(Boss::MARK_X, tx);
            x.set16(Boss17::MARK_Y, ty);
            strike_anim(x, 0x15, 7);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Boss::INVULNERABLE, 0);
            x.set16(Boss::TIMER, 0x14);
            let Some(shot) = x.alloc(&pools::ESHL) else { return throw_end(x) };
            x.set16(Boss17::CHILD, shot.0 as u16);
            shot.set_u8(x.b, Obj::ACTIVE, 1);
            shot.set_u8(x.b, Obj::KIND, 0x20);
            shot.set_u8(x.b, Obj::VARIANT, 1);
            shot.set_u16(x.b, ShotB::HOST, x.o.0 as u16);
            let side = x.u8(f::FLIP);
            shot.set_u8(x.b, f::FLIP, side);
            for off in [Obj::X, Obj::Z, Obj::Y] {
                let v = x.u16(off);
                shot.set_u16(x.b, off, v);
            }
            let dx: i16 = if x.u8(f::FLIP) == 0 { -0x44 } else { 0x44 };
            let px = shot.u16(x.b, Obj::X).wrapping_add(dx as u16);
            shot.set_u16(x.b, Obj::X, px);
            shot.set_u16(x.b, Obj::Z, 0x30);
            lib::step(x);
        }
        4 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                lib::step(x);
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Boss::TIMER, 0xa);
            let shot = x.linked(Boss17::CHILD);
            shot.set_u8(x.b, Shot32::ARMED, 1);
            lib::sound(x, crate::services::sound::CMD_84_84);
            lib::step(x);
        }
        6 => {
            let shot = x.linked(Boss17::CHILD);
            if shot.u8(x.b, Obj::ACTIVE) == 0 || shot.u8(x.b, Shot32::DONE) != 0 {
                return throw_end(x);
            }
            lib::step(x);
        }
        8 => {
            if lib::step(x) {
                var::INPUT_MASK.set(x.b, 0xff);
                if !players_alive(x) {
                    return decide::wait(x);
                }
                decide::chase(x);
            }
        }
        _ => {}
    }
}

/// 0x96614: the shot has disappeared (or could not be born): end of the gesture.
fn throw_end<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Enemy::SUB_STEP, 2);
    lib::start(x, SET_STRIKE.0, 8);
}

// --- +6 = 12: breath ------------------------------------------------------------------------------

fn breath<R: Bench>(x: &mut Cx<'_, R>) {
    match state7(x) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            var::CAMERA_MODE.set(x.b, 4);
            var::INPUT_MASK.set(x.b, 0x7f);
            x.set8(Boss::INVULNERABLE, 1);
            x.add8(Boss17::BUDGET_B, 0xff);
            strike_anim(x, 0x16, 9);
        }
        2 => {
            var::CAMERA_MODE.set(x.b, 4);
            lib::step(x);
            if x.u8(f::KEY_FRAME) == 0 {
                return;
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Boss::TIMER, 0xa);
            x.set8(f::KEY_FRAME, 0);
            let Some(shot) = x.alloc(&pools::ESHL) else { return breath_end(x) };
            x.set16(Boss17::CHILD, shot.0 as u16);
            shot.set_u8(x.b, Obj::ACTIVE, 1);
            shot.set_u8(x.b, Obj::KIND, 0x21);
            shot.set_u16(x.b, ShotB::HOST, x.o.0 as u16);
            let side = x.u8(f::FLIP) ^ 0x20;
            shot.set_u8(x.b, f::FLIP, side);
            for off in [Obj::X, Obj::Z, Obj::Y] {
                let v = x.u16(off);
                shot.set_u16(x.b, off, v);
            }
            let dx: i16 = if x.u8(f::FLIP) == 0 { -0x3b } else { 0x3b };
            let px = shot.u16(x.b, Obj::X).wrapping_add(dx as u16);
            let pz = shot.u16(x.b, Obj::Z).wrapping_add(0x30);
            shot.set_u16(x.b, Obj::X, px);
            shot.set_u16(x.b, Obj::Z, pz);
        }
        4 => {
            var::CAMERA_MODE.set(x.b, 4);
            if sub16(x, Boss::TIMER, 1).gt() {
                return step_only(x);
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set8(Boss::INVULNERABLE, 0);
            x.set16(Boss::TIMER, 0x5a);
            let shot = x.linked(Boss17::CHILD);
            shot.set_u8(x.b, Shot33::FIRST, 1);
            lib::sound(x, crate::services::sound::CMD_85_85);
            lib::step(x);
        }
        6 => {
            var::CAMERA_MODE.set(x.b, 4);
            if sub16(x, Boss::TIMER, 1).gt() {
                return step_only(x);
            }
            x.add8(Enemy::SUB_STEP, 2);
            let shot = x.linked(Boss17::CHILD);
            shot.set_u8(x.b, Shot33::SECOND, 1);
            lib::step(x);
        }
        8 => {
            let shot = x.linked(Boss17::CHILD);
            if shot.u8(x.b, Obj::ACTIVE) == 0 || shot.u8(x.b, Shot33::DONE) != 0 {
                return breath_end(x);
            }
            var::CAMERA_MODE.set(x.b, 4);
            lib::step(x);
        }
        10 => {
            if lib::step(x) {
                var::INPUT_MASK.set(x.b, 0xff);
                if !players_alive(x) {
                    return decide::wait(x);
                }
                decide::stand(x);
            }
        }
        _ => {}
    }
}

fn step_only<R: Bench>(x: &mut Cx<'_, R>) {
    lib::step(x);
}

/// 0x9676c.
fn breath_end<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Enemy::SUB_STEP, 2);
    var::CAMERA_MODE.set(x.b, 0);
    lib::start(x, SET_STRIKE.0, 0xa);
}

// --- +6 = 14: cry ----------------------------------------------------------------------------------

fn roar<R: Bench>(x: &mut Cx<'_, R>) {
    match state7(x) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            var::INPUT_MASK.set(x.b, 0x7f);
            x.set8(Boss::INVULNERABLE, 1);
            x.add8(Boss17::BUDGET_C, 0xff);
            strike_anim(x, 0x17, 0xb);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) != 0 {
                x.add8(Enemy::SUB_STEP, 2);
                x.set16(Boss::TIMER, 0x1e);
                x.set8(f::KEY_FRAME, 0);
            }
        }
        4 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                return step_only(x);
            }
            x.add8(Enemy::SUB_STEP, 2);
            var::INPUT_MASK.set(x.b, 0xff);
            x.set8(Boss::INVULNERABLE, 0);
            decide::spawn_by_level(x);
            lib::sound(x, crate::services::sound::CMD_86_86);
            x.set16(Boss::TIMER, 0x3c);
            lib::step(x);
        }
        6 => {
            if sub16(x, Boss::TIMER, 1).le() {
                x.add8(Enemy::SUB_STEP, 2);
                lib::start(x, SET_STRIKE.0, 0xc);
            } else {
                lib::step(x);
            }
        }
        8 => finish(x, decide::stand),
        _ => {}
    }
}

// --- +6 = 16: final cry ----------------------------------------------------------------------------

fn final_roar<R: Bench>(x: &mut Cx<'_, R>) {
    match state7(x) {
        0 => {
            x.add8(Enemy::SUB_STEP, 2);
            var::INPUT_MASK.set(x.b, 0x7f);
            x.set8(Enemy::ACTIVE, 2);
            strike_anim(x, 0x18, 0xd);
        }
        2 => {
            lib::step(x);
            if x.u8(f::KEY_FRAME) != 0 {
                x.add8(Enemy::SUB_STEP, 2);
                x.set16(Boss::TIMER, 0xa);
                x.set8(f::KEY_FRAME, 0);
            }
        }
        4 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                return step_only(x);
            }
            x.add8(Enemy::SUB_STEP, 2);
            x.set16(Boss::TIMER, 0x3c);
            if let Some(e) = x.alloc(&pools::EFFCT) {
                e.set_u8(x.b, Obj::ACTIVE, 1);
                e.set_u8(x.b, Obj::KIND, 0x25);
                e.set_u16(x.b, Enemy::CREATOR, x.o.0 as u16);
                let (px, py) = (x.u16(Obj::X), x.u16(Obj::Y));
                e.set_u16(x.b, Obj::X, px);
                e.set_u16(x.b, Obj::Y, py);
                e.set_u16(x.b, Obj::Z, 0xa0);
                let dx: i16 = if x.u8(f::FLIP) != 0 { -5 } else { 5 };
                e.set_u16(x.b, Obj::X, px.wrapping_add(dx as u16));
            }
            lib::step(x);
        }
        6 => {
            if sub16(x, Boss::TIMER, 1).le() {
                x.add8(Enemy::SUB_STEP, 2);
                lib::start(x, SET_STRIKE.0, 0xe);
            } else {
                lib::step(x);
            }
        }
        8 => {
            if !lib::step(x) {
                return;
            }
            var::INPUT_MASK.set(x.b, 0xff);
            x.set8(Enemy::ACTIVE, 1);
            x.set8(Boss17::SLOW_TICK, 0);
            decide::pick_slow_time(x);
            decide::pick_slow_mode(x);
            let mode = x.u8(Boss17::SLOW);
            x.set8(Boss17::SLOW_COUNT, mode);
            if !players_alive(x) {
                return decide::wait(x);
            }
            decide::stand(x);
        }
        _ => {}
    }
}
