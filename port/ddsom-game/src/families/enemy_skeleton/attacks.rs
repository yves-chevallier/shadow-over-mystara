//! Type 1, coups au corps à corps (`+5 = 4`). `+6` : 0 estoc, 2 coup d'estoc haut, 4 enchaînement,
//! 6 double coup, 8 bond-coup, 10 glissade, 12 grand bond, 14 et 16 bond avec demi-tour, 18 et 20 garde (série 1).
//! `+7` est l'étape : les étapes 0 à 4 (selon le coup) sont communes (`ready`, `settle`, `face`).

use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

pub(super) fn run(x: &mut Cx) {
    if special_hit(x) {
        return;
    }
    match x.u8(f::SUB2) {
        0 => thrust(x, 5, 5, 0),
        2 => thrust(x, 6, 4, 1),
        4 => combo(x),
        6 => double_cut(x),
        8 => leap(x, Leap::Short),
        10 => slide(x),
        12 => leap(x, Leap::Long),
        14 => leap(x, Leap::Turning),
        16 => leap(x, Leap::TurningCombo),
        18 | 20 => super::leaps::guard(x),
        _ => {}
    }
    zone_and_terrain(x);
}

// --- étapes communes ----------------------------------------------------------------------------

/// 0x59b98 : le squelette se prépare (animation de repos).
pub(super) fn ready(x: &mut Cx) {
    x.add8(f::SUB3, 2);
    body(x, 0);
}

/// 0x59b82 : attend que le délai `+0x77` soit écoulé.
pub(super) fn settle(x: &mut Cx) {
    let wait = x.u8(COOLDOWN);
    if wait != 0 {
        let n = wait.wrapping_sub(1);
        x.set8(COOLDOWN, n);
        if (n as i8) > 0 {
            return;
        }
    }
    x.add8(f::SUB3, 2);
}

/// 0x591d6 : se tourne vers la cible si elle est derrière (`+0xa6` = 2).
pub(super) fn face(x: &mut Cx) {
    let side = x.u8(SIDE) as i8;
    if side < 2 {
        return x.add8(f::SUB3, 2);
    }
    if side == 2 {
        x.set8(SIDE, 3);
        return body(x, 1);
    }
    if (x.u8(ANIM_STATE) as i8) < 0 {
        x.add8(f::SUB3, 2);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        body(x, 0);
    } else {
        step(x);
    }
}

/// 0x59222 : mémorise si la cible est derrière (`+0xa6` = 2, sinon 0) ; vrai si un demi-tour est nécessaire.
fn needs_turn_around(x: &mut Cx) -> bool {
    let mut side = 0;
    if x.u16(f::TARGET) != 0 {
        let target = load_target(x);
        let tx = target.u16(x.b, Obj::X);
        let mine = x.u16(Obj::X);
        if mine != tx {
            let want = if mine < tx { 0x20 } else { 0 };
            if want != x.u8(f::FLIP) as u32 {
                side = 2;
            }
        }
    }
    x.set8(SIDE, side);
    side != 0
}

/// 0x58370 : fin d'un coup — on attend la fin de l'animation, puis on choisit la suite.
fn finish(x: &mut Cx) {
    if (x.u16(ANIM_STATE) as i16) >= 0 {
        step(x);
    } else {
        x.set8(AI_KIND, 0xff);
        end_attack(x);
    }
}

// --- coups simples ------------------------------------------------------------------------------

/// Estoc (`+6` = 0 ou 2) : tire au sort de reprendre l'idée, sinon prépare puis frappe (type `kind`,
/// force `power`, animation `anim`).
fn thrust(x: &mut Cx, kind: u8, power: u8, anim: u32) {
    match x.u8(f::SUB3) {
        0 => {
            let r = random(x) & 0xf;
            x.c.d[0] = x.c.d[0] & 0xffff_0000 | r;
            if 0xffc0u32 >> r & 1 != 0 {
                set_state_long(x, STANCE_ALT);
            } else {
                x.add8(f::SUB3, 2);
            }
        }
        2 => ready(x),
        4 => settle(x),
        6 => face(x),
        8 => {
            x.add8(f::SUB3, 2);
            x.set8(STRIKE_KIND, kind);
            x.set8(STRIKE_POWER, power);
            strike(x, anim);
        }
        10 => finish(x),
        _ => {}
    }
}

