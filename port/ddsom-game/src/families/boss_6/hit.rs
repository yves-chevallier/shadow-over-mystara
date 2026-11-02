//! Boss 6, state 4 "hit" (0x85222). `+5` groups:
//!
//! | `+5` | Role |
//! |---|---|
//! | 0 | reaction to the hit: death if out of HP, otherwise routine from table 0x8527a (code `+0x49`) |
//! | 2 | knockback |
//! | 4 | getting up |
//! | 6, 8 | death: fall, landing, blinking, release |
//! | 10 | resumption |

use crate::families::cx::Bench;
use super::decide;
use super::*;
use crate::objects::pools;
use crate::objects::Current;
use crate::objects::Obj;

const REACTIONS: u32 = 0x8527a;
/// The only routine of the table specific to the boss (code 0x4c): stun with trembling.
const STUN_REACTION: u32 = 0x85326;
/// HP thresholds per phase (words).
const THRESHOLDS: u32 = 0x10de2;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else {
        match x.u8(f::SUB) {
            0 => reaction(x),
            2 => pushed_back(x),
            4 => get_up(x),
            6 | 8 => down(x),
            10 => recovered_tail(x),
            _ => {}
        }
    }
    lib::keep_in_zone(x);
    lib::collide_terrain(x);
}

/// 0x859d8: out of HP? Then moves to group 6 (death).
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
        lib::drop_once(x);
        lib::set_68(x);
        x.set8(INVULNERABLE, 0);
        if out_of_hp(x) {
            return down(x);
        }
    }
    if reaction_routine(x, REACTIONS) == STUN_REACTION {
        return stun(x);
    }
    react(x, REACTIONS);
}

/// 0x85326: stun: the boss trembles in place then sets off again.
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

fn pushed_back<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.set16(TIMER, 0x14);
            let (angle, vz) = if x.u8(f::FLIP) == 0 { (8, -0x20i16) } else { (0x18, 0x20) };
            x.set8(Obj::SPEED_INDEX, angle);
            x.set16(0x16, vz as u16);
            speed(x, 0x17);
            x.set8(ANIM_KIND, 0x20);
            x.set8(ANIM_LAST, 0x20);
            x.add8(f::SUB2, 2);
            body(x, 9);
        }
        2 => {
            if !sub16(x, TIMER, 1).gt() {
                x.add8(f::SUB2, 2);
                x.set16(TIMER, 0x14);
                x.set16(0x16, 0);
            }
            x.move_x();
            lib::step(x);
        }
        4 => {
            if sub16(x, TIMER, 1).le() {
                x.set32(0x58, 0);
                decide::attack_6_shielded(x);
            } else {
                x.move_x();
                lib::step(x);
            }
        }
        _ => {}
    }
}

fn get_up<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            lib::behaviour_by_kind(x);
            x.set8(ANIM_LAST, 0xff);
            x.anim_start(SET_BODY, 0xa);
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
        }
        4 => {
            if var::PAUSE.get(x.b) != 0 {
                return;
            }
            if x.anim_step() {
                recovered_tail(x);
            }
        }
        _ => {}
    }
}

/// 0x85490: the boss is standing; it continues in melee or walks.
pub(crate) fn recovered_tail<R: Bench>(x: &mut Cx<'_, R>) {
    x.set8(0, 1);
    x.set8(ANIM_LAST, 0xff);
    if !pick_target(x) {
        return decide::wait(x);
    }
    lib::next_threshold(x, THRESHOLDS);
    if lib::target_in_box(x, 0, 0x30, 0xc) {
        decide::attack_6_shielded(x);
    } else {
        decide::chase(x);
    }
}

fn down<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set32(f::IMMUNITY, 0xffff_ffff);
            x.set_a5_u8(0x42, 1);
            x.set_a5_u8(0x50, 0);
            let kind = x.u8(0x2);
            set_abs8(x, 0xff2a84, kind);
            set_abs8(x, 0xff2a85, 0);
            let bar = x.u8(0x7e);
            set_abs8(x, 0xff2a86, bar);
            lib::flash_and_shake(x);
            crate::sound::service(x.b, crate::sound::Service::Ff05);
            lib::sound(x, crate::sound::CMD_FF);
            lib::sound(x, crate::sound::CMD_30E_40E);
            lib::drop_rewards(x);
            x.set8(0, 2);
            x.set16(0x16, 0);
            x.set16(0x18, 0x500);
            x.set16(0x1a, (-0x38i16) as u16);
            let mut vx: i16 = 0x200;
            let mut side = 0;
            let edge = var::CAMERA1_X.get(x.b).wrapping_add(0xc0);
            *x.c.d2() = *x.c.d2() & 0xffff_0000 | edge as u32; // D2 re-read afterwards (DDSOM_POISON : cov_tour1_c0, cov_tour5_c4)
            if !Fl::sub16(edge, x.u16(0x8)).ge() {
                vx = -vx;
                side = 0x20;
            }
            x.set16(0x14, vx as u16);
            x.set8(f::FLIP, side);
            lib::flash_and_shake(x);
            x.anim_start(SET_HURT, 0);
        }
        2 => {
            x.move_xz();
            if tst16(x, 0xc).gt() {
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
                if x.u8(Obj::VARIANT) != 0 {
                    x.set16(TIMER, 0x1e);
                }
            }
        }
        6 => {
            if x.u8(Obj::VARIANT) == 0 || sub16(x, TIMER, 1).gt() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(TIMER, 0x28);
        }
        8 => {
            if sub16(x, TIMER, 1).le() {
                x.free(&pools::BOSS);
            } else {
                let b = x.u8(0x1) ^ 0x2;
                x.set8(0x1, b);
            }
        }
        _ => {}
    }
}
