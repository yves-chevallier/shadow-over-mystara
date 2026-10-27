//! Type 11, sous-état 4 : les coups. `+6` = 0 coup de poing, 2 à 6 séries de coups (précédées d'un demi-tour et
//! d'une pause), 8 et 12 bonds en coup plongeant, 10 marche d'approche, 14 et 16 bonds avec demi-tour, 18 et 20
//! accroupi.

use super::*;
use crate::objects::Courant;

pub(super) fn run(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    let step = x.u8(STEP);
    match x.u8(f::SUB2) {
        0 => match step {
            0 => {
                x.add8(STEP, 2);
                set_strike(x, 0xa, 5);
                strike(x, 0);
            }
            2 => until_end(x),
            _ => {}
        },
        2 => match step {
            0 => coin_flip(x),
            2 => next_anim(x),
            4 => wait(x),
            6 => turn(x),
            8 => {
                x.add8(STEP, 2);
                set_strike(x, 0xc, 4);
                strike(x, 1);
            }
            10 => until_end(x),
            _ => {}
        },
        4 => match step {
            0 => turn(x),
            2 => next_anim(x),
            4 => wait(x),
            6 => {
                if x.u8(GROUP) != 0 {
                    x.set8(f::SUB2, 0x12);
                    return squat_step(x);
                }
                set_strike(x, 0xa, 4);
                x.set8(AI_SHOT, 3);
                let state = x.u32(f::STATE);
                x.set32(0x8a, state);
                x.add8(STEP, 2);
                strike(x, 3);
            }
            8 => {
                if !x.anim_step() {
                    return;
                }
                if rand_and(x, 1) == 0 {
                    return choose(x);
                }
                x.add8(STEP, 2);
                strike(x, 0);
            }
            10 => until_end(x),
            _ => {}
        },
        6 => match step {
            0 => turn(x),
            2 => next_anim(x),
            4 => wait(x),
            6 => {
                x.add8(STEP, 2);
                let state = x.u32(f::STATE);
                x.set32(0x8a, state);
                set_strike(x, 0xc, 2);
                x.set8(AI_SHOT, 3);
                let anim = rand_and(x, 1);
                strike(x, anim);
            }
            8 => {
                if x.anim_step() {
                    x.add8(STEP, 2);
                    strike(x, 1);
                }
            }
            10 => until_end(x),
            _ => {}
        },
        8 => match step {
            0 => next_anim(x),
            2 => wait(x),
            4 => leap_start(x, Some(0x180), Some(0xe)),
            6 => wait_key(x),
            8 => rise(x),
            10 => fall(x),
            12 => end_choose(x),
            _ => {}
        },
        10 => match step {
            0 => next_anim(x),
            2 => wait(x),
            4 => {
                x.add8(STEP, 2);
                x.set16(TIMER, 0x2a);
                restart_walk(x);
            }
            6 => approach(x),
            _ => {}
        },
        12 => match step {
            0 => next_anim(x),
            2 => wait(x),
            4 => {
                x.set8(STRIKE_KIND, 0x10);
                leap_start(x, Some(0x480), None);
            }
            6 => wait_key(x),
            8 => rise(x),
            10 => fall(x),
            12 => end_choose(x),
            _ => {}
        },
        14 | 16 => hop_turn(x, step),
        18 | 20 => squat_step(x),
        _ => {}
    }
}

/// 0x67c74 : tant que le drapeau de fin `+0x32` n'est pas levé, l'animation avance ; ensuite debout.
fn until_end(x: &mut Cx) {
    if (x.u16(ANIM_END) as i16) >= 0 {
        x.anim_step();
        return;
    }
    set_state_long(x, STANCE);
    x.set8(AI_SHOT, 0xff);
    mv(x, 0);
}

/// 0x67c32 : au tirage (dix chances sur seize), retour à la marche (état 0x2020600), sinon étape suivante.
fn coin_flip(x: &mut Cx) {
    x.c.d[1] = x.c.d[1] & 0xffff_0000 | 0xffc0;
    let r = rand_and(x, 0xf);
    if (0xffc0u32 >> r) & 1 != 0 {
        return set_state_long(x, 0x0202_0600);
    }
    x.add8(STEP, 2);
}

/// 0x67dce / 0x67dda / 0x67f5e : met en route un bond : vitesse horizontale (inversée si l'orientation est nulle),
/// poussée vers le haut 0x900, coup de force 5 ; `speed` / `kind` absents quand l'appelant les a déjà posés.
fn leap_start(x: &mut Cx, speed: Option<i16>, kind: Option<u8>) {
    if let Some(speed) = speed {
        x.set16(SPEED_X, speed as u16);
    }
    if let Some(kind) = kind {
        x.set8(STRIKE_KIND, kind);
    }
    x.set8(STRIKE_POWER, 5);
    if x.u8(f::FLIP) == 0 {
        negate(x, SPEED_X);
    }
    x.set16(SPEED_Y, 0);
    x.set16(SPEED_Z, 0x900);
    x.set16(GRAVITY, -0x80i16 as u16);
    x.add8(STEP, 2);
    strike(x, 2);
}

