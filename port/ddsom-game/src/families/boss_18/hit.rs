//! Boss 18, state 4 "hit" (0x98680). `+5` groups: 0 reaction (one routine per hit code `+0x49`, nine specific
//! routines), 2 knockback (aborted magic pursuit), 4, 6, 8 death, 10 getting up.
//!
//! Reactions (`+0x49` / 4): `hit_a` (knockback in place, animation 0), `hit_b` (same, animation 1), `hit_c` (throw
//! depending on the code, animation 0), `hit_d`, `hit_e`, `hit_f`, `hit_g` (throws, animation 1), `hit_h` (lifted,
//! puffs of effects), `hit_i` (bounces until death).

use crate::families::cx::Bench;
use super::decide;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

/// Table of reaction routines (words relative to 0x986d6, indexed by `+0x49 / 2`).
const REACTIONS: &str = "boss_18/reactions";
/// Throw speeds (4 words per row) and vertical drift (0x993ec).
const THROWS: &str = "boss_18/throws";
const DRIFTS: &str = "boss_18/drifts";

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else {
        match x.u8(f::SUB) {
            0 => reaction(x),
            2 => pushed_back(x),
            4 | 6 | 8 => death(x),
            10 => get_up(x),
            _ => {}
        }
    }
    lib::keep_in_zone(x);
    lib::collide_terrain(x);
}

/// 0x99362: out of HP? Then moves to group 6 (death) and returns true.
fn dead<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if (x.u16(f::HP) as i16) > 0 {
        return false;
    }
    x.set8(f::SUB, 6);
    x.set8(Enemy::ACTIVE, 2);
    true
}

fn reaction<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        x.set8(Boss::INVULNERABLE, 0);
        var::INPUT_MASK.set(x.b, 0xff);
        lib::set_68(x);
    }
    let code = (x.u8(Enemy::REACTION) as u32) >> 1;
    let routine = jump_target(x, REACTIONS, code);
    match routine {
        0x9872c => hit_a(x, 0),
        0x9874a => hit_a(x, 1),
        0x987b8 => hit_c(x),
        0x987fc => hit_d(x, 1, 0x20, false, false),
        0x9883a => hit_d(x, 1, 0x38, true, false),
        0x98882 => hit_d(x, 1, 0x2c, false, true),
        0x988c4 => hit_d(x, 1, 0x38, false, true),
        0x98982 => hit_h(x),
        0x98a1e => hit_i(x),
        t => unreachable!("boss 18: reaction {t:x}"),
    }
}

// --- aides ------------------------------------------------------------------------------------------------

/// 0x99414: boss orientation opposite to the hit (`+0x4a`); speeds reversed if it is non-zero.
fn face_away<R: Bench>(x: &mut Cx<'_, R>) {
    let side = x.u8(Enemy::ATTACKER_FLIP) ^ 0x20;
    x.set8(f::FLIP, side);
    if side != 0 {
        let (a, b) = (x.u16(Enemy::VX).wrapping_neg(), x.u16(Enemy::AX).wrapping_neg());
        x.set16(Enemy::VX, a);
        x.set16(Enemy::AX, b);
    }
}

/// 0x9940c: the freeze after the hit (`+0x53`) takes the stun value again.
fn freeze_by_stun<R: Bench>(x: &mut Cx<'_, R>) {
    let stun = x.u8(Enemy::STUN);
    x.set8(f::FREEZE, stun);
}

/// 0x9937a: throw speeds of row `row` of the tables.
fn throw_speeds<R: Bench>(x: &mut Cx<'_, R>, row: u32) {
    let i = row.wrapping_mul(2) & 0xffff;
    for (k, off) in [Enemy::VX, Enemy::AX, Enemy::VZ, Enemy::GRAVITY].into_iter().enumerate() {
        let v = word(x, THROWS, i + 2 * k as u32);
        x.set16(off, v);
    }
    let r = ((x.random() as u32) & 0xf) * 2;
    let v = word(x, DRIFTS, r);
    x.set16(Enemy::VY, v);
    x.set16(Enemy::AY, 0);
}

fn begin_throw<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 2);
    face_away(x);
    freeze_by_stun(x);
    x.move_xyz();
}

// --- hit_a, hit_b: knockback in place --------------------------------------------------------------------------

fn hit_a<R: Bench>(x: &mut Cx<'_, R>, anim: u32) {
    if x.u8(f::SUB2) != 0 {
        return slide_back(x);
    }
    if dead(x) {
        return death(x);
    }
    x.anim_start(SET_HURT.0, anim as u16);
    x.add8(f::SUB2, 2);
    x.set8(Boss::SHAKE, 0x10);
    x.set16(Enemy::VX, 0x180);
    x.set16(Enemy::AX, (-0x30i16) as u16);
    face_away(x);
    freeze_by_stun(x);
}

