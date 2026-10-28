//! Type 19, seconde série de coups de la bête (sous-état `+5 = 6`, 0x739b2) : bonds et sauts. Mêmes
//! étapes communes que `attacks` (préparation, regard), reprise après demi-tour en sous-état 6.

use super::attacks::{phased, pounce_repeat, roll_pounces};
use super::parts::{bound_land, bound_wait, bound_y_land, bound_y_wait};
use super::stance::pursue_tail;
use super::*;

const RESUME_ATTACK_B: u32 = 0x0206_0000;

pub(super) fn run(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => hop_or_pounce(x),
        2 => roar(x),
        4 => side_leap(x),
        6 => short_leap(x),
        8 => leap_and_claw(x),
        10 => long_leap(x),
        12 => retreat(x),
        _ => {}
    }
}

/// Retour à la bête debout, puis suite de la poursuite (0x73a4a).
fn end_and_pursue(x: &mut Cx) {
    if x.anim_step() {
        set_state_long(x, ACTIVE_STANCE);
        x.set8(WANT_ANIM, 0);
        play(x, 0);
        pursue_tail(x);
    }
}

/// 0x739cc : au hasard un petit saut vers l'avant, sinon une série de sauts sur place.
fn hop_or_pounce(x: &mut Cx) {
    phased(x, RESUME_ATTACK_B, |x, n| match n {
        4 => {
            if random(x) & 1 != 0 {
                set_state_long(x, 0x0202_0002);
                x.set8(WANT_ANIM, 0xc);
                play(x, 8);
                return pursue_tail(x);
            }
            x.add8(0x7, 4);
            x.set8(VULNERABLE, 1);
            x.set8(WANT_ANIM, 7);
            roll_pounces(x);
            play(x, 7);
        }
        6 => end_and_pursue(x),
        8 => pounce_repeat(x),
        _ => {}
    });
}

/// 0x73a78 : rugit.
fn roar(x: &mut Cx) {
    phased(x, RESUME_ATTACK_B, |x, n| match n {
        4 => {
            x.add8(0x7, 2);
            x.set8(VULNERABLE, 1);
            x.set8(WANT_ANIM, 0xf);
            play(x, 0xb);
        }
        6 => end_and_pursue(x),
        _ => {}
    });
}

/// 0x73aae : bond de côté (en profondeur), vers le haut ou le bas au hasard.
fn side_leap(x: &mut Cx) {
    phased(x, RESUME_ATTACK_B, |x, n| match n {
        4 => {
            x.add8(0x7, 2);
            let vy = if random(x) & 1 != 0 { -0x100i16 } else { 0x100 };
            x.set16(0x1c, vy as u16);
            x.set8(VULNERABLE, 1);
            x.set8(WANT_ANIM, 6);
            x.set8(CUR_ANIM, 6);
            x.set16(0x1e, 0);
            x.set16(0x18, 0x400);
            x.set16(0x1a, (-0x38i16) as u16);
            play(x, 6);
        }
        6 => bound_y_wait(x),
        8 => bound_y_land(x),
        10 => end_and_pursue(x),
        _ => {}
    });
}

/// Début d'un bond en X de la seconde série (0x73b30) : vitesse `vx`, hauteur `vz`.
fn leap_start<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>, vx: i16, vz: u16, anim: Option<u8>) {
    x.add8(0x7, 2);
    x.set8(VULNERABLE, 1);
    x.set8(WANT_ANIM, 0x1c);
    x.set8(CUR_ANIM, 0x1c);
    let vx = if x.u8(f::FLIP) != 0 { vx.wrapping_neg() } else { vx };
    set_velocity(x, vx, vz, -0x38);
    match anim {
        None => play(x, 0x11),
        Some(n) => {
            x.set8(STRIKE_POWER, 6);
            x.set8(STRIKE_KIND, 2);
            play_b(x, n);
        }
    }
}

/// 0x73b18 : petit bond en avant.
fn short_leap(x: &mut Cx) {
    phased(x, RESUME_ATTACK_B, |x, n| match n {
        4 => leap_start(x, 0x80, 0x2c0, None),
        6 => bound_wait(x),
        8 => bound_land(x),
        10 => end_and_pursue(x),
        _ => {}
    });
}

/// 0x73b7a : petit bond puis coup de griffe.
fn leap_and_claw(x: &mut Cx) {
    phased(x, RESUME_ATTACK_B, |x, n| match n {
        4 => leap_start(x, 0x80, 0x2c0, None),
        6 => bound_wait(x),
        8 => bound_land(x),
        10 => {
            if x.anim_step() {
                x.add8(0x7, 2);
                x.set8(STRIKE_POWER, 5);
                x.set8(STRIKE_KIND, 3);
                x.set8(WANT_ANIM, 0x28);
                play_b(x, 0);
            }
        }
        12 => end_and_pursue(x),
        _ => {}
    });
}

/// 0x73bec : grand bond griffu.
fn long_leap(x: &mut Cx) {
    phased(x, RESUME_ATTACK_B, |x, n| match n {
        4 => leap_start(x, 0x280, 0x2c0, Some(5)),
        6 => bound_wait(x),
        8 => bound_land(x),
        10 => end_and_pursue(x),
        _ => {}
    });
}

/// 0x73c5a : recule en rugissant puis se retourne, à terre.
fn retreat<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        return play(x, 0x18);
    }
    if x.anim_step() {
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        set_state_long(x, KNOCKED);
    }
}
