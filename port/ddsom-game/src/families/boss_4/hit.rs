//! Boss 4, état 4 « touché » (0x82c36). Groupes `+5` :
//!
//! | `+5` | Rôle |
//! |---|---|
//! | 0 | réaction au coup : mort si plus de PV, sinon routine de la table 0x82c8a (code `+0x49`) |
//! | 2 | recul, puis traversée en vol (riposte) |
//! | 4 | à terre, puis relevé |
//! | 6, 8 | mort : projection, chute dans la poussière |
//! | 10 | retour au combat |

use super::decide;
use super::*;
use crate::objects::Courant;

/// Table des routines de réaction, indexée par le code de coup `+0x49` (longs).
const REACTIONS: u32 = 0x82c8a;
/// La seule routine propre au boss (code 0x4c) : tremblement sur place.
const SHAKE_REACTION: u32 = 0x82d36;
/// Seuils de PV par phase (mots, 128 octets par ligne).
const THRESHOLDS: u32 = 0x10ce2;

pub(super) fn run(x: &mut Cx) {
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

/// 0x82c68 : premier passage : dégâts (0x10112) et, sans PV, la mort ; puis la routine du code de coup.
fn reaction(x: &mut Cx) {
    x.c.d[0] = 0;
    if x.u8(f::SUB2) == 0 {
        lib::set_68(x);
        x.set8(INVULNERABLE, 0);
        // 0x833fc
        if !tst16(x, f::HP).gt() {
            x.set8(f::SUB, 6);
            x.set8(0, 2);
            return death(x);
        }
        x.c.d[0] = 0;
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

/// 0x82d36 : tremblement sur place (16 images, ±2 pixels), puis retour au recul (état 4, groupe 4, étape 2).
fn shake(x: &mut Cx) {
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
        x.c.d[0] = d;
        x.add16(0x8, d as u16);
        return;
    }
    let px = x.u16(0xa);
    x.set16(0x8, px);
    x.set8(0, 1);
    x.set32(f::STATE, 0x0404_0200);
    x.set32(0x58, 0);
}

/// 0x82dae : recul (0x14 images en arrière, puis 0x14 sur place), puis la traversée en vol.
fn pushed_back(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.set16(TIMER, 0x14);
            let (angle, vy) = if x.u8(f::FLIP) != 0 { (0x18u8, 0x20u16) } else { (8, (-0x20i16) as u16) };
            x.c.d[0] = angle as u32;
            setw(x, 1, vy);
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
            x.c.d[0] = 0;
            x.set32(0x58, 0);
            decide::swoop(x);
        }
    }
}

/// 0x82e40 : à terre (`+0x84` images hors pause), puis relevé.
fn down(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            lib::behaviour_by_kind(x);
            x.set8(ANIM_LAST, 0xff);
            x.anim_start(SET_BODY, 0xa);
        }
        2 => {
            if (x.a5_u8(var::PAUSE) as i8) > 0 || sub16(x, HITS, 1).gt() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(VULNERABLE, 1);
            x.set8(0x86, 0);
            x.set8(0x49, 0);
        }
        _ => {
            if x.a5_u8(var::PAUSE) != 0 || !x.anim_step() {
                return;
            }
            resume(x);
        }
    }
}

/// 0x82ea0 : retour au combat (aussi appelé par les réactions communes, table 0x3e000) : franchit le palier de
/// PV, puis traversée en vol si un joueur est tout près, sinon la marche.
pub(super) fn resume(x: &mut Cx) {
    x.set8(0, 1);
    x.set8(ANIM_LAST, 0xff);
    if x.a5_u8(0x30) == 0 || !pick_target(x) {
        return decide::wait(x);
    }
    lib::next_threshold(x, THRESHOLDS);
    if lib::target_in_box(x, 0, 0x30, 0xc) {
        decide::swoop(x);
    } else {
        decide::walk(x);
    }
}

/// 0x82ee2 : mort. Projeté en l'air vers le milieu de l'écran, le boss retombe dans la poussière ; la fin de scène
/// est lancée au premier passage.
fn death(x: &mut Cx) {
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
            for a in [0x103ba, 0xe0a10, 0xe3b6c, 0xe411e, 0xb780, 0xbad6, 0xe2f6] {
                x.call(a);
            }
            x.set8(0, 2);
            x.set16(0x16, 0);
            x.set16(0x18, 0x500);
            x.set16(0x1a, (-0x38i16) as u16);
            let mid = x.a5_u16(var::CAMERA1_X).wrapping_add(0xc0);
            setw(x, 2, mid);
            let (vx, side) = if Fl::sub16(mid, x.u16(0x8)).ge() { (0x200u16, 0u8) } else { ((-0x200i16) as u16, 0x20) };
            setw(x, 0, vx);
            x.c.d[1] = side as u32;
            x.set16(0x14, vx);
            x.set8(f::FLIP, side);
            x.move_xz();
            x.c.d[0] = 0;
            x.anim_start(SET_HURT, 0);
        }
        2 => {
            x.move_xz();
            if (x.u16(0xc) as i16) > 0 {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(0xc, 0);
            x.c.d[5] = 5;
            x.call(call::DUST);
            lib::sound(x, 0xec9e8);
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
