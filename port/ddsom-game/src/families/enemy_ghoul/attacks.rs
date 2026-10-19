//! Type 5, coups (`+5 = 4`). `+6` : 0 et 2 coups de griffe, 4, 10 et 12 bonds-coups, 6 reprise de la marche,
//! 8 attente en garde, 14 et 16 approche rapide. `+7` est l'étape.

use super::stance::begin_walk;
use super::*;

pub(super) fn run(x: &mut Cx) {
    if special_hit(x, 0x5ff60) {
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

/// Coup de griffe (type `kind`, animation `anim` des coups).
fn claw(x: &mut Cx, kind: u8, anim: u32) {
    if x.u8(STEP) == 0 {
        x.add8(STEP, 2);
        x.set8(STRIKE_KIND, kind);
        look(x, 6);
        x.set8(VULNERABLE, 1);
        return strike(x, anim);
    }
    if step(x) {
        end_attack(x);
    }
}

/// Ce que fait le ghoul à la fin d'un bond.
#[derive(Clone, Copy)]
enum Then {
    EndAttack,
    /// Retour à la marche (0x601a2).
    Walk,
}

/// Paramètres d'un bond-coup : type de coup (s'il est posé ici), vitesse en X, impulsion, pesanteur.
#[derive(Clone, Copy)]
struct Leap {
    kind: Option<u8>,
    speed: i16,
    rise: u16,
    gravity: u16,
    then: Then,
}

fn leap(x: &mut Cx, l: Leap) {
    match x.u8(STEP) {
        0 => {
            x.add8(STEP, 2);
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
            x.add8(STEP, 2);
            x.set16(SPEED_X, -l.speed as u16);
            if x.u8(f::FLIP) != 0 {
                negate(x, SPEED_X);
            }
            x.set16(SPEED_Z, l.rise);
            x.set16(GRAVITY, -(l.gravity as i16) as u16);
            x.set16(SPEED_Y, 0);
            strike(x, 4);
        }
        4 => {
            x.call(sub::MOVE_XZ);
            if x.c.cond(14) {
                return;
            }
            x.add8(STEP, 2);
            x.set32(HEIGHT, 0);
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
fn guard(x: &mut Cx) {
    if x.u8(STEP) == 0 {
        x.add8(STEP, 2);
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

/// Début d'une approche : marche vers l'avant (animation 2) pendant `0x18` images.
fn begin_charge(x: &mut Cx) {
    x.add8(STEP, 2);
    look(x, 6);
    x.set8(VULNERABLE, 1);
    x.set8(TIMER, 0x18);
    set_angle(x, 8, 0x18);
    set_speed(x, 0xd);
    body(x, 2);
}

/// 14 : approche, puis un nouveau choix (ou un coup immédiat).
fn charge(x: &mut Cx) {
    if x.u8(STEP) == 0 {
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

/// 16 : approche puis geste de feinte (griffe ou animation 3 des coups).
fn feint(x: &mut Cx) {
    match x.u8(STEP) {
        0 => begin_charge(x),
        2 => {
            let t = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, t);
            if t == 0 {
                x.add8(STEP, 2);
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
