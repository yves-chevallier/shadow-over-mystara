//! Boss 7, groupes 6 et 8 : esquives (disparition et réapparition, bond en arrière, marche courte), lancées par
//! `aids::after_attack` et les coups encaissés. Le groupe 6 tire deux variantes de la réapparition selon `+6`, le
//! groupe 8 en a quatre.

use super::aids;
use super::attacks::{leap_rest, leap_start};
use super::*;
use crate::objects::Courant;

fn step(x: &mut Cx, table: u32) -> u32 {
    dispatch(x, table, f::SUB3)
}

/// 0x8699e : le boss s'efface (début commun des esquives).
fn fade_out(x: &mut Cx) {
    addq8(x, f::SUB3, 2);
    x.set8(0, 2);
    key(x, 0x12);
    lib::sound(x, 0xecf7a);
    body(x, 4);
}

/// 0x86a06 : le boss reparaît quand le décompte est écoulé.
fn fade_in(x: &mut Cx) {
    if timer_done(x) {
        addq8(x, f::SUB3, 2);
        x.set_bit(0x1, 1, false);
        x.set8(0, 1);
        lib::sound(x, 0xecf7a);
    }
}

/// Fin des esquives : joue l'animation, puis passe à `state`.
fn finish(x: &mut Cx, state: u32) {
    if x.anim_step() {
        x.set32(f::STATE, state);
    }
}

/// Réapparition : le boss est invisible (`bset 1`) 8 images, animation 5.
fn reappear(x: &mut Cx) {
    x.set_bit(0x1, 1, true);
    x.set16(TIMER, 8);
    body(x, 5);
}

/// 0x869c6 : réapparition un peu en arrière (0x60 pixels), dans l'aire de jeu.
fn back_off(x: &mut Cx) {
    if x.anim_step() {
        addq8(x, f::SUB3, 2);
        let mut d = (-0x60i16) as u16;
        if x.u8(f::FLIP) != 0 {
            d = d.wrapping_neg();
        }
        x.add16(0x8, d);
        aids::clamp(x);
        reappear(x);
    }
}

/// 0x86a4e : réapparition à un point tiré selon le quart d'écran.
fn jump_away(x: &mut Cx) {
    if x.anim_step() {
        addq8(x, f::SUB3, 2);
        aids::dodge_spot(x);
        reappear(x);
    }
}

/// 0x868a4 (`+5` = 6).
pub(super) fn group6(x: &mut Cx) {
    let table = if dispatch(x, 0x868b2, f::SUB2) == 0x868ba { 0x868c8 } else { 0x868de };
    match step(x, table) {
        0x8699e => fade_out(x),
        0x869c6 => back_off(x),
        0x86a4e => jump_away(x),
        0x86a06 => fade_in(x),
        _ => finish(x, FIGHT),
    }
}

/// 0x868e6 (`+5` = 8).
pub(super) fn group8(x: &mut Cx) {
    match dispatch(x, 0x868f4, f::SUB2) {
        0x868fe => short_walk(x),
        0x86988 => match step(x, 0x86996) {
            0x8699e => fade_out(x),
            0x869c6 => back_off(x),
            0x86a06 => fade_in(x),
            _ => finish(x, 0x0204_0a00),
        },
        0x86a38 => match step(x, 0x86a46) {
            0x8699e => fade_out(x),
            0x86a4e => jump_away(x),
            0x86a06 => fade_in(x),
            _ => finish(x, FIGHT),
        },
        // bond en arrière : les étapes du bond d'attaque, mais il finit sur le combat (0x86a7c)
        _ => match step(x, 0x86a9e) {
            0x86aaa => leap_start(x, 0x400),
            0x86a7c => finish(x, FIGHT),
            t => leap_rest(x, t),
        },
    }
}

/// 0x868fe (`+6` = 0) : quelques pas sur place (0x14 images) en surveillant l'adversaire.
fn short_walk(x: &mut Cx) {
    match step(x, 0x8690c) {
        0x86950 => {
            if timer_done(x) {
                x.set32(f::STATE, FIGHT);
                return;
            }
            if aids::must_turn(x) {
                x.set32(RESUME_STATE, 0x0208_0004);
                let b6 = x.u8(f::SUB2);
                x.set8(RESUME_SUB, b6);
                x.set32(f::STATE, 0x0202_0200);
                return;
            }
            x.anim_step();
        }
        t => {
            if t == 0x8691a {
                addq8(x, f::SUB3, 2);
                x.set16(TIMER, 0x14);
            } else {
                x.add8(f::SUB3, 0xfe);
            }
            x.set8(VULNERABLE, 1);
            x.set8(ANIM_KIND, 0);
            setb(x, 2, 0);
            if x.u8(ANIM_LAST) != 0 {
                x.set8(ANIM_LAST, 0);
            }
            body(x, 0);
        }
    }
}