/// 0x98786: trembles then slides until it stops.
fn slide_back<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(Boss::SHAKE) != 0 {
        let mut d: i16 = 2;
        sub8(x, Boss::SHAKE, 1);
        if x.u8(Boss::SHAKE) & 1 != 0 {
            d = -d;
        }
        if x.u8(f::FLIP) != 0 {
            d = -d;
        }
        x.add16(Obj::X, d as u16);
        return;
    }
    if x.u8(Enemy::VX) == 0 {
        return decide::back_on_feet(x);
    }
    x.move_x();
}

// --- hit_c: throw depending on the code ---------------------------------------------------------------------------

fn hit_c<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            if dead(x) {
                return death(x);
            }
            x.anim_start(SET_HURT.0, 0);
            let code = x.u8(Enemy::REACTION);
            let row = if code >= 0x58 { 0 } else { (code as u32).wrapping_sub(4) };
            throw_speeds(x, row);
            begin_throw(x);
        }
        _ => thrown(x),
    }
}

/// Common steps of the throws (`+6` = 2, 4, 6): rise, landing, slide.
fn thrown<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        2 => {
            x.move_xyz();
            if (x.u16(Enemy::VZ) as i16) >= 0 {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(Enemy::ACTIVE, 2);
            x.set16(Enemy::AX, 0);
        }
        4 => {
            x.move_xyz();
            if (x.u16(Obj::Z) as i16) > 0 {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(Obj::Z, 0);
            x.set16(Boss::TIMER, 0x14);
            let v = ((x.u16(Enemy::VX) as i16) >> 5).wrapping_neg();
            x.set16(Enemy::AX, v as u16);
            x.anim_start(SET_BODY.0, 5);
        }
        6 => {
            if !sub16(x, Boss::TIMER, 1).gt() {
                return decide::back_on_feet(x);
            }
            x.move_xy();
            let t = crate::video::terrain_of(x.b, x.o);
                if t.number != 0 {
                return decide::back_on_feet(x);
            }
            x.anim_step();
        }
        _ => {}
    }
}

// --- hit_d, hit_e, hit_f, hit_g : projections ---------------------------------------------------------------------

/// `anim`: animation of the reactions set; `base`: code offset for the speeds row; `slow_rise`: divides the
/// vertical speed by 4; `grounded`: the boss moves to state 2 during the throw.
fn hit_d<R: Bench>(x: &mut Cx<'_, R>, anim: u32, base: u32, slow_rise: bool, grounded: bool) {
    if x.u8(f::SUB2) != 0 {
        return thrown(x);
    }
    if dead(x) {
        return death(x);
    }
    if grounded {
        x.set8(Enemy::ACTIVE, 2);
    }
    x.anim_start(SET_HURT.0, anim as u16);
    let row = (x.u8(Enemy::REACTION) as u32).wrapping_sub(base);
    throw_speeds(x, row);
    if slow_rise {
        let v = x.u16(Enemy::VZ) >> 2;
        x.set16(Enemy::VZ, v);
    }
    begin_throw(x);
}

// --- hit_h: lifted --------------------------------------------------------------------------------------------------

fn hit_h<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            if (x.u16(Obj::Z) as i16) > 0 {
                x.set16(Enemy::VZ, 0);
                x.set16(Enemy::GRAVITY, (-0x40i16) as u16);
                x.set32(Enemy::VX_LONG, 0);
                x.set8(f::SUB2, 0);
            }
            x.add8(f::SUB2, 2);
            x.set8(Enemy::DOWNED, 0);
            x.set8(Enemy::GRABBED, 0);
            x.set8(Boss::SHAKE, 0xf0);
            lib::start(x, SET_HURT.0, 1);
            if !lib::attack_effects(x, 4, 5) {
                x.add8(f::SUB2, 2);
            }
        }
        2 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0 {
                return;
            }
            x.set32(Obj::Z, 0);
            x.add8(f::SUB2, 2);
        }
        4 => {
            lib::step(x);
        }
        6 => {
            lib::step(x);
            if sub8(x, Boss::SHAKE, 1).eq() {
                decide::back_on_feet(x);
            }
        }
        _ => {}
    }
}

// --- hit_i: bounces until death --------------------------------------------------------------------------------

/// Durations (words) of the successive bounces (0x98ab8, one per value of `+0xa6`).
const BOUNCE_TIMES: &str = "boss_18/bounce_times";

fn hit_i<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.anim_start(SET_HURT.0, 2);
            x.set32(Enemy::VX_LONG, 0);
            x.set32(Enemy::VZ_LONG, 0);
            x.set32(Enemy::VY_LONG, 0);
            x.set16(Boss::TIMER, 6);
            let right_of = var::CAMERA1_X.get(x.b).wrapping_add(0xc0);
            let side = if Fl::sub16(right_of, x.u16(Obj::X)).ge() { 0x20 } else { 0 };
            x.set8(f::FLIP, side);
            bounce_start(x);
        }
        2 => {
            if sub16(x, Boss::TIMER2, 1).gt() {
                x.move_xz();
                if (x.u16(Obj::Z) as i16) < 0 {
                    x.set16(Obj::Z, 0);
                }
                return;
            }
            let side = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, side);
            if sub16(x, Boss::TIMER, 1).pl() {
                return bounce_start(x);
            }
            x.set8(f::FREEZE, 0);
            x.set16(f::HP, 0);
            x.set32(f::STATE, 0x0406_0000);
            var::BOSS_DOWN.set(x.b, 0xff);
        }
        _ => {}
    }
}