/// Enchaînement (`+6` = 4) : coup de bouclier, puis, au hasard, un second coup ; la série 1 passe à la garde.
fn combo(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => face(x),
        2 => ready(x),
        4 => settle(x),
        6 => {
            if x.u8(SERIES) != 0 {
                x.set16(f::SUB2, 0x1200);
                return super::leaps::guard(x);
            }
            x.set8(STRIKE_KIND, 5);
            x.set8(AI_KIND, 3);
            x.set8(STRIKE_POWER, 4);
            let state = x.u32(f::STATE);
            x.set32(0x8a, state);
            x.add8(f::SUB3, 2);
            strike(x, 3);
        }
        8 => {
            if !step(x) {
                return;
            }
            if random(x) & 1 == 0 {
                end_attack(x);
            } else {
                x.add8(f::SUB3, 2);
                strike(x, 0);
            }
        }
        10 => finish(x),
        _ => {}
    }
}

/// Double coup (`+6` = 6) : un coup au hasard parmi deux animations, puis le suivant.
fn double_cut(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => face(x),
        2 => ready(x),
        4 => settle(x),
        6 => {
            x.add8(f::SUB3, 2);
            let state = x.u32(f::STATE);
            x.set32(0x8a, state);
            x.set8(STRIKE_KIND, 5);
            x.set8(STRIKE_POWER, 2);
            x.set8(AI_KIND, 3);
            let n = rand_and(x, 1);
            let kind = x.u8(STRIKE_KIND).wrapping_add(n as u8);
            x.set8(STRIKE_KIND, kind);
            strike(x, n);
        }
        8 => {
            if step(x) {
                x.add8(f::SUB3, 2);
                strike(x, 1);
            }
        }
        10 => finish(x),
        _ => {}
    }
}

// --- bonds ----------------------------------------------------------------------------------------

#[derive(Clone, Copy)]
pub(super) enum Leap {
    /// Bond court : coup de type 7.
    Short,
    /// Grand bond : type 8, plus rapide.
    Long,
    /// Bond avec demi-tour à l'atterrissage.
    Turning,
    /// Idem, suivi d'un second coup.
    TurningCombo,
}

fn leap(x: &mut Cx, kind: Leap) {
    match x.u8(f::SUB3) {
        0 => ready(x),
        2 => settle(x),
        4 => jump_up(x, kind),
        6 => land_wait(x),
        8 => rise(x),
        10 => fall(x),
        12 => match kind {
            Leap::Short | Leap::Long => {
                step(x);
                if (x.u8(ANIM_STATE) as i8) < 0 {
                    end_attack(x);
                }
            }
            Leap::Turning | Leap::TurningCombo => {
                step(x);
                if (x.u8(ANIM_STATE) as i8) < 0 {
                    x.add8(f::SUB3, 2);
                    if !needs_turn_around(x) {
                        x.add8(f::SUB3, 2);
                    }
                }
            }
        },
        14 => face(x),
        16 => {
            x.add8(f::SUB3, 2);
            let n = if x.u8(f::SUB3) == 0x10 {
                0
            } else {
                rand_and(x, 1)
            };
            strike(x, n);
        }
        18 => match kind {
            Leap::TurningCombo => {
                if step(x) {
                    x.add8(f::SUB3, 2);
                    strike(x, 1);
                }
            }
            _ => finish(x),
        },
        20 => finish(x),
        _ => {}
    }
}

/// 0x584fe : attend l'image-clé du saut.
pub(super) fn land_wait(x: &mut Cx) {
    step(x);
    if x.u8(f::KEY_FRAME) != 0 {
        x.add8(f::SUB3, 2);
    }
}

