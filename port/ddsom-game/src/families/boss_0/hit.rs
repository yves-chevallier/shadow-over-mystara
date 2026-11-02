//! Boss 0, state 4 "hit" (0x7a344) and return to combat. `+5` groups:
//!
//! | `+5` | Role |
//! |---|---|
//! | 0 | reaction to the hit: death if out of HP, otherwise routine from table 0x7a396 (code `+0x49`) |
//! | 2 | knocked back sliding, then counterattack |
//! | 4 | on the ground, getting up |
//! | 6, 8 | death: thrown, falls into the dust |
//! | 10 | return to combat |

use super::*;
use crate::families::cx::Bench;
use crate::objects::Current;
use crate::objects::Obj;

/// Table of reaction routines, indexed by the hit code `+0x49` (longs).
const REACTIONS: u32 = 0x7a396;
/// The only routine specific to the boss (code 0x4c): shaking in place.
const SHAKE_REACTION: u32 = 0x7a442;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else {
        match x.u8(f::SUB) {
            0 => reaction(x),
            2 => pushed_back(x),
            4 => down(x),
            6 | 8 => death(x),
            10 => resting(x),
            _ => {}
        }
    }
    lib::keep_in_zone(x);
    lib::collide_terrain(x);
}

/// 0x7a374: first pass: damage (0x10112) and, with no HP left, death; then the hit code's routine.
fn reaction<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        lib::set_68(x);
        x.set8(INVULNERABLE, 0);
        if !tst16(x, f::HP).gt() {
            x.set8(f::SUB, 6);
            x.set16(f::SUB2, 0);
            x.set8(0, 2);
            return death(x);
        }
    }
    let code = x.u8(0x49) as u32;
    let routine = rom32(x, REACTIONS + code);
    if routine == SHAKE_REACTION {
        shake(x)
    } else {
        run_reaction(x, routine)
    }
}

/// 0x7a442: shaking in place (16 frames), then on the ground.
fn shake<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        next6(x);
        x.set8(0, 2);
        let px = x.u16(0x8);
        x.set16(SAVED_X, px);
        x.set16(DOWN_TIME, 0);
        let freeze = x.u8(0x56);
        x.set8(f::FREEZE, freeze);
        x.set8(SHAKE, 0x10);
        return lib::start(x, SET_HURT, 8);
    }
    if tst8(x, SHAKE).gt() {
        let n = x.u8(SHAKE).wrapping_sub(1);
        x.set8(SHAKE, n);
        let mut d = 2u16;
        if n & 1 != 0 {
            d = d.wrapping_neg();
        }
        if x.u8(f::FLIP) != 0 {
            d = d.wrapping_neg();
        }
        x.add16(0x8, d);
        return;
    }
    let px = x.u16(SAVED_X);
    x.set16(0x8, px);
    x.set8(0, 1);
    x.set32(f::STATE, 0x0404_0200);
    x.set32(0x58, 0);
}

/// 0x7a4ba: knocked back sliding, then counterattack (attack 10, invulnerable).
fn pushed_back<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.set16(TIMER, 0x14);
            let (angle, vz) = if x.u8(f::FLIP) != 0 { (0x18u32, 0x20u16) } else { (8, (-0x20i16) as u16) };
            x.set8(Obj::SPEED_INDEX, angle as u8);
            x.set16(0x16, vz);
            speed(x, 0x17);
            anim_kind(x, 0x20);
            next6(x);
            lib::sound(x, crate::sound::CMD_34F_44F);
            body(x, 0xb);
        }
        2 => {
            if !sub16(x, TIMER, 1).gt() {
                next6(x);
                x.set16(TIMER, 6);
                x.set16(0x16, 0);
            }
            x.move_x();
            lib::step(x);
        }
        _ => {
            if sub16(x, TIMER, 1).le() {
                x.set32(0x58, 0);
                return strike(x, 10, true);
            }
            x.move_x();
            lib::step(x);
        }
    }
}

