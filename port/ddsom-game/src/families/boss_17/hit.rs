//! Boss 17, state 4 "hit" (0x96990). `+5` groups:
//!
//! | `+5` | Role |
//! |---|---|
//! | 0 | reaction to the hit: death if out of HP, otherwise routine from table 0x96a0c (code `+0x49`) |
//! | 2 | knockback (pushed back, `+6` = 0 to 4) |
//! | 4 | getting up after a fall (`+6` = 0 to 6) |
//! | 6, 8 | death: fall, flight, end (`+6` = 0 to 10) |
//! | 10 | end of getting up (resumes the middle of `+5` = 4) |

use crate::families::cx::Bench;
use super::decide;
use super::*;
use crate::objects::pools;
use crate::objects::Current;
use crate::objects::Obj;

/// Table of reaction routines, indexed by the hit code `+0x49` (longs).
const REACTIONS: u32 = 0x96a0c;
/// The only routine specific to the boss (code 0x4c): stun with trembling.
const STUN_REACTION: u32 = 0x96ab8;
/// HP thresholds per phase (words, 128 bytes per row).
const THRESHOLDS_A: u32 = 0x10ee2;
const THRESHOLDS_B: u32 = 0x110e2;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else {
        match x.u8(f::SUB) {
            0 => reaction(x),
            2 => pushed_back(x),
            4 => get_up(x),
            6 | 8 => down(x),
            10 => back_on_feet(x),
            _ => {}
        }
    }
    lib::collide_terrain(x);
}

/// 0x975cc: out of HP? Then moves to group 6 (death) and returns true.
fn out_of_hp<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if tst16(x, f::HP).gt() {
        return false;
    }
    x.set8(f::SUB, 6);
    x.set8(0, 2);
    true
}

fn reaction<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB2) == 0 {
        if x.u8(SLOW) != 0 {
            let mut add = x.u8(STUN);
            if x.u8(SLOW) == 2 {
                add = add.wrapping_add(add);
            }
            let v = x.u8(STUN).wrapping_add(add);
            x.set8(STUN, v);
        }
        x.set8(INVULNERABLE, 0);
        set_abs8(x, 0xff8043, 0xff);
        x.set_a5_u16(0x51e, 0);
        lib::set_68(x);
        if x.u8(Obj::VARIANT) == 0 && tst16(x, f::HP).le() {
            return decide::start_death(x);
        }
        if out_of_hp(x) {
            return down(x);
        }
    }
    if reaction_routine(x, REACTIONS) == STUN_REACTION {
        return stun(x);
    }
    react(x, REACTIONS);
}

/// 0x96ab8: stun: the boss trembles in place then sets off again.
fn stun<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(0, 2);
            let px = x.u16(0x8);
            x.set16(0xa, px);
            x.set16(0x84, 0);
            let stun = x.u8(STUN);
            x.set8(f::FREEZE, stun);
            x.set8(0x90, 0x10);
            lib::start(x, SET_HURT, 8);
        }
        2 => {
            if tst8(x, 0x90).le() {
                let px = x.u16(0xa);
                x.set16(0x8, px);
                x.set8(0, 1);
                x.set32(f::STATE, 0x0404_0200);
                x.set32(0x58, 0);
                lib::step(x);
                return;
            }
            sub8(x, 0x90, 1);
            let mut d = 2i16;
            if x.u8(0x90) & 1 != 0 {
                d = -d;
            }
            if x.u8(f::FLIP) != 0 {
                d = -d;
            }
            x.add16(0x8, d as u16);
        }
        _ => {}
    }
}

// --- +5 = 2 : recul -----------------------------------------------------------------------------

fn pushed_back<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.set16(TIMER, 0x14);
            let (vx, vz) = if x.u8(f::FLIP) == 0 { (0x400i16, -0x30i16) } else { (-0x400, 0x30) };
            x.set16(0x14, vx as u16);
            x.set16(0x16, vz as u16);
            x.set8(ANIM_KIND, 0x20);
            x.set8(ANIM_LAST, 0x20);
            x.add8(f::SUB2, 2);
            lib::sound(x, crate::sound::CMD_126_226_B);
            body(x, 0xb);
        }
        2 => {
            if sub16(x, TIMER, 1).le() {
                x.add8(f::SUB2, 2);
                x.set16(0x16, 0);
                let n = if cmp8(x, PHASE, 2).ge() { 0xd } else { 0xc };
                body(x, n);
            } else {
                x.move_x();
                lib::step(x);
            }
        }
        4 => {
            if !lib::step(x) {
                return;
            }
            x.set32(0x58, 0);
            if cmp8(x, PHASE, 2).ge() {
                decide::attack_4_shielded(x);
            } else {
                decide::chase(x);
            }
        }
        _ => {}
    }
}

// --- +5 = 4: getting up ----------------------------------------------------------------------------

