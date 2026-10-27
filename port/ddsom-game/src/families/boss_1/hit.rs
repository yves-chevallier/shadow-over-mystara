//! Boss 1, état 4 « touché » (0x7bb66) et retour au combat. Groupes `+5` :
//!
//! | `+5` | Rôle |
//! |---|---|
//! | 0 | réaction au coup : mort si plus de PV, sinon routine de la table 0x7bbbe (code `+0x49`) |
//! | 2 | recul en glissant, puis riposte |
//! | 4 | à terre, relevé, bond de riposte |
//! | 6, 8 | mort : projection, chute dans la poussière |
//! | 10 | retour au combat |

use super::*;
use crate::objects::Courant;

/// Table des routines de réaction, indexée par le code de coup `+0x49` (longs).
const REACTIONS: u32 = 0x7bbbe;
/// La seule routine propre au boss (code 0x4c) : tremblement sur place.
const SHAKE_REACTION: u32 = 0x7bc6a;

pub(super) fn run(x: &mut Cx) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else {
        match x.u8(f::SUB) {
            0 => reaction(x),
            2 => pushed_back(x),
            4 => down(x),
            6 | 8 => death(x),
            _ => get_up(x),
        }
    }
    x.call(IN_ZONE);
    x.call(SHADOW);
}

/// 0x7bb96 : premier passage : dégâts (0xb648, 0x10112) et, sans PV, la mort ; puis la routine du code de coup.
fn reaction(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        x.call(0xb648);
        x.call(0x10112);
        x.set8(INVULNERABLE, 0);
        // 0x7c588
        if !tst16(x, f::HP).gt() {
            x.set8(f::SUB, 6);
            x.set8(0, 2);
            x.c.d[0] = 1;
            return death(x);
        }
    }
    let code = x.u8(0x49) as u32;
    x.c.d[0] = code;
    let routine = rom32(x, REACTIONS + code);
    x.c.a[1] = routine;
    if routine == SHAKE_REACTION {
        shake(x)
    } else {
        x.call(routine)
    }
}

/// 0x7bc6a : tremblement sur place (16 images), puis à terre.
fn shake(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        next6(x);
        x.set8(0, 2);
        let px = x.u16(0x8);
        x.set16(SAVED_X, px);
        x.set16(DOWN_TIME, 0);
        let freeze = x.u8(0x56);
        x.set8(f::FREEZE, freeze);
        x.set8(SHAKE, 0x10);
        return play(x, SET_HURT, 8);
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
        set_dw(x, 0, d);
        x.add16(0x8, d);
        return;
    }
    let px = x.u16(SAVED_X);
    x.set16(0x8, px);
    x.set8(0, 1);
    x.set32(f::STATE, 0x0404_0200);
    x.set32(0x58, 0);
}

/// 0x7bce2 : recul en glissant, puis riposte selon la position de la cible.
fn pushed_back(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.set16(TIMER, 0xa);
            x.set16(0xc, 0);
            let (angle, vz) = if x.u8(f::FLIP) != 0 { (0x18u32, 0x20u16) } else { (8, (-0x20i16) as u16) };
            x.c.d[0] = angle;
            set_dw(x, 1, vz);
            x.set8(ANGLE, angle as u8);
            x.set16(0x16, vz);
            speed(x, 0x17);
            kind(x, 0x20);
            next6(x);
            body(x, 0x1a);
        }
        2 => {
            if expired(x, TIMER) {
                next6(x);
                x.set16(TIMER, 4);
                x.set16(0x16, 0);
            }
            x.move_x();
            step(x);
        }
        _ => {
            if !expired(x, TIMER) {
                x.move_x();
                step(x);
                return;
            }
            x.c.d[0] = 0;
            x.set32(0x58, 0);
            if target_in_box(x, -0x60, 0x35, 6) {
                return go(x, 0x7c344);
            }
            if target_in_box(x, -0xe0, 0x30, 8) {
                return go(x, 0x7c2fa);
            }
            walk(x)
        }
    }
}