/// 0x7a552: on the ground (`+0x84` frames outside pause), then getting up.
fn down<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            x.set8(ANIM_LAST, 0xff);
            x.anim_start(SET_BODY, 0x12);
        }
        2 => {
            if (var::PAUSE.get(x.b) as i8) > 0 || sub16(x, DOWN_TIME, 1).gt() {
                return;
            }
            next6(x);
            x.set8(VULNERABLE, 1);
            x.set8(0x86, 0);
            x.set8(0x49, 0);
        }
        _ => {
            if var::PAUSE.get(x.b) != 0 {
                return;
            }
            if x.anim_step() {
                get_up(x);
            }
        }
    }
}

/// 0x7a5ac: return to combat (also called by the common reactions, table 0x3e000): crosses the next HP threshold
/// (0x105b6); the main half transforms at the second threshold; otherwise a hit if the target is very close, or a
/// run.
pub(crate) fn get_up<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0, 1);
    x.set8(ANIM_LAST, 0xff);
    if !pick_target(x) {
        return idle_alone(x);
    }
    if lib::next_threshold(x, THRESHOLDS) && x.u8(3) == 0 && x.u8(TRANSFORMED) == 0 && x.u8(PHASE) == 2 {
        return transform(x);
    }
    if lib::target_in_box(x, 0, 0x38, 0xc) {
        return strike(x, 6, true);
    }
    run_start(x)
}

/// 0x7a5fe: death. While the other half is still alive, only a fall (0x19ad6); the last one to fall notifies the
/// stage that the boss is defeated.
fn death<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.set32(f::IMMUNITY, 0xffff_ffff);
            let mut last = true;
            if x.u16(PARTNER) != 0 {
                let other = x.linked(PARTNER);
                if other.u32(x.b, f::STATE) == x.u32(f::STATE) {
                    x.set8(DONE, 1);
                    last = false;
                } else if other.u8(x.b, DONE) == 0 && other.u16(x.b, f::STATE) != 0x406 {
                    last = false;
                }
            }
            if last {
                x.set_a5_u8(0x42, 1);
                x.set_a5_u8(0x50, 0);
                let kind = x.u8(2);
                set_abs8(x, A5_DEATH, kind);
                set_abs8(x, A5_DEATH + 1, 0);
                let score = x.u8(0x7e);
                set_abs8(x, A5_DEATH + 2, score);
                x.set8(0x93, 1);
                x.set_a5_u8(0x4f, 0xff);
                lib::flash_and_shake(x);
                crate::sound::service(x.b, crate::sound::Service::Ff05);
                lib::sound(x, crate::sound::CMD_FF);
            } else {
                lib::object_event(x);
            }
            lib::drop_rewards(x);
            x.set8(0, 2);
            let freeze = x.u8(0x56);
            x.set8(f::FREEZE, freeze);
            lib::sound(x, crate::sound::CMD_30B_40B);
            x.set16(0x16, 0);
            x.set16(0x18, 0x500);
            x.set16(0x1a, (-0x38i16) as u16);
            let center = camera_x(x).wrapping_add(0xc0);
            let (vx, side) = if Fl::sub16(center, x.u16(0x8)).ge() { (0x200u16, 0u32) } else { ((-0x200i16) as u16, 0x20) };
            x.set16(0x14, vx);
            x.set8(f::FLIP, side as u8);
            x.move_xz();
            next6(x);
            x.anim_start(SET_BODY, 0x13);
        }
        2 => {
            x.move_xz();
            if tst16(x, 0x18).gt() {
                lib::step(x);
                return;
            }
            next6(x);
            x.anim_start(SET_BODY, 0x14);
        }
        4 => {
            x.move_xz();
            if tst16(x, 0xc).gt() {
                return;
            }
            next6(x);
            x.set16(0xc, 0);
            lib::debris(x, 5);
            if x.u8(0x93) != 0 {
                lib::sound(x, crate::sound::CMD_35D);
            } else {
                lib::sound(x, crate::sound::CMD_327_427_B);
            }
            lib::step(x);
        }
        6 => {
            if x.anim_step() {
                next6(x);
            }
        }
        _ => {}
    }
}

/// 0x7a76e: return to combat after a wait (`+5` = 10).
fn resting<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        next7(x);
        return body(x, 0xc);
    }
    if lib::step(x) {
        if !lib::players_alive(x) {
            return idle_alone(x);
        }
        get_up(x);
    }
}
