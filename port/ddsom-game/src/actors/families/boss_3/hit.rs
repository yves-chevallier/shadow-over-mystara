//! Boss 3, state 4 "hit" (0x80a90). `+5` groups:
//!
//! | `+5` | Role |
//! |---|---|
//! | 0 | reaction to the hit: routine from table 0x80b0a (code `+0x49`) |
//! | 2 | knockback in flight |
//! | 4 | on the ground, then getting up (randomly drawn counterattack) |
//! | 6 | death: fall, bounce, collapse |
//! | 10 | return to combat |

use crate::actors::families::cx::Bench;
use crate::world::data::RomAddr;
use super::*;
use crate::world::objects::Current;

/// Table of reaction routines, indexed by the hit code `+0x49` (longs, all common).
const REACTIONS: RomAddr = RomAddr(0x80b0a);

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB) == 0 && !tst16(x, f::HP).gt() {
        x.set8(f::SUB, 6);
        x.set8(f::SUB2, 0);
        return death(x);
    }
    let freeze = x.u8(f::FREEZE);
    if freeze != 0 && (freeze as i8) > 0 {
        x.set8(f::FREEZE, freeze - 1);
    } else {
        match x.u8(f::SUB) {
            0 => reaction(x),
            2 => pushed_back(x),
            4 => down(x),
            6 => death(x),
            8 => {}
            _ => after_attack(x),
        }
    }
    end_frame(x, 0x50);
}

/// 0x80ae8: first pass: the type's immunities; then the common routine of the hit code.
fn reaction<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(Boss3::HOVER, 0xff);
    if x.u8(f::SUB2) == 0 {
        x.set_bit(Enemy::DISPLAY, 1, false);
        lib::set_immunities(x);
    }
    // The original indexes the table by D0.w of which only the low byte is set: the high byte comes from the `moveq #0`
    // of
    // the group dispatch, or from the index returned by 0xfe5a (at most (32 + type) × 4 < 0x100): it is zero.
    let routine = reaction_routine(x, REACTIONS.0);
    run_reaction(x, routine);
}

/// 0x80bb6: knockback in flight (animation 9) until it stops; it ends up on the ground.
fn pushed_back<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        next6(x);
        x.set8(Enemy::AI_68, 1);
        x.set8(Enemy::AI_69, 1);
        x.set16(Enemy::VX, 0x400);
        x.set16(Enemy::AX, (-0x88i16) as u16);
        if x.u8(f::FLIP) != 0 {
            negate(x, 0x14);
            negate(x, 0x16);
        }
        return body_anim(x, 9);
    }
    x.move_x();
    if (x.u8(Enemy::VX) as i8) < 0 {
        return stop(x);
    }
    if !lib::step(x) {
        return;
    }
    x.set32(Enemy::HIT_LOCK, 0);
    x.set32(f::STATE, 0x0204_0400);
    x.set8(Boss3::INVULNERABLE, 6);
}

/// 0x80c2c: on the ground (`+0x84` frames outside pause), then getting up or counterattack.
fn down<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            lib::behaviour_by_kind(x);
            body_anim(x, 0x11);
        }
        2 => {
            if var::PAUSE.get(x.b) != 0 || sub16(x, Boss::DOWN_TIME, 1).gt() {
                return;
            }
            x.set16(Boss::DOWN_TIME, 0);
            next6(x);
            x.set8(Enemy::DOWNED, 0);
            x.set8(Enemy::REACTION, 0);
            // 0x80cb0: counterattack 6 chances in 16
            let r = rand_and(x, 0xf);
            if 0xfc00u32 >> r & 1 == 0 {
                return;
            }
            let r = rand_and(x, 0xf);
            let k = byte(x, "boss_3/counter_steps", r);
            x.set8(f::SUB2, k);
            x.set16(f::STATE, 0x0204);
            x.set8(Enemy::SUB_STEP, 0);
            x.set8(Enemy::DOWNED, 0);
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(Boss3::INVULNERABLE, 6);
            x.set8(Enemy::FRAME_TAG, 0);
            x.set8(Enemy::ACTIVE, 1);
        }
        _ => {
            if var::PAUSE.get(x.b) != 0 || !lib::step(x) {
                return;
            }
            x.set8(Enemy::VULNERABLE, 1);
            x.set8(Enemy::ACTIVE, 1);
            x.set32(f::STATE, 0x0202_0400);
            body_anim(x, 0);
        }
    }
}

/// 0x80d16: death: the stage is notified (0x300: boss 3 defeated), fall, bounce, collapse.
fn death<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            var::PLAYERS_FROZEN.set(x.b, 1);
            var::BOSS_FIGHT.set(x.b, 0);
            record_defeat(x, 3);
            lib::drop_rewards(x);
            crate::services::sound::service(x.b, crate::services::sound::Service::Ff05);
            lib::sound(x, crate::services::sound::CMD_FF);
            lib::flash_and_shake(x);
            x.set8(Enemy::ACTIVE, 2);
            let f = x.u8(Enemy::STUN);
            x.set8(f::FREEZE, f);
            stop(x);
            x.set16(Enemy::VZ, 0x400);
            x.set16(Enemy::GRAVITY, (-0x40i16) as u16);
            let mut v = 0x200u16;
            if x.u8(f::FLIP) != 0 {
                v = v.wrapping_neg();
            }
            x.set16(Enemy::VX, v);
            x.anim_start(SET_DEATH.0, 2);
            falling(x)
        }
        2 => falling(x),
        4 => {
            x.move_x();
            if !x.anim_step() {
                return;
            }
            next6(x);
            x.anim_start(SET_DEATH.0, 0x16);
        }
        _ => {}
    }
}

/// 0x80da2: fall to the ground, then bounce.
fn falling<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
    x.move_xz();
    if (x.u16(Obj::Z) as i16) >= 0 {
        return;
    }
    x.set16(Obj::Z, 0);
    let v = ((x.u16(Enemy::VX) as i16) >> 2) as u16;
    x.set16(Enemy::VX, v);
    x.set32(Enemy::VY_LONG, 0);
    x.set32(Enemy::VZ_LONG, 0);
    next6(x);
    x.anim_start(SET_DEATH.0, 0x10);
    lib::sound(x, crate::services::sound::CMD_AA);
}