/// 0x7bda4 : à terre (`+0x84` images hors pause), relevé, et bond si la cible est devant.
fn down(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            x.call(0xfffc);
            x.set8(ANIM_LAST, 0xff);
            body(x, 0x1b);
        }
        2 => {
            if (x.a5_u8(var::PAUSE) as i8) > 0 || sub16(x, DOWN_TIME, 1).gt() {
                return;
            }
            next6(x);
            x.set8(VULNERABLE, 1);
            x.set8(0x86, 0);
            x.set8(0x49, 0);
        }
        4 => {
            if x.a5_u8(var::PAUSE) != 0 {
                return;
            }
            if step(x) {
                return get_up(x);
            }
            if x.u8(KEY) == 0 || outside(x, 0x18, 0x168) || !target_in_box(x, -0x10, 0x30, 0xc) {
                return;
            }
            next6(x);
            x.c.d[0] = 0;
            x.set16(0x16, 0);
            x.set16(0x1e, 0);
            x.set16(0x18, 0x480);
            x.set16(0x1a, (-0x38i16) as u16);
            let angle = if x.u8(f::FLIP) != 0 { 0x18 } else { 8 };
            x.set8(ANGLE, angle);
            speed(x, 0x12);
            body(x, 0x1c);
        }
        6 => {
            step(x);
            x.move_xyz();
            if x.u16(0xc) as i16 > 0 {
                return;
            }
            next6(x);
            x.set16(0xc, 0);
            dust(x, 6);
            x.call(call::LANDING);
            body(x, 0x1d);
        }
        _ => {
            if step(x) {
                return;
            }
            x.set8(0, 1);
            x.set8(ANIM_LAST, 0xff);
            x.c.a[1] = THRESHOLDS;
            x.call(call::PHASE_UP);
            get_up(x)
        }
    }
}

/// 0x7bece : retour au combat (aussi appelé par les réactions communes, table 0x3e000) : franchit le palier de
/// PV suivant (0x105b6), puis bond si hors de l'écran, bond en ciseaux, saisie ou attente.
pub(super) fn get_up(x: &mut Cx) {
    x.set8(0, 1);
    x.set8(ANIM_LAST, 0xff);
    if !pick_target(x) {
        return idle_alone(x);
    }
    x.c.a[1] = THRESHOLDS;
    x.call(call::PHASE_UP);
    if outside(x, 0x18, 0x168) {
        return leap_guarded(x);
    }
    if try_split_leap(x) {
        return split_leap(x);
    }
    if target_in_box(x, -0x40, 0x30, 8) {
        return go(x, 0x7c344);
    }
    stand(x)
}

/// 0x7bf22 : mort : la scène est prévenue, le boss est projeté puis s'effondre.
fn death(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            x.set_a5_u8(0x42, 1);
            x.set_a5_u8(0x50, 0);
            let kind = x.u8(2);
            set_abs8(x, A5_DEATH, kind);
            set_abs8(x, A5_DEATH + 1, 0);
            let score = x.u8(0x7e);
            set_abs8(x, A5_DEATH + 2, score);
            for r in [0x103ba, 0xe0a10, 0xe3b6c, 0xe3e8e, 0xb780, 0xbad6, 0xe2f6] {
                x.call(r);
            }
            x.set8(0, 2);
            x.set16(0x16, 0);
            x.set16(0x18, 0x500);
            x.set16(0x1a, (-0x38i16) as u16);
            let center = x.a5_u16(var::CAMERA1_X).wrapping_add(0xc0);
            set_dw(x, 2, center);
            let (vx, side) = if center as i16 >= x.u16(0x8) as i16 { (0x200u16, 0u32) } else { ((-0x200i16) as u16, 0x20) };
            set_dw(x, 0, vx);
            x.c.d[1] = side;
            x.set16(0x14, vx);
            x.set8(f::FLIP, side as u8);
            x.move_xz();
            x.anim_start(SET_BODY, 0x1e);
        }
        2 => {
            x.move_xz();
            if x.u16(0xc) as i16 > 0 {
                step(x);
                return;
            }
            next6(x);
            x.set16(0xc, 0);
            dust(x, 6);
            x.call(0xec9e8);
            x.anim_start(SET_BODY, 0x1f);
        }
        4 => {
            if x.anim_step() {
                next6(x);
            }
        }
        _ => {}
    }
}

/// Boss vaincu : type, variante et score (`A5-0x557c..`), lus par la fin de scène.
const A5_DEATH: u32 = 0xff8000 - 0x557c;
