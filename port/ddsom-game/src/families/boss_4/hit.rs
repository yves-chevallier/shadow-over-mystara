//! Boss 4, state 4 "hit" (0x82c36). `+5` groups:
//!
//! | `+5` | Role |
//! |---|---|
//! | 0 | reaction to the hit: death if out of HP, otherwise routine from table 0x82c8a (code `+0x49`) |
//! | 2 | knockback, then crossing in flight (counterattack) |
//! | 4 | on the ground, then getting up |
//! | 6, 8 | death: thrown, falls into the dust |
//! | 10 | return to combat |

use super::decide;
use crate::families::cx::Bench;
use super::*;
use crate::objects::Current;

/// Table of reaction routines, indexed by the hit code `+0x49` (longs).
const REACTIONS: u32 = 0x82c8a;
/// The only routine specific to the boss (code 0x4c): shaking in place.
const SHAKE_REACTION: u32 = 0x82d36;
/// HP thresholds per phase (words, 128 bytes per row).
const THRESHOLDS: u32 = 0x10ce2;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else {
        match x.u8(f::SUB) {
            0 => reaction(x),
            2 => pushed_back(x),
            4 => down(x),
            6 | 8 => death(x),
            _ => resume(x),
        }
    }
    lib::keep_in_zone(x);
    lib::collide_terrain(x);
}

/// 0x82c68: first pass: damage (0x10112) and, with no HP left, death; then the hit code's routine.
fn reaction<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        lib::set_68(x);
        x.set8(INVULNERABLE, 0);
        // 0x833fc
        if !tst16(x, f::HP).gt() {
            x.set8(f::SUB, 6);
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

/// 0x82d36: shaking in place (16 frames, ±2 pixels), then back to the knockback (state 4, group 4, step 2).
fn shake<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        x.set8(0, 2);
        let px = x.u16(0x8);
        x.set16(0xa, px);
        x.set16(HITS, 0);
        let stun = x.u8(STUN);
        x.set8(f::FREEZE, stun);
        x.set8(SHAKE, 0x10);
        x.anim_start(SET_HURT, 8);
        return;
    }
    if tst8(x, SHAKE).gt() {
        sub8(x, SHAKE, 1);
        let mut d = 2u32;
        if x.bit(SHAKE, 0) {
            d = d & 0xffff_0000 | (d as u16).wrapping_neg() as u32;
        }
        if x.u8(f::FLIP) != 0 {
            d = d & 0xffff_0000 | (d as u16).wrapping_neg() as u32;
        }
        x.add16(0x8, d as u16);
        return;
    }
    let px = x.u16(0xa);
    x.set16(0x8, px);
    x.set8(0, 1);
    x.set32(f::STATE, 0x0404_0200);
    x.set32(0x58, 0);
}

/// 0x82dae: knockback (0x14 frames backward, then 0x14 in place), then the crossing in flight.
fn pushed_back<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.set16(TIMER, 0x14);
            let (angle, vy) = if x.u8(f::FLIP) != 0 { (0x18u8, 0x20u16) } else { (8, (-0x20i16) as u16) };
            x.set8(Obj::SPEED_INDEX, angle);
            x.set16(0x16, vy);
            speed(x, 0x17);
            kind(x, 0x20);
            x.add8(f::SUB2, 2);
            x.anim_start(SET_BODY, 9);
        }
        2 => {
            if expired(x, TIMER) {
                x.add8(f::SUB2, 2);
                x.set16(TIMER, 0x14);
                x.set16(0x16, 0);
            }
            x.move_x();
            x.anim_step();
        }
        _ => {
            if !expired(x, TIMER) {
                x.move_x();
                x.anim_step();
                return;
            }
            x.set32(0x58, 0);
            decide::swoop(x);
        }
    }
}

/// 0x82e40: on the ground (`+0x84` frames outside pause), then getting up.
fn down<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            lib::behaviour_by_kind(x);
            x.set8(ANIM_LAST, 0xff);
            x.anim_start(SET_BODY, 0xa);
        }
        2 => {
            if (var::PAUSE.get(x.b) as i8) > 0 || sub16(x, HITS, 1).gt() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(VULNERABLE, 1);
            x.set8(0x86, 0);
            x.set8(0x49, 0);
        }
        _ => {
            if var::PAUSE.get(x.b) != 0 || !x.anim_step() {
                return;
            }
            resume(x);
        }
    }
}

/// 0x82ea0: return to combat (also called by the common reactions, table 0x3e000): crosses the HP threshold,
/// then crossing in flight if a player is very close, otherwise the walk.
pub(crate) fn resume<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0, 1);
    x.set8(ANIM_LAST, 0xff);
    if var::GAME_ON.get(x.b) == 0 || !pick_target(x) {
        return decide::wait(x);
    }
    lib::next_threshold(x, THRESHOLDS);
    if lib::target_in_box(x, 0, 0x30, 0xc) {
        decide::swoop(x);
    } else {
        decide::walk(x);
    }
}

/// 0x82ee2: death. Thrown into the air toward the middle of the screen, the boss falls back into the dust; the end of
/// the stage is started on the first pass.
fn death<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set_a5_u8(0x42, 1);
            x.set_a5_u8(0x50, 0);
            let kind = x.u8(2);
            let points = x.u8(0x7e);
            let a5 = crate::mem::A5;
            crate::mem::wr8(x.b, a5.wrapping_sub(0x557c), kind);
            crate::mem::wr8(x.b, a5.wrapping_sub(0x557b), 0);
            crate::mem::wr8(x.b, a5.wrapping_sub(0x557a), points);
            lib::flash_and_shake(x);
            crate::sound::service(x.b, crate::sound::Service::Ff05);
            lib::sound(x, crate::sound::CMD_FF);
            lib::sound(x, crate::sound::CMD_30E);
            lib::drop_rewards(x);
            x.set8(0, 2);
            x.set16(0x16, 0);
            x.set16(0x18, 0x500);
            x.set16(0x1a, (-0x38i16) as u16);
            let mid = var::CAMERA1_X.get(x.b).wrapping_add(0xc0);
            let (vx, side) = if Fl::sub16(mid, x.u16(0x8)).ge() { (0x200u16, 0u8) } else { ((-0x200i16) as u16, 0x20) };
            setw(x, 0, vx);
            x.set16(0x14, vx);
            x.set8(f::FLIP, side);
            x.move_xz();
            x.anim_start(SET_HURT, 0);
        }
        2 => {
            x.move_xz();
            if (x.u16(0xc) as i16) > 0 {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(0xc, 0);
            lib::debris(x, 5);
            lib::sound(x, crate::sound::CMD_35D);
            x.anim_start(SET_BODY, 0xb);
        }
        4 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
            }
        }
        _ => {}
    }
}
