//! Boss 1, state 4 "hit" (0x7bb66) and return to combat. `+5` groups:
//!
//! | `+5` | Role |
//! |---|---|
//! | 0 | reaction to the hit: death if out of HP, otherwise routine from table 0x7bbbe (code `+0x49`) |
//! | 2 | knocked back sliding, then counterattack |
//! | 4 | on the ground, getting up, counterattack leap |
//! | 6, 8 | death: thrown, falls into the dust |
//! | 10 | return to combat |

use super::*;
use crate::world::data::RomAddr;
use crate::actors::families::cx::Bench;
use crate::world::objects::Current;
use crate::world::objects::Obj;

/// Table of reaction routines, indexed by the hit code `+0x49` (longs).
const REACTIONS: RomAddr = RomAddr(0x7bbbe);
/// The only routine specific to the boss (code 0x4c): shaking in place.
const SHAKE_REACTION: RomAddr = RomAddr(0x7bc6a);

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else {
        match x.u8(f::SUB) {
            0 => reaction(x),
            2 => pushed_back(x),
            4 => down(x),
            6 | 8 => death(x),
            _ => get_up(x),
        }
    }
    lib::keep_in_zone(x);
    lib::collide_terrain(x);
}

/// 0x7bb96: first pass: damage (0xb648, 0x10112) and, with no HP left, death; then the hit code's routine.
fn reaction<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        lib::drop_once(x);
        lib::set_68(x);
        x.set8(Boss::INVULNERABLE, 0);
        // 0x7c588
        if !tst16(x, f::HP).gt() {
            x.set8(f::SUB, 6);
            x.set8(Enemy::ACTIVE, 2);
            return death(x);
        }
    }
    let routine = reaction_routine(x, REACTIONS.0);
    if routine == SHAKE_REACTION.0 {
        shake(x)
    } else {
        run_reaction(x, routine)
    }
}

/// 0x7bc6a: shaking in place (16 frames), then on the ground.
fn shake<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        next6(x);
        x.set8(Enemy::ACTIVE, 2);
        let px = x.u16(Obj::X);
        x.set16(Enemy::X_SAVED, px);
        x.set16(Boss::DOWN_TIME, 0);
        let freeze = x.u8(Enemy::STUN);
        x.set8(f::FREEZE, freeze);
        x.set8(Boss::SHAKE, 0x10);
        return lib::start(x, SET_HURT.0, 8);
    }
    if tst8(x, Boss::SHAKE).gt() {
        let n = x.u8(Boss::SHAKE).wrapping_sub(1);
        x.set8(Boss::SHAKE, n);
        let mut d = 2u16;
        if n & 1 != 0 {
            d = d.wrapping_neg();
        }
        if x.u8(f::FLIP) != 0 {
            d = d.wrapping_neg();
        }
        x.add16(Obj::X, d);
        return;
    }
    let px = x.u16(Enemy::X_SAVED);
    x.set16(Obj::X, px);
    x.set8(Enemy::ACTIVE, 1);
    x.set32(f::STATE, 0x0404_0200);
    x.set32(Enemy::HIT_LOCK, 0);
}

/// 0x7bce2: knocked back sliding, then counterattack depending on the target's position.
fn pushed_back<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.set16(Boss::TIMER, 0xa);
            x.set16(Obj::Z, 0);
            let (angle, vz) = if x.u8(f::FLIP) != 0 { (0x18u32, 0x20u16) } else { (8, (-0x20i16) as u16) };
            x.set8(Obj::SPEED_INDEX, angle as u8);
            x.set16(Enemy::AX, vz);
            speed(x, 0x17);
            kind(x, 0x20);
            next6(x);
            body(x, 0x1a);
        }
        2 => {
            if expired(x, Boss::TIMER) {
                next6(x);
                x.set16(Boss::TIMER, 4);
                x.set16(Enemy::AX, 0);
            }
            x.move_x();
            lib::step(x);
        }
        _ => {
            if !expired(x, Boss::TIMER) {
                x.move_x();
                lib::step(x);
                return;
            }
            x.set32(Enemy::HIT_LOCK, 0);
            if lib::target_in_box(x, -0x60, 0x35, 6) {
                return go(x, 0x7c344);
            }
            if lib::target_in_box(x, -0xe0, 0x30, 8) {
                return go(x, 0x7c2fa);
            }
            walk(x)
        }
    }
}