fn get_up<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(ANIM_LAST, 0xff);
            x.anim_start(SET_BODY, 0xe);
        }
        2 => {
            if var::PAUSE.get(x.b) as i8 > 0 {
                return;
            }
            if sub16(x, 0x84, 1).gt() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(VULNERABLE, 1);
            x.set8(0x86, 0);
            x.set8(0x49, 0);
            x.set16(0x16, 0);
            x.set16(0x1e, 0);
            let mut edge = camera_x(x);
            let mut angle = 8;
            edge = edge.wrapping_add(0x30);
            if !Fl::sub16(edge, x.u16(0x8)).ge() {
                angle = 0x18;
                edge = edge.wrapping_add(0x120);
                if !Fl::sub16(edge, x.u16(0x8)).le() {
                    // between the two edges: no about-face
                    x.add8(f::SUB2, 2);
                    return;
                }
            }
            x.set8(Obj::SPEED_INDEX, angle);
            lib::drift_heading(x);
            speed(x, 0x2b);
            x.set16(TIMER, 0x1e);
            // the animation depends on the boss's direction relative to the chosen edge
            let wrong = (x.u8(f::FLIP) as i8 as i16 as u16).rotate_right(6) != 0;
            body_hurt(x, if wrong { 0xf } else { 0x10 });
        }
        4 => {
            if var::PAUSE.get(x.b) != 0 {
                return;
            }
            if sub16(x, TIMER, 1).le() {
                x.add8(f::SUB2, 2);
                let side = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, side);
                body(x, 0x11);
            } else {
                x.move_xy();
                lib::step(x);
            }
        }
        6 => {
            if var::PAUSE.get(x.b) != 0 {
                return;
            }
            if !x.anim_step() {
                return;
            }
            x.add8(STRIKES, 1);
            recovered_tail(x);
        }
        _ => {}
    }
}

fn body_hurt<R: Bench>(x: &mut Cx<'_, R>, n: u32) {
    body(x, n);
}

/// 0x96ee6: end of getting up (without counting the hit).
fn back_on_feet<R: Bench>(x: &mut Cx<'_, R>) {
    recovered_tail(x);
}

/// 0x96ce0: the boss is standing; chooses what follows according to its phase.
pub(crate) fn recovered_tail<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0, 1);
    x.set8(ANIM_LAST, 0xff);
    if !pick_target(x) {
        return decide::wait(x);
    }
    let thresholds = if x.u8(Obj::VARIANT) == 0 { THRESHOLDS_A } else { THRESHOLDS_B };
    if lib::next_threshold(x, thresholds) {
        if x.u8(Obj::VARIANT) == 0 {
            if cmp8(x, PHASE, 3).ge() {
                return decide::start_death(x);
            }
            return decide::attack_14(x);
        }
        if x.u8(Obj::VARIANT) == 2 && (var::PLAYERS_ALIVE.get(x.b) as i8) > 2 && cmp8(x, PHASE, 2).eq() {
            if let Some(o) = x.alloc(&pools::SET) {
                o.set_u8(x.b, 0, 1);
                o.set_u8(x.b, 2, 2);
                o.set_u8(x.b, 3, 0x6f);
            }
        }
    }
    let mut limit = 2;
    if (var::PLAYERS_ALIVE.get(x.b) as i8) > 2 {
        limit = 0;
    }
    if !Fl::sub8(limit, x.u8(PHASE)).gt() {
        if x.u8(SLOW) != 0 {
            return decide::chase(x);
        }
        if Fl::sub8(x.u8(STRIKE_LIMIT), x.u8(STRIKES)).lt() {
            return decide::attack_16(x);
        }
    }
    if lib::target_in_box(x, 0, 0x20, 0xc) {
        decide::attack_4_shielded(x);
    } else {
        decide::chase(x);
    }
}

// --- +5 = 6, 8 : mort ---------------------------------------------------------------------------

fn down<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            if x.u8(Obj::VARIANT) == 0 {
                return decide::start_death(x);
            }
            x.add8(f::SUB2, 2);
            x.set_a5_u8(0x42, 1);
            x.set_a5_u8(0x50, 0);
            let kind = x.u8(0x2);
            set_abs8(x, 0xff2a84, kind);
            set_abs8(x, 0xff2a85, 0);
            let bar = x.u8(0x7e);
            set_abs8(x, 0xff2a86, bar);
            x.set32(f::IMMUNITY, 0xffff_ffff);
            lib::flash_and_shake(x);
            crate::sound::service(x.b, crate::sound::Service::Ff05);
            lib::sound(x, crate::sound::CMD_FF);
            lib::sound(x, crate::sound::CMD_83_83);
            lib::drop_rewards(x);
            x.set8(0, 2);
            x.set8(SLOW, 0);
            x.set16(0x16, 0);
            x.set16(0x18, 0x500);
            x.set16(0x1a, (-0x38i16) as u16);
            let mut vx: i16 = 0x200;
            let mut side = 0;
            let mut edge = camera_x(x).wrapping_add(0xc0);
            if !Fl::sub16(edge, x.u16(0x8)).ge() {
                vx = -vx;
                side = 0x20;
            }
            edge = 0;
            let _ = edge;
            x.set16(0x14, vx as u16);
            x.set8(f::FLIP, side);
            x.move_xz();
            x.anim_start(SET_BODY, 0x12);
        }
        2 => {
            x.move_xz();
            if cmp16(x, 0x18, 0x100).gt() {
                x.anim_step();
            } else {
                x.add8(f::SUB2, 2);
                x.anim_start(SET_BODY, 0x13);
            }
        }
        4 => {
            x.move_xz();
            if cmp16(x, 0x18, 0xff00).gt() {
                x.anim_step();
            } else {
                x.add8(f::SUB2, 2);
                x.anim_start(SET_BODY, 0x14);
            }
        }
        6 => {
            x.move_xz();
            if tst16(x, 0xc).gt() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(0xc, 0);
            lib::sound(x, crate::sound::CMD_AA);
            x.anim_start(SET_BODY, 0x15);
        }
        8 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
            }
        }
        _ => {}
    }
}
