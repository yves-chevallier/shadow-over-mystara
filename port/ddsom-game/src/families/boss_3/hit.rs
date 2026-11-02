//! Boss 3, state 4 "hit" (0x80a90). `+5` groups:
//!
//! | `+5` | Role |
//! |---|---|
//! | 0 | reaction to the hit: routine from table 0x80b0a (code `+0x49`) |
//! | 2 | knockback in flight |
//! | 4 | on the ground, then getting up (randomly drawn counterattack) |
//! | 6 | death: fall, bounce, collapse |
//! | 10 | return to combat |

use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;

/// Table of reaction routines, indexed by the hit code `+0x49` (longs, all common).
const REACTIONS: u32 = 0x80b0a;

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
    x.set8(HOVER, 0xff);
    if x.u8(f::SUB2) == 0 {
        x.set_bit(0x1, 1, false);
        lib::set_immunities(x);
    }
    // The original indexes the table by D0.w of which only the low byte is set: the high byte comes from the `moveq #0`
    // of
    // the group dispatch, or from the index returned by 0xfe5a (at most (32 + type) × 4 < 0x100): it is zero.
    let code = x.u8(0x49) as u32;
    let routine = x.prog32(REACTIONS + code);
    run_reaction(x, routine);
}

/// 0x80bb6: knockback in flight (animation 9) until it stops; it ends up on the ground.
fn pushed_back<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        next6(x);
        x.set8(0x68, 1);
        x.set8(0x69, 1);
        x.set16(0x14, 0x400);
        x.set16(0x16, (-0x88i16) as u16);
        if x.u8(f::FLIP) != 0 {
            negate(x, 0x14);
            negate(x, 0x16);
        }
        return body_anim(x, 9);
    }
    x.move_x();
    if (x.u8(0x14) as i8) < 0 {
        return stop(x);
    }
    if !lib::step(x) {
        return;
    }
    x.set32(0x58, 0);
    x.set32(f::STATE, 0x0204_0400);
    x.set8(INVULNERABLE, 6);
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
            if var::PAUSE.get(x.b) != 0 || sub16(x, DOWN_TIME, 1).gt() {
                return;
            }
            x.set16(DOWN_TIME, 0);
            next6(x);
            x.set8(0x86, 0);
            x.set8(0x49, 0);
            // 0x80cb0: counterattack 6 chances in 16
            let r = rand_and(x, 0xf);
            if 0xfc00u32 >> r & 1 == 0 {
                return;
            }
            let r = rand_and(x, 0xf);
            let k = x.rom8(0x80cf6 + r);
            x.set8(f::SUB2, k);
            x.set16(f::STATE, 0x0204);
            x.set8(0x7, 0);
            x.set8(0x86, 0);
            x.set8(VULNERABLE, 1);
            x.set8(INVULNERABLE, 6);
            x.set8(0x38, 0);
            x.set8(0, 1);
        }
        _ => {
            if var::PAUSE.get(x.b) != 0 || !lib::step(x) {
                return;
            }
            x.set8(VULNERABLE, 1);
            x.set8(0, 1);
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
            x.set_a5_u8(0x42, 1);
            x.set_a5_u8(0x50, 0);
            crate::mem::wr16(x.b, A5_DEATH, 0x300);
            let score = x.u8(0x7e);
            set_abs8(x, A5_DEATH + 2, score);
            lib::drop_rewards(x);
            crate::sound::service(x.b, crate::sound::Service::Ff05);
            lib::sound(x, crate::sound::CMD_FF);
            lib::flash_and_shake(x);
            x.set8(0, 2);
            let f = x.u8(0x56);
            x.set8(f::FREEZE, f);
            stop(x);
            x.set16(0x18, 0x400);
            x.set16(0x1a, (-0x40i16) as u16);
            let mut v = 0x200u16;
            if x.u8(f::FLIP) != 0 {
                v = v.wrapping_neg();
            }
            x.set16(0x14, v);
            x.anim_start(SET_DEATH, 2);
            falling(x)
        }
        2 => falling(x),
        4 => {
            x.move_x();
            if !x.anim_step() {
                return;
            }
            next6(x);
            x.anim_start(SET_DEATH, 0x16);
        }
        _ => {}
    }
}

/// 0x80da2: fall to the ground, then bounce.
fn falling<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
    x.move_xz();
    if (x.u16(0xc) as i16) >= 0 {
        return;
    }
    x.set16(0xc, 0);
    let v = ((x.u16(0x14) as i16) >> 2) as u16;
    x.set16(0x14, v);
    x.set32(0x1c, 0);
    x.set32(0x18, 0);
    next6(x);
    x.anim_start(SET_DEATH, 0x10);
    lib::sound(x, crate::sound::CMD_AA);
}

/// Defeated boss: type and variant (word), score (`A5-0x557c..`), read by the end of the stage.
const A5_DEATH: u32 = 0xff8000 - 0x557c;
