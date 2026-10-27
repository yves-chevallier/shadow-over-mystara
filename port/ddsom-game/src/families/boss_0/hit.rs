//! Boss 0, état 4 « touché » (0x7a344) et retour au combat. Groupes `+5` :
//!
//! | `+5` | Rôle |
//! |---|---|
//! | 0 | réaction au coup : mort si plus de PV, sinon routine de la table 0x7a396 (code `+0x49`) |
//! | 2 | recul en glissant, puis riposte |
//! | 4 | à terre, relevé |
//! | 6, 8 | mort : projection, chute dans la poussière |
//! | 10 | retour au combat |

use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

/// Table des routines de réaction, indexée par le code de coup `+0x49` (longs).
const REACTIONS: u32 = 0x7a396;
/// La seule routine propre au boss (code 0x4c) : tremblement sur place.
const SHAKE_REACTION: u32 = 0x7a442;

pub(super) fn run(x: &mut Cx) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else {
        match x.u8(f::SUB) {
            0 => reaction(x),
            2 => pushed_back(x),
            4 => down(x),
            6 | 8 => death(x),
            10 => resting(x),
            _ => {}
        }
    }
    lib::keep_in_zone(x);
    x.call(SHADOW);
}

/// 0x7a374 : premier passage : dégâts (0x10112) et, sans PV, la mort ; puis la routine du code de coup.
fn reaction(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        lib::set_68(x);
        x.set8(INVULNERABLE, 0);
        if !tst16(x, f::HP).gt() {
            x.set8(f::SUB, 6);
            x.set16(f::SUB2, 0);
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

/// 0x7a442 : tremblement sur place (16 images), puis à terre.
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
        return lib::start(x, SET_HURT, 8);
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

/// 0x7a4ba : recul en glissant, puis riposte (attaque 10, invulnérable).
fn pushed_back(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.set16(TIMER, 0x14);
            let (angle, vz) = if x.u8(f::FLIP) != 0 { (0x18u32, 0x20u16) } else { (8, (-0x20i16) as u16) };
            x.c.d[0] = angle;
            set_dw(x, 1, vz);
            x.set8(Obj::SPEED_INDEX, angle as u8);
            x.set16(0x16, vz);
            speed(x, 0x17);
            anim_kind(x, 0x20);
            next6(x);
            lib::sound(x, 0xec5e6);
            body(x, 0xb);
        }
        2 => {
            if !sub16(x, TIMER, 1).gt() {
                next6(x);
                x.set16(TIMER, 6);
                x.set16(0x16, 0);
            }
            x.move_x();
            lib::step(x);
        }
        _ => {
            if sub16(x, TIMER, 1).le() {
                x.c.d[0] = 0;
                x.set32(0x58, 0);
                return strike(x, 10, true);
            }
            x.move_x();
            lib::step(x);
        }
    }
}

/// 0x7a552 : à terre (`+0x84` images hors pause), puis relevé.
fn down(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            next6(x);
            x.set8(ANIM_LAST, 0xff);
            x.anim_start(SET_BODY, 0x12);
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
        _ => {
            if x.a5_u8(var::PAUSE) != 0 {
                return;
            }
            if x.anim_step() {
                get_up(x);
            }
        }
    }
}

/// 0x7a5ac : retour au combat (aussi appelé par les réactions communes, table 0x3e000) : franchit le palier de
/// PV suivant (0x105b6) ; la moitié principale se transforme au deuxième palier ; sinon coup si la cible est
/// tout près, ou course.
pub(super) fn get_up(x: &mut Cx) {
    x.set8(0, 1);
    x.set8(ANIM_LAST, 0xff);
    if !pick_target(x) {
        return idle_alone(x);
    }
    if lib::next_threshold(x, THRESHOLDS) && x.u8(3) == 0 && x.u8(TRANSFORMED) == 0 && x.u8(PHASE) == 2 {
        return transform(x);
    }
    if lib::target_in_box(x, 0, 0x38, 0xc) {
        return strike(x, 6, true);
    }
    run_start(x)
}

/// 0x7a5fe : mort. Quand l'autre moitié vit encore, seule une chute (0x19ad6) ; la dernière à tomber prévient la
/// scène du boss vaincu.
fn death(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.set32(f::IMMUNITY, 0xffff_ffff);
            let mut last = true;
            if x.u16(PARTNER) != 0 {
                let other = x.linked(PARTNER);
                if other.u32(x.b, f::STATE) == x.u32(f::STATE) {
                    x.set8(DONE, 1);
                    last = false;
                } else if other.u8(x.b, DONE) == 0 && other.u16(x.b, f::STATE) != 0x406 {
                    last = false;
                }
            }
            if last {
                x.set_a5_u8(0x42, 1);
                x.set_a5_u8(0x50, 0);
                let kind = x.u8(2);
                set_abs8(x, A5_DEATH, kind);
                set_abs8(x, A5_DEATH + 1, 0);
                let score = x.u8(0x7e);
                set_abs8(x, A5_DEATH + 2, score);
                x.set8(0x93, 1);
                x.set_a5_u8(0x4f, 0xff);
                for r in [0x103ba, 0xe0a10, 0xe3b6c] {
                    x.call(r);
                }
            } else {
                x.call(0x19ad6);
            }
            for r in [0xb780, 0xbad6, 0xe2f6] {
                x.call(r);
            }
            x.set8(0, 2);
            let freeze = x.u8(0x56);
            x.set8(f::FREEZE, freeze);
            lib::sound(x, 0xe3cbe);
            x.set16(0x16, 0);
            x.set16(0x18, 0x500);
            x.set16(0x1a, (-0x38i16) as u16);
            let center = camera_x(x).wrapping_add(0xc0);
            set_dw(x, 2, center);
            let (vx, side) = if Fl::sub16(center, x.u16(0x8)).ge() { (0x200u16, 0u32) } else { ((-0x200i16) as u16, 0x20) };
            set_dw(x, 0, vx);
            x.c.d[1] = side;
            x.set16(0x14, vx);
            x.set8(f::FLIP, side as u8);
            x.move_xz();
            next6(x);
            x.anim_start(SET_BODY, 0x13);
        }
        2 => {
            x.move_xz();
            if tst16(x, 0x18).gt() {
                lib::step(x);
                return;
            }
            next6(x);
            x.anim_start(SET_BODY, 0x14);
        }
        4 => {
            x.move_xz();
            if tst16(x, 0xc).gt() {
                return;
            }
            next6(x);
            x.set16(0xc, 0);
            lib::debris(x, 5);
            if x.u8(0x93) != 0 {
                lib::sound(x, 0xec9e8);
            } else {
                lib::sound(x, 0xec904);
            }
            lib::step(x);
        }
        6 => {
            if x.anim_step() {
                next6(x);
            }
        }
        _ => {}
    }
}

/// 0x7a76e : retour au combat après une attente (`+5` = 10).
fn resting(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        next7(x);
        return body(x, 0xc);
    }
    if lib::step(x) {
        if !lib::any_player_alive(x) {
            return idle_alone(x);
        }
        get_up(x);
    }
}