/// 0x7bda4: on the ground (`+0x84` frames outside pause), getting up, and a leap if the target is ahead.
fn down<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            lib::behaviour_by_kind(x);
            x.set8(Boss::ANIM_LAST, 0xff);
            body(x, 0x1b);
        }
        2 => {
            if (var::PAUSE.get(x.b) as i8) > 0 || sub16(x, Boss::DOWN_TIME, 1).gt() {
                return;
            }
            next6(x);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(Enemy::DOWNED, 0);
            x.set8(Enemy::REACTION, 0);
        }
        4 => {
            if var::PAUSE.get(x.b) != 0 {
                return;
            }
            if lib::step(x) {
                return get_up(x);
            }
            if x.u8(f::KEY_FRAME) == 0 || outside(x, 0x18, 0x168) || !lib::target_in_box(x, -0x10, 0x30, 0xc) {
                return;
            }
            next6(x);
            x.set16(Enemy::AX, 0);
            x.set16(Enemy::AY, 0);
            x.set16(Enemy::VZ, 0x480);
            x.set16(Enemy::GRAVITY, (-0x38i16) as u16);
            let angle = if x.u8(f::FLIP) != 0 { 0x18 } else { 8 };
            x.set8(Obj::SPEED_INDEX, angle);
            speed(x, 0x12);
            body(x, 0x1c);
        }
        6 => {
            lib::step(x);
            x.move_xyz();
            if x.u16(Obj::Z) as i16 > 0 {
                return;
            }
            next6(x);
            x.set16(Obj::Z, 0);
            dust(x, 6);
            lib::sound(x, crate::hw::sound::CMD_35F_45F);
            body(x, 0x1d);
        }
        _ => {
            if lib::step(x) {
                return;
            }
            x.set8(Enemy::ACTIVE, 1);
            x.set8(Boss::ANIM_LAST, 0xff);
            lib::next_threshold(x, THRESHOLDS);
            get_up(x)
        }
    }
}

/// 0x7bece: return to combat (also called by the common reactions, table 0x3e000): crosses the next HP threshold
/// (0x105b6), then a leap if off screen, a scissor leap, a grab or a wait.
pub(crate) fn get_up<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Enemy::ACTIVE, 1);
    x.set8(Boss::ANIM_LAST, 0xff);
    if !pick_target(x) {
        return idle_alone(x);
    }
    lib::next_threshold(x, THRESHOLDS);
    if outside(x, 0x18, 0x168) {
        return leap_guarded(x);
    }
    if try_split_leap(x) {
        return split_leap(x);
    }
    if lib::target_in_box(x, -0x40, 0x30, 8) {
        return go(x, 0x7c344);
    }
    stand(x)
}

/// 0x7bf22: death: the stage is notified, the boss is thrown then collapses.
fn death<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            var::PLAYERS_FROZEN.set(x.b, 1);
            var::BOSS_FIGHT.set(x.b, 0);
            let kind = x.u8(Enemy::KIND);
            record_defeat(x, kind);
            lib::flash_and_shake(x);
            crate::hw::sound::service(x.b, crate::hw::sound::Service::Ff05);
            lib::sound(x, crate::hw::sound::CMD_FF);
            lib::sound(x, crate::hw::sound::CMD_30E_40E);
            lib::drop_rewards(x);
            x.set8(Enemy::ACTIVE, 2);
            x.set16(Enemy::AX, 0);
            x.set16(Enemy::VZ, 0x500);
            x.set16(Enemy::GRAVITY, (-0x38i16) as u16);
            let center = var::CAMERA1_X.get(x.b).wrapping_add(0xc0);
            *x.c.d2() = *x.c.d2() & 0xffff_0000 | center as u32;
            let (vx, side) = if center as i16 >= x.u16(Obj::X) as i16 { (0x200u16, 0u32) } else { ((-0x200i16) as u16, 0x20) };
            x.set16(Enemy::VX, vx);
            x.set8(f::FLIP, side as u8);
            x.move_xz();
            x.anim_start(SET_BODY.0, 0x1e);
        }
        2 => {
            x.move_xz();
            if x.u16(Obj::Z) as i16 > 0 {
                lib::step(x);
                return;
            }
            next6(x);
            x.set16(Obj::Z, 0);
            dust(x, 6);
            lib::sound(x, crate::hw::sound::CMD_35D);
            x.anim_start(SET_BODY.0, 0x1f);
        }
        4 => {
            if x.anim_step() {
                next6(x);
            }
        }
        _ => {}
    }
}