/// 0x98a68: prepares a bounce: horizontal momentum of 0x800 toward the side faced, height depending on the camera
/// altitude.
fn bounce_start<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(f::SUB2, 2);
    let (mut vx, mut vz) = (0x800i16, -0x40i16);
    if x.u8(f::FLIP) == 0 {
        vx = -vx;
        vz = -vz;
    }
    x.set16(Enemy::VX, vx as u16);
    x.set16(Enemy::AX, vz as u16);
    let d1 = x.u16(Obj::Z).wrapping_add(x.u16(Obj::Y));
    let d0 = var::CAMERA1_Y.get(x.b).wrapping_add(0x80);
    if Fl::sub16(d1, d0).lt() {
        let v = x.u16(Enemy::VZ).wrapping_add(0x80);
        x.set16(Enemy::VZ, v);
    } else {
        let v = x.u16(Enemy::VZ).wrapping_neg();
        x.set16(Enemy::VZ, v);
    }
    let i = (x.u16(Boss::TIMER) as u32).wrapping_mul(2) & 0xffff;
    let n = word(x, BOUNCE_TIMES, i);
    x.set16(Boss::TIMER2, n);
}

// --- +5 = 2: knockback (aborted magic pursuit) ------------------------------------------------------------------

fn pushed_back<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.set16(Boss::TIMER, 0x14);
            x.set16(Enemy::AY, 0);
            let (angle, vz) = if x.u8(f::FLIP) == 0 { (8, -0x20i16) } else { (0x18, 0x20) };
            x.set8(Obj::SPEED_INDEX, angle);
            x.set16(Enemy::AX, vz as u16);
            lib::drift_heading(x);
            speed(x, 0x17);
            x.set8(Boss::ANIM_KIND, 0x20);
            x.set8(Boss::ANIM_LAST, 0x20);
            x.add8(f::SUB2, 2);
            body(x, 7);
        }
        2 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                x.move_xy();
                return step_only(x);
            }
            x.add8(f::SUB2, 2);
            x.set16(Boss::TIMER, 0x14);
            x.set16(Enemy::AX, 0);
            x.move_xy();
            step_only(x);
        }
        4 => {
            if sub16(x, Boss::TIMER, 1).gt() {
                x.move_xy();
                return step_only(x);
            }
            x.set32(Enemy::HIT_LOCK, 0);
            decide::attack_6(x);
        }
        _ => {}
    }
}

fn step_only<R: Bench>(x: &mut Cx<'_, R>) {
    lib::step(x);
}

// --- mort ------------------------------------------------------------------------------------------------------------

fn death<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            var::PLAYERS_FROZEN.set(x.b, 1);
            var::BOSS_FIGHT.set(x.b, 0);
            let kind = x.u8(Enemy::KIND);
            var::DEFEAT_NAME.set(x.b, kind);
            var::DEFEAT_FIRST.set(x.b, 0);
            let bar = x.u8(Enemy::EVENT_KIND);
            var::DEFEAT_BONUS.set(x.b, bar);
            lib::flash_and_shake(x);
            crate::sound::service(x.b, crate::sound::Service::Ff05);
            lib::sound(x, crate::sound::CMD_FF);
            lib::sound(x, crate::sound::CMD_6B_6B);
            lib::drop_rewards(x);
            x.set8(Enemy::ACTIVE, 2);
            x.set16(Enemy::AX, 0);
            x.set16(Enemy::VZ, 0x400);
            x.set16(Enemy::GRAVITY, (-0x28i16) as u16);
            let (mut vx, mut side) = (0x180i16, 0x20u8);
            let edge = var::CAMERA1_X.get(x.b).wrapping_add(0xc0);
            if !Fl::sub16(edge, x.u16(Obj::X)).ge() {
                vx = -vx;
                side = 0;
            }
            x.set16(Enemy::VX, vx as u16);
            x.set8(f::FLIP, side);
            x.move_xz();
            x.anim_start(SET_HURT.0, 2);
        }
        2 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0 {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(Obj::Z, 0);
        }
        4 => {
            if !x.anim_step() {
                return;
            }
            x.add8(f::SUB2, 2);
            lib::sound(x, crate::sound::CMD_AA);
        }
        _ => {}
    }
}

// --- +5 = 10: getting up ---------------------------------------------------------------------------------------------

fn get_up<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        x.add8(Enemy::SUB_STEP, 2);
        return body(x, 6);
    }
    if !lib::step(x) {
        return;
    }
    if !lib::players_alive(x) {
        return decide::wait(x);
    }
    decide::back_on_feet(x);
}