/// 0x58512 : montée du bond (déplacement en X et en hauteur).
pub(super) fn rise(x: &mut Cx) {
    x.move_xz();
    step(x);
    if (x.u16(Obj::VZ) as i16) < 0 {
        x.add8(f::SUB3, 2);
        x.set16(ANIM_COUNT, 1);
        x.set8(f::KEY_FRAME, 0);
    }
}

/// 0x58536 : descente jusqu'au sol.
pub(super) fn fall(x: &mut Cx) {
    x.move_xz();
    step(x);
    if (x.u32(Obj::Z) as i32) <= 0 {
        x.set32(Obj::Z, 0);
        x.set16(ANIM_COUNT, 1);
        x.add8(f::SUB3, 2);
    }
}

/// Début du bond : vitesse horizontale selon le type de bond et l'orientation, puis envol.
fn jump_up(x: &mut Cx, kind: Leap) {
    match kind {
        Leap::Short => {
            x.set16(Obj::VX, 0x180);
            x.set8(STRIKE_KIND, 7);
        }
        Leap::Long => {
            x.set8(STRIKE_KIND, 8);
            x.set16(Obj::VX, 0x480);
        }
        Leap::Turning | Leap::TurningCombo => x.set16(Obj::VX, 0x480),
    }
    x.set8(STRIKE_POWER, 5);
    if x.u8(f::FLIP) == 0 {
        negate(x, Obj::VX);
    }
    x.set16(Obj::AX, 0);
    x.set16(Obj::VZ, 0x900);
    x.set16(Obj::AZ, -0x80i16 as u16);
    x.add8(f::SUB3, 2);
    strike(x, 2);
}

/// Glissade (`+6` = 10) : le squelette fonce vers la cible en déplaçant son animation.
fn slide(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => ready(x),
        2 => settle(x),
        4 => begin_slide(x),
        6 => slide_move(x),
        _ => {}
    }
}

/// 0x58582 : lance l'animation 2 en la ramenant à l'image qu'elle avait, mémorise la position.
fn begin_slide(x: &mut Cx) {
    x.add8(f::SUB3, 2);
    x.set16(TIMER, 0x2a);
    restart_slide(x);
}

/// 0x5858c : animation 2 rejouée jusqu'à retrouver la même image (au plus huit pas).
pub(super) fn restart_slide(x: &mut Cx) {
    let (count, state) = (x.u16(ANIM_COUNT), x.u16(ANIM_STATE));
    set_dw(x, 1, count);
    set_dw(x, 2, state);
    x.anim_start(SET_BODY, 2);
    x.c.d[3] = 7;
    loop {
        if x.u16(ANIM_STATE) == x.c.d[2] as u16 {
            let d1 = x.c.d[1] as u16;
            x.set16(ANIM_COUNT, d1);
            break;
        }
        x.set16(ANIM_STATE, 1);
        step(x);
        let n = (x.c.d[3] as u16).wrapping_sub(1);
        set_dw(x, 3, n); // relu : D3 relu par la suite (DDSOM_POISON : skeleton_scene13, boss17_scene13)
        if n == 0xffff {
            break;
        }
    }
    finish_restart(x);
}

fn finish_restart(x: &mut Cx) {
    x.set32(MOVE_TABLE, 0x1c7336);
    let angle = if x.u8(f::FLIP) != 0 { 8 + 0x10 } else { 8 };
    x.set8(Obj::SPEED_INDEX, angle);
    let pos = x.u16(Obj::X);
    x.set16(SAVED_POS, pos);
}

fn slide_move(x: &mut Cx) {
    step(x);
    if (x.u8(f::KEY_FRAME) as i8) >= 0 {
        let t = x.u16(TIMER).wrapping_sub(1);
        x.set16(TIMER, t);
        if t == 0 {
            return end_attack(x);
        }
        move_xy(x);
    }
    let d = x.u16(SAVED_POS).wrapping_sub(x.u16(Obj::X)).wrapping_add(0x40);
    if d >= 0x80 {
        end_attack(x);
    }
}
