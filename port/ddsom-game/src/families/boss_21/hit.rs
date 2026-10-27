//! Boss 21, état 4 « touché » (0x9a3a2). Groupes `+5` :
//!
//! | `+5` | Rôle |
//! |---|---|
//! | 0 | réaction au coup : mort si plus de PV, sinon routine de la table 0x9a40a (code `+0x49`) |
//! | 2 | recul (poussé en arrière, `+6` = 0 à 4) |
//! | 4 | relevé après une chute (`+6` = 0 à 6) |
//! | 6, 8 | mort : chute, envol, fin (`+6` = 0 à 8) |
//! | 10 | fin de relevé |

use super::decide;
use super::*;
use crate::objects::Courant;

/// Table des routines de réaction, indexée par le code de coup `+0x49` (longs).
const REACTIONS: u32 = 0x9a40a;
/// La seule routine propre au boss (code 0x4c) : étourdissement avec tremblement.
const STUN_REACTION: u32 = 0x9a4b6;
/// Seuils de PV par phase (mots, 128 octets par ligne).
const THRESHOLDS: u32 = 0x112e2;

pub(super) fn run(x: &mut Cx) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else {
        match x.u8(f::SUB) {
            0 => reaction(x),
            2 => pushed_back(x),
            4 => get_up(x),
            6 | 8 => down(x),
            10 => recovered(x),
            _ => {}
        }
    }
    x.call(SHADOW);
}

/// 0x9ae8a : plus de PV ? Passe alors au groupe 6 (mort) et rend vrai.
fn out_of_hp(x: &mut Cx) -> bool {
    if tst16(x, f::HP).gt() {
        return false;
    }
    x.set8(f::SUB, 6);
    x.set8(0, 2);
    true
}

fn reaction(x: &mut Cx) {
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
        x.set_a5_u16(0x51e, 0);
        x.call(0x10112);
        if out_of_hp(x) {
            return down(x);
        }
    }
    let code = x.u8(0x49) as u32;
    let routine = rom32(x, REACTIONS + code);
    if routine == STUN_REACTION {
        return stun(x);
    }
    x.c.d[0] = code;
    x.c.a[1] = routine;
    x.call(routine);
}

/// 0x9a4b6 : étourdissement : le boss tremble sur place puis repart.
fn stun(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(0, 2);
            let px = x.u16(0x8);
            x.set16(0xa, px);
            x.set16(HITS, 0);
            let stun = x.u8(STUN);
            x.set8(f::FREEZE, stun);
            x.set8(0x90, 0x10);
            play(x, SET_HURT, 8);
        }
        2 => {
            if tst8(x, 0x90).le() {
                let px = x.u16(0xa);
                x.set16(0x8, px);
                x.set8(0, 1);
                x.set32(f::STATE, 0x0404_0200);
                x.set32(0x58, 0);
                step(x);
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

fn pushed_back(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.set16(TIMER, 0x14);
            let (vx, vz) = if x.u8(f::FLIP) == 0 { (0x400i16, -0x30i16) } else { (-0x400, 0x30) };
            x.set16(0x14, vx as u16);
            x.set16(0x16, vz as u16);
            x.set8(ANIM_KIND, 0x20);
            x.set8(ANIM_LAST, 0x20);
            x.add8(f::SUB2, 2);
            x.call(0xec73c);
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
                step(x);
            }
        }
        4 => {
            if !step(x) {
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

// --- +5 = 4 : relevé ----------------------------------------------------------------------------

fn get_up(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(ANIM_LAST, 0xff);
            x.anim_start(SET_BODY, 0xe);
        }
        2 => {
            if x.a5_u8(var::PAUSE) as i8 > 0 {
                return;
            }
            if sub16(x, HITS, 1).gt() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(VULNERABLE, 1);
            x.set8(0x86, 0);
            x.set8(0x49, 0);
            x.set16(0x16, 0);
            x.set16(0x1e, 0);
            let mut edge = x.a5_u16(var::CAMERA1_X);
            let mut angle = 8;
            edge = edge.wrapping_add(0x30);
            if !Fl::sub16(edge, x.u16(0x8)).ge() {
                angle = 0x18;
                edge = edge.wrapping_add(0x120);
                if !Fl::sub16(edge, x.u16(0x8)).le() {
                    // entre les deux bords : pas de demi-tour
                    x.add8(f::SUB2, 2);
                    return;
                }
            }
            x.set8(ANGLE, angle);
            x.call(call::RANDOM_TURN);
            speed(x, 0x2b);
            x.set16(TIMER, 0x1e);
            // l'animation dépend du sens du boss par rapport au bord choisi
            let wrong = (x.u8(f::FLIP) as i8 as i16 as u16).rotate_right(6) != 0;
            body(x, if wrong { 0xf } else { 0x10 });
        }
        4 => {
            if x.a5_u8(var::PAUSE) != 0 {
                return;
            }
            if sub16(x, TIMER, 1).le() {
                x.add8(f::SUB2, 2);
                let side = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, side);
                body(x, 0x11);
            } else {
                x.move_xy();
                step(x);
            }
        }
        6 => {
            if x.a5_u8(var::PAUSE) != 0 {
                return;
            }
            if !x.anim_step() {
                return;
            }
            x.add8(STRIKES, 1);
            recovered(x);
        }
        _ => {}
    }
}

/// 0x9a6d4 : le boss est debout ; choisit sa suite d'après sa phase.
pub(super) fn recovered(x: &mut Cx) {
    x.set8(0, 1);
    x.set8(ANIM_LAST, 0xff);
    if !pick_target(x) {
        return decide::wait(x);
    }
    x.c.a[1] = THRESHOLDS;
    x.call(0x105b6);
    let mut limit = 2;
    if (x.a5_u8(var::PLAYERS_ALIVE) as i8) > 2 {
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
    if target_in_box(x, 0, 0x20, 0xc) {
        decide::attack_4_shielded(x);
    } else {
        decide::chase(x);
    }
}

// --- +5 = 6, 8 : mort ---------------------------------------------------------------------------

fn down(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set32(f::IMMUNITY, 0xffff_ffff);
            x.call(0x19ad6);
            x.set8(0, 2);
            x.set8(SLOW, 0);
            for addr in [0xe5966, 0xb780, 0xbad6, 0xe2f6] {
                x.call(addr);
            }
            x.set16(0x16, 0);
            x.set16(0x18, 0x500);
            x.set16(0x1a, (-0x38i16) as u16);
            let mut vx: i16 = 0x200;
            let mut side = 0;
            let edge = x.a5_u16(var::CAMERA1_X).wrapping_add(0xc0);
            if !Fl::sub16(edge, x.u16(0x8)).ge() {
                vx = -vx;
                side = 0x20;
            }
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
