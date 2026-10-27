//! Boss 8, groupes 6 et 8 : esquives (disparition et réapparition, pas de côté, bond, repli), lancées par `aids::after_attack`
//! et les coups encaissés. Le groupe 6 tire deux variantes de l'étape 2 selon `+6`, le groupe 8 en a cinq.

use super::aids;
use super::attacks::{leap_rest, leap_start};
use super::*;
use crate::objects::Courant;

fn key(x: &mut Cx, kind: u8) {
    x.set8(ANIM_KIND, kind);
    x.set8(ANIM_LAST, kind);
}

fn step(x: &mut Cx, table: u32) -> u32 {
    dispatch(x, table, f::SUB3)
}

/// 0x89030 : le boss s'efface (début commun des esquives).
fn fade_out(x: &mut Cx) {
    addq8(x, f::SUB3, 2);
    x.set8(PHASE_TIMER, 0x10);
    key(x, 0x12);
    lib::sound(x, 0xecf7a);
    body(x, 4);
}

/// 0x8909a : le boss reparaît quand le décompte est écoulé.
fn fade_in(x: &mut Cx) {
    if sub16(x, TIMER, 1).mi() {
        addq8(x, f::SUB3, 2);
        x.set_bit(0x1, 1, false);
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

/// 0x88f36 (`+5` = 6).
pub(super) fn group6(x: &mut Cx) {
    let table = if dispatch(x, 0x88f44, f::SUB2) == 0x88f4c { 0x88f5a } else { 0x88f70 };
    match step(x, table) {
        0x89030 => fade_out(x),
        0x8905a => {
            if x.anim_step() {
                addq8(x, f::SUB3, 2);
                let mut d = (-0x80i16) as u16;
                if x.u8(f::FLIP) != 0 {
                    d = d.wrapping_neg();
                }
                x.add16(0x8, d);
                aids::clamp(x);
                reappear(x);
            }
        }
        0x890de => {
            if x.anim_step() {
                addq8(x, f::SUB3, 2);
                aids::dodge_spot(x);
                reappear(x);
            }
        }
        0x8909a => fade_in(x),
        _ => finish(x, 0x0202_0000),
    }
}

/// 0x88f78 (`+5` = 8).
pub(super) fn group8(x: &mut Cx) {
    match dispatch(x, 0x88f86, f::SUB2) {
        0x88f90 => rest(x),
        0x8901a => match step(x, 0x89028) {
            0x89030 => fade_out(x),
            0x8905a => {
                if x.anim_step() {
                    addq8(x, f::SUB3, 2);
                    let mut d = (-0x80i16) as u16;
                    if x.u8(f::FLIP) != 0 {
                        d = d.wrapping_neg();
                    }
                    x.add16(0x8, d);
                    aids::clamp(x);
                    reappear(x);
                }
            }
            0x8909a => fade_in(x),
            _ => finish(x, 0x0204_0a00),
        },
        0x890c8 => match step(x, 0x890d6) {
            0x89030 => fade_out(x),
            0x890de => {
                if x.anim_step() {
                    addq8(x, f::SUB3, 2);
                    aids::dodge_spot(x);
                    reappear(x);
                }
            }
            0x8909a => fade_in(x),
            _ => finish(x, 0x0202_0000),
        },
        0x89120 => match step(x, 0x8912e) {
            0x8913a => leap_start(x, 0x400, 0x10, 9),
            0x8910c => finish(x, 0x0202_0000),
            t => leap_rest(x, t),
        },
        _ => match step(x, 0x89194) {
            0x89030 => fade_out(x),
            0x8919c => {
                if x.anim_step() {
                    addq8(x, f::SUB3, 2);
                    aids::random_spot(x);
                    aids::clamp(x);
                    x.set8(f::FLIP, 0);
                    if !Fl::sub16(x.u16(0x8), 0xc0).gt() {
                        x.set8(f::FLIP, 0x20);
                    }
                    reappear(x);
                }
            }
            0x8909a => fade_in(x),
            _ => finish(x, 0x0202_0000),
        },
    }
}

/// 0x88f90 (`+6` = 0 du groupe 8) : attente debout, puis retour au combat.
fn rest(x: &mut Cx) {
    match dispatch(x, 0x88f9e, f::SUB3) {
        0x88fe2 => {
            if sub16(x, TIMER, 1).mi() {
                x.set32(f::STATE, 0x0202_0000);
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
            if t == 0x88fac {
                addq8(x, f::SUB3, 2);
                x.set16(TIMER, 0x14);
            } else {
                sub8(x, f::SUB3, 2);
            }
            x.set8(VULNERABLE, 1);
            x.set8(ANIM_KIND, 0);
            let k = x.u8(ANIM_KIND);
            if k != x.u8(ANIM_LAST) {
                x.set8(ANIM_LAST, k);
            }
            body(x, 0);
        }
    }
}
