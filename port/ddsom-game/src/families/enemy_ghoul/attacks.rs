//! Type 5, attacks (`+5 = 4`). `+6`: 0 and 2 claw attacks, 4, 10 and 12 leap attacks, 6 resume walking, 8 wait on
//! guard, 14 and 16 fast approach. `+7` is the step.

use crate::families::cx::Bench;
use super::stance::begin_walk;
use super::*;
use crate::objects::Current;
use crate::objects::Obj;

pub(super) fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if special_hit(x) {
        return;
    }
    match x.u8(f::SUB2) {
        0 => claw(x, 0x11, 0),
        2 => claw(x, 0x12, 1),
        4 => leap(x, Leap { kind: Some(0x13), speed: 0x200, rise: 0x200, gravity: 0x38, then: Then::EndAttack }),
        6 => {
            set_state_long(x, 0x0202_0600);
            x.set8(LOOK, 6);
            begin_walk(x);
        }
        8 => guard(x),
        10 => leap(x, Leap { kind: Some(0x14), speed: 0x400, rise: 0x280, gravity: 0x38, then: Then::Walk }),
        12 => leap(x, Leap { kind: None, speed: 0x400, rise: 0x280, gravity: 0x38, then: Then::EndAttack }),
        14 => charge(x),
        16 => feint(x),
        _ => {}
    }
}

/// Claw attack (type `kind`, `anim` animation of the attacks).
fn claw<R: Bench>(x: &mut Cx<'_, R>, kind: u8, anim: u32) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(STRIKE_KIND, kind);
        look(x, 6);
        x.set8(VULNERABLE, 1);
        return strike(x, anim);
    }
    if step(x) {
        end_attack(x);
    }
}

/// What the ghoul does at the end of a leap.
#[derive(Clone, Copy)]
enum Then {
    EndAttack,
    /// Back to walking (0x601a2).
    Walk,
}

/// Parameters of a leap attack: attack type (if set here), X speed, impulse, gravity.
#[derive(Clone, Copy)]
struct Leap {
    kind: Option<u8>,
    speed: i16,
    rise: u16,
    gravity: u16,
    then: Then,
}

fn leap<R: Bench>(x: &mut Cx<'_, R>, l: Leap) {
    match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            if let Some(k) = l.kind {
                x.set8(STRIKE_KIND, k);
            }
            look(x, 4);
            x.set8(TIMER, 4);
            x.set8(VULNERABLE, 1);
            body(x, 3);
        }
        2 => {
            let t = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, t);
            if t != 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set16(Obj::VX, -l.speed as u16);
            if x.u8(f::FLIP) != 0 {
                negate(x, Obj::VX);
            }
            x.set16(Obj::VZ, l.rise);
            x.set16(Obj::AZ, -(l.gravity as i16) as u16);
            x.set16(Obj::AX, 0);
            strike(x, 4);
        }
        4 => {
            if x.move_xz().gt() {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set32(Obj::Z, 0);
            body(x, 5);
        }
        6 => {
            if step(x) {
                match l.then {
                    Then::EndAttack => end_attack(x),
                    Then::Walk => {
                        set_state_long(x, 0x0202_0600);
                        x.set8(LOOK, 6);
                    }
                }
            }
        }
        _ => {}
    }
}

/// 8 : garde (animation 3) pendant `0x78` images.
fn guard<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        look(x, 6);
        x.set8(VULNERABLE, 1);
        x.set8(TIMER, 0x78);
        return strike(x, 3);
    }
    step(x);
    let t = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, t);
    if t == 0 {
        end_attack(x);
    }
}

/// Start of an approach: walks forward (animation 2) for `0x18` frames.
fn begin_charge<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    look(x, 6);
    x.set8(VULNERABLE, 1);
    x.set8(TIMER, 0x18);
    set_angle(x, 8, 0x18);
    set_speed(x, 0xd);
    body(x, 2);
}

/// 14: approach, then a new choice (or an immediate attack).
fn charge<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(f::SUB3) == 0 {
        return begin_charge(x);
    }
    let t = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, t);
    if t == 0 {
        end_attack(x);
        if box_attack(x, 0, 0x50, 8, CLOSE_ATTACKS, 0) || box_attack(x, -0x80, 0x30, 8, FAR_ATTACKS, 0xa) {
            return;
        }
    }
    move_xy(x);
    step(x);
}

/// 16: approach then feint move (claw or animation 3 of the attacks).
fn feint<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::SUB3) {
        0 => begin_charge(x),
        2 => {
            let t = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, t);
            if t == 0 {
                x.add8(f::SUB3, 2);
                x.set8(TIMER, 0x10);
                if rand_and(x, 1) == 0 {
                    return body(x, 0);
                }
                return strike(x, 3);
            }
            move_xy(x);
            step(x);
        }
        4 => {
            let t = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, t);
            if t == 0 {
                end_attack(x);
            } else {
                step(x);
            }
        }
        _ => {}
    }
}