/// 0x67e0e : attend la clé d'animation.
fn wait_key(x: &mut Cx) {
    x.anim_step();
    if x.u8(KEY) == 0 {
        return;
    }
    x.add8(STEP, 2);
}

/// 0x67e22 : montée, jusqu'au sommet (`+0x18` négatif).
fn rise(x: &mut Cx) {
    x.move_xz();
    x.anim_step();
    if (x.u16(SPEED_Z) as i16) < 0 {
        x.add8(STEP, 2);
        x.set16(ANIM_TIME, 1);
        x.set8(KEY, 0);
    }
}

/// 0x67e46 : descente jusqu'au sol.
fn fall(x: &mut Cx) {
    x.move_xz();
    x.anim_step();
    if x.u32(HEIGHT) as i32 > 0 {
        return;
    }
    x.set32(HEIGHT, 0);
    x.set16(ANIM_TIME, 1);
    x.add8(STEP, 2);
}

/// 0x67e6a : fin du bond : choix du geste suivant à la fin de l'animation.
fn end_choose(x: &mut Cx) {
    x.anim_step();
    if (x.u8(ANIM_END) as i8) < 0 {
        choose(x);
    }
}

/// 0x67f00 : marche d'approche : avance tant que la clé n'est pas négative, puis choix quand la minuterie est
/// écoulée ou que le Golem a trop bougé.
fn approach(x: &mut Cx) {
    x.anim_step();
    if (x.u8(KEY) as i8) >= 0 {
        let n = x.u16(TIMER).wrapping_sub(1);
        x.set16(TIMER, n);
        if n == 0 {
            return choose(x);
        }
        x.move_by_table_xy();
    }
    let d = x.u16(TIMER2).wrapping_sub(x.u16(X)).wrapping_add(0x40);
    if d >= 0x80 {
        choose(x);
    }
}

/// +6 = 14 et 16 : bond (vitesse 0x480) puis demi-tour si la cible est derrière, et un coup (16).
fn hop_turn(x: &mut Cx, step: u8) {
    match step {
        0 => next_anim(x),
        2 => wait(x),
        4 => leap_start(x, Some(0x480), None),
        6 => wait_key(x),
        8 => rise(x),
        10 => fall(x),
        12 => {
            x.anim_step();
            if (x.u8(ANIM_END) as i8) < 0 {
                x.add8(STEP, 2);
                if !target_behind(x) {
                    x.add8(STEP, 2);
                }
            }
        }
        14 => turn(x),
        16 => {
            x.add8(STEP, 2);
            let anim = if x.u8(STEP) == 0x10 { 0 } else { rand_and(x, 1) };
            strike(x, anim);
        }
        18 if x.u8(f::SUB2) == 14 => until_end(x),
        18 => {
            if x.anim_step() {
                x.add8(STEP, 2);
                strike(x, 1);
            }
        }
        20 => until_end(x),
        _ => {}
    }
}

/// +6 = 18 et 20 (et 0x12 posé par le profil `+0x2f`) : se fige un moment (animation 8), puis choix.
fn squat_step(x: &mut Cx) {
    match x.u8(STEP) {
        0 => next_anim(x),
        2 => wait(x),
        4 => {
            x.set16(0xa8, 0x100);
            // (le test porte sur l'étape `+7`, pas sur `+6`)
            if x.u8(STEP) != 0x12 {
                x.set8(0xa9, 1);
            }
            squat_begin(x);
        }
        6 => {
            if x.u8(KEY) != 0 {
                let n = x.u16(TIMER).wrapping_sub(1);
                x.set16(TIMER, n);
                if (n as i16) >= 0 {
                    return;
                }
                x.add8(STEP, 2);
                x.set16(ANIM_TIME, 1);
                let group = x.u8(GROUP) as i8 as i32 as u32;
                x.c.d[0] = group;
                let v = x.rom8(0x682e8u32.wrapping_add(group));
                x.set8(0x68, v);
            }
            x.anim_step();
        }
        8 => {
            if x.anim_step() {
                choose(x);
            }
        }
        _ => {}
    }
}

/// 0x6829e : s'accroupit : étape suivante, minuterie 0x3c, animation 8.
pub(super) fn squat_begin(x: &mut Cx) {
    x.add8(STEP, 2);
    x.set16(TIMER, 0x3c);
    x.set8(0x68, 9);
    mv(x, 8);
}
