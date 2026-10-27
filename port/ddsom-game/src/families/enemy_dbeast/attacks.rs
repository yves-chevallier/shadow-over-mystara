//! Type 19, les coups de la bête (sous-état `+5 = 4`, 0x72faa) : `+6` choisit le coup, `+7` ses étapes.
//!
//! Tous les coups commencent de la même façon : `+7 = 0` la bête se prépare (animation de repos),
//! `+7 = 2` elle suit sa cible du regard (demi-tour si elle est derrière) ; les étapes suivantes
//! sont propres au coup. Quand le coup est fini, `next_move` enchaîne ou remet la bête debout.

use super::parts::{bound_land, bound_wait};
use super::stance::{engage, next_move};
use super::*;
use crate::objects::Courant;

/// État long à reprendre après un demi-tour : sous-état 4 (ici) ou 6 (`attacks_b`).
pub(super) const RESUME_ATTACK: u32 = 0x0204_0000;

pub(super) fn run(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => jab(x),
        2 => jab_b(x),
        4 => lunge(x),
        6 => double_strike(x),
        8 => claw_pair(x),
        10 => leap_strike(x),
        12 => jump_in(x),
        14 | 34 => pounce(x),
        16 | 36 => backflip(x),
        18 => turn_run(x),
        20 => leap_strike_b(x),
        22 => claw(x),
        24 => claw_b(x),
        26 => claw_series(x),
        28 => claw_series_b(x),
        30 => claw_leap(x),
        32 => crouch(x),
        _ => {}
    }
}

// --- étapes communes ----------------------------------------------------------------------------

/// +7 = 0 (0x72e80) : la bête se prépare (animation de repos).
pub(super) fn ready(x: &mut Cx) {
    x.add8(0x7, 2);
    x.set8(VULNERABLE, 1);
    x.set8(WANT_ANIM, 0);
    if x.u8(WANT_ANIM) != x.u8(CUR_ANIM) {
        x.set8(CUR_ANIM, 0);
        play(x, 0);
    }
}

/// +7 = 2 (0x72ffc / 0x72ff0) : elle suit sa cible du regard pendant `+0x77` images (demi-tour au
/// besoin, puis reprise de l'attaque `resume`).
pub(super) fn track(x: &mut Cx, resume: u32) {
    x.set32(RESUME_STATE, resume);
    let n = x.u8(AI_KICK).wrapping_sub(1);
    x.set8(AI_KICK, n);
    if (n as i8) < 0 {
        return x.add8(0x7, 2);
    }
    if player_ahead(x) {
        let sub2 = x.u8(f::SUB2);
        x.set8(RESUME_SUB2, sub2);
        set_state_long(x, 0x0202_0200);
    }
    step(x);
}

/// Les trois premières étapes communes, puis `later(étape)` pour les autres.
pub(super) fn phased(x: &mut Cx, resume: u32, later: impl FnOnce(&mut Cx, u8)) {
    match x.u8(0x7) {
        0 => ready(x),
        2 => track(x, resume),
        n => later(x, n),
    }
}

/// Ce que doit faire un coup quand il voudrait jouer son animation `+0xa5`.
pub(super) enum Replay {
    /// Une autre animation tournait : la lancer.
    New,
    /// La même tournait déjà : une répétition de moins (`+0xae`), il en reste.
    Again,
    /// Plus de répétition : la bête est remise debout.
    End,
}

pub(super) fn replay(x: &mut Cx) -> Replay {
    let want = x.u8(WANT_ANIM);
    if want != x.u8(CUR_ANIM) {
        x.set8(CUR_ANIM, want);
        return Replay::New;
    }
    let left = x.u8(REPEAT_LEFT).wrapping_sub(1);
    x.set8(REPEAT_LEFT, left);
    if left == 0 {
        set_state_long(x, ACTIVE_STANCE);
        return Replay::End;
    }
    Replay::Again
}

/// Début d'un coup : étape suivante, vulnérabilité, animation voulue, force du coup.
fn strike_begin(x: &mut Cx, vulnerable: u8, want: u8, power: u8, kind: u8) {
    x.add8(0x7, 2);
    x.set8(VULNERABLE, vulnerable);
    x.set8(STRIKE_POWER, power);
    x.set8(STRIKE_KIND, kind);
    x.set8(WANT_ANIM, want);
}

/// Lance l'animation `n` de la série des coups, sauf si la série de répétitions est finie.
fn strike_play(x: &mut Cx, n: u8) {
    if !matches!(replay(x), Replay::End) {
        play_b(x, n);
    }
}

/// Fin d'animation d'un coup, retour au repos puis choix du coup suivant.
fn strike_end(x: &mut Cx) {
    if x.anim_step() {
        play(x, 0);
        next_move(x);
    }
}

/// Fin d'animation d'un coup, choix immédiat du coup suivant (0x73108).
fn strike_end_quick(x: &mut Cx) {
    if x.anim_step() {
        next_move(x);
    }
}

/// Un son d'impact, quand l'image-clé est atteinte.
fn key_sound(x: &mut Cx, sound: u32) {
    if x.u8(f::KEY_FRAME) != 0 {
        x.set8(f::KEY_FRAME, 0);
        x.call(sound);
    }
}

/// Direction de marche : vers la droite (8) ou la gauche (0x18) selon l'orientation.
fn walk_angle(x: &mut Cx) {
    x.set8(Obj::SPEED_INDEX, 8);
    if x.u8(f::FLIP) == 0 {
        x.set8(Obj::SPEED_INDEX, 0x18);
    }
}

fn random_bit(x: &mut Cx) -> bool {
    random(x) & 1 != 0
}

// --- les coups ----------------------------------------------------------------------------------

/// 0x72fdc : coup de patte rapide.
fn jab(x: &mut Cx) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => {
            strike_begin(x, 0, 0x2d, 5, 0);
            strike_play(x, 3);
        }
        6 => strike_end(x),
        _ => {}
    });
}

/// 0x730a2 : second coup de patte.
fn jab_b(x: &mut Cx) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => {
            strike_begin(x, 0, 0x28, 5, 1);
            strike_play(x, 0);
        }
        6 => strike_end_quick(x),
        _ => {}
    });
}

/// 0x73118 : avance puis bondit sur un joueur en le griffant.
fn lunge(x: &mut Cx) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => {
            x.add8(0x7, 2);
            x.set8(TIMER, 0x14);
            x.set8(VULNERABLE, 1);
            x.set8(WANT_ANIM, 0x16);
            walk_angle(x);
            play(x, 0xd);
            speed(x, 0x17);
        }
        6 => {
            let t = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, t);
            if (t as i8) >= 0 {
                step(x);
                return move_xy(x);
            }
            x.add8(0x7, 2);
            let flip = x.u8(f::FLIP) != 0;
            x.set16(0x14, if flip { -0x180i16 } else { 0x180 } as u16);
            x.set16(0x16, if flip { 0x10 } else { -0x10i16 } as u16);
            play_b(x, 0);
        }
        8 => {
            if x.u8(0x14) != 0 {
                x.move_x();
            }
            if x.anim_step() {
                x.set32(0x14, 0);
                next_move(x);
            }
        }
        _ => {}
    });
}

/// 0x731da : deux ou trois coups de patte d'affilée.
fn double_strike(x: &mut Cx) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => {
            let count = if random_bit(x) { 3 } else { 2 };
            x.set8(REPEAT, count);
            strike_begin(x, 0, 0x2d, 5, 0);
            match replay(x) {
                Replay::New => play_b(x, 3),
                Replay::Again => {
                    let a = if random_bit(x) { 3 } else { 0 };
                    play_b(x, a);
                }
                Replay::End => {}
            }
        }
        6 => {
            if x.anim_step() {
                play(x, 0);
                let left = x.u8(REPEAT).wrapping_sub(1);
                x.set8(REPEAT, left);
                if left == 0 {
                    return next_move(x);
                }
                let a = if random_bit(x) { 3 } else { 0 };
                play_b(x, a);
            }
        }
        _ => {}
    });
}

/// 0x732b4 : coup de patte suivi, au hasard, d'un second.
fn claw_pair(x: &mut Cx) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => {
            strike_begin(x, 0, 0x2a, 5, 1);
            play_b(x, 1);
            if random_bit(x) {
                x.add8(0x7, 2);
                x.set8(WANT_ANIM, 0x2b);
                play_b(x, 2);
            }
            replay(x);
        }
        6 => strike_end_quick(x),
        8 => strike_end(x),
        _ => {}
    });
}

/// Bond griffes dehors, vitesse initiale en X `vx` (0x7337a et ses trois entrées).
fn leap_strike_start(x: &mut Cx, vx: i16) {
    strike_begin(x, 1, 0x2f, 6, 2);
    let vx = if x.u8(f::FLIP) != 0 { vx.wrapping_neg() } else { vx };
    set_velocity(x, vx, 0x4c0, -0x38);
    strike_play(x, 5);
}

/// Retombée d'un bond griffu : attendre l'image-clé, atterrir, finir l'animation.
fn leap_landing(x: &mut Cx, n: u8, first: u8) {
    match n.wrapping_sub(first) {
        0 => bound_wait(x),
        2 => bound_land(x),
        _ => strike_end_quick(x),
    }
}

/// 0x73348 : bond en avant griffes dehors.
fn leap_strike(x: &mut Cx) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => leap_strike_start(x, -0x1c0),
        n => leap_landing(x, n, 6),
    });
}

/// 0x73754 : le même bond, plus rapide.
fn leap_strike_b(x: &mut Cx) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => leap_strike_start(x, -0x300),
        n => leap_landing(x, n, 6),
    });
}

/// 0x733e8 : saute vers un joueur.
fn jump_in(x: &mut Cx) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => super::moves::jump_toward(x),
        6 => super::moves::jump_wait(x),
        8 => bound_wait(x),
        10 => bound_land(x),
        12 => super::moves::jump_end(x),
        _ => {}
    });
}

/// 0x7346a : prend son élan.
fn crouch(x: &mut Cx) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => {
            x.add8(0x7, 2);
            x.set8(VULNERABLE, 1);
            x.set8(TIMER, 0xe);
            play(x, 8);
            speed(x, 0x11);
        }
        6 => super::moves::jump_wait(x),
        _ => {}
    });
}

/// 0x734a8 : saute sur place plusieurs fois (le nombre est tiré dans une table).
fn pounce(x: &mut Cx) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => pounce_start(x),
        6 => pounce_repeat(x),
        _ => {}
    });
}

/// Tire le nombre de sauts sur place (table de la ROM en 0x73516).
pub(super) fn roll_pounces(x: &mut Cx) {
    const COUNTS: u32 = 0x73516;
    let r = random(x) & 0x1f;
    let count = x.data8(COUNTS + r);
    x.set8(REPEAT, count);
}

/// 0x734bc : début d'une série de sauts sur place.
fn pounce_start(x: &mut Cx) {
    x.add8(0x7, 2);
    x.set8(VULNERABLE, 1);
    x.set8(WANT_ANIM, 7);
    roll_pounces(x);
    if !matches!(replay(x), Replay::End) {
        play(x, 7);
    }
}

/// 0x73536 : un saut sur place fini ; au hasard un cri accompagne l'image-clé.
pub(super) fn pounce_repeat(x: &mut Cx) {
    const SOUNDS: u32 = 0x73584;
    if x.u8(f::KEY_FRAME) != 0 {
        x.set8(f::KEY_FRAME, 0);
        let r = random(x) & 0xf;
        let sound = x.rom8(SOUNDS + r);
        if sound != 0 {
            set_dw(x, 0, sound as u16);
            x.call(0xe38d2);
        }
    }
    if x.anim_step() {
        let left = x.u8(REPEAT).wrapping_sub(1);
        x.set8(REPEAT, left);
        if left == 0 {
            return next_move(x);
        }
        play(x, 7);
    }
}

/// 0x73594 : saut périlleux.
fn backflip(x: &mut Cx) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => {
            x.add8(0x7, 2);
            x.set8(VULNERABLE, 1);
            x.set8(STRIKE_POWER, 6);
            x.set8(STRIKE_KIND, 2);
            x.set32(0x14, 0);
            x.set16(0x18, 0x380);
            x.set16(0x1a, (-0x38i16) as u16);
            play(x, 6);
        }
        6 | 10 => bound_wait(x),
        8 => {
            if x.move_xz().gt() {
                return step(x);
            }
            x.set32(0xc, 0);
            x.add8(0x7, 2);
            x.set8(VULNERABLE, 1);
            x.set8(WANT_ANIM, 0x18);
            x.set8(CUR_ANIM, 0x18);
            x.set32(0x14, 0);
            x.set16(0x18, 0x380);
            x.set16(0x1a, (-0x38i16) as u16);
            play(x, 6);
        }
        12 => bound_land(x),
        14 => strike_end(x),
        _ => {}
    });
}

/// 0x7365e : fait demi-tour en courant, repart vers le joueur puis s'arrête.
fn turn_run(x: &mut Cx) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => {
            x.add8(0x7, 2);
            x.set8(VULNERABLE, 1);
            let ahead = target(x).is_some_and(|t| (t.u16(x.b, 0x8).wrapping_sub(x.u16(0x8)) as i16) >= 0);
            x.set8(Obj::SPEED_INDEX, if ahead { 0x18 } else { 8 });
            play(x, 5);
        }
        6 => {
            if x.anim_step() {
                x.add8(0x7, 2);
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                x.set8(WANT_ANIM, 0x16);
                x.set8(TIMER, 0x2a);
                play(x, 0xd);
                speed(x, 0x17);
            }
        }
        8 => {
            let t = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, t);
            if (t as i8) >= 0 {
                step(x);
                return move_xy(x);
            }
            x.add8(0x7, 2);
            play(x, 5);
        }
        10 => {
            if x.anim_step() {
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                set_state_long(x, 0x0202_0002);
                x.set8(WANT_ANIM, 0xc);
                play(x, 8);
                engage(x);
            }
        }
        _ => {}
    });
}

/// Début d'une griffe (0x73786, 0x7382c) : animation 4 de la série des coups.
fn claw_begin(x: &mut Cx) {
    x.add8(0x7, 2);
    x.set8(WANT_ANIM, 0x2e);
    x.set8(VULNERABLE, 1);
    strike_play(x, 4);
}

/// Fin d'une griffe simple : son d'impact, puis repos et coup suivant.
fn claw_end(x: &mut Cx, sound: u32) {
    key_sound(x, sound);
    strike_end(x);
}

/// 0x7376c : griffe.
fn claw(x: &mut Cx) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => claw_begin(x),
        6 => claw_end(x, 0x9dda8),
        _ => {}
    });
}

/// 0x737fe : griffe, autre son.
fn claw_b(x: &mut Cx) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => claw_begin(x),
        6 => claw_end(x, 0x9e062),
        _ => {}
    });
}

/// Fin d'une griffe d'une série de `+0xab` : la dernière ramène au repos.
fn series_end(x: &mut Cx, sound: u32) {
    key_sound(x, sound);
    if x.anim_step() {
        play_b(x, 4);
        let left = x.u8(REPEAT).wrapping_sub(1);
        x.set8(REPEAT, left);
        if left == 0 {
            play(x, 0);
        }
        next_move(x);
    }
}

/// 0x738a4 : une griffe, puis coup suivant.
fn claw_series(x: &mut Cx) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => {
            x.set8(REPEAT, 1);
            claw_begin(x);
        }
        6 => series_end(x, 0x9dda8),
        _ => {}
    });
}

/// 0x73904 : une ou deux griffes.
fn claw_series_b(x: &mut Cx) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => {
            let count = if random_bit(x) { 2 } else { 1 };
            x.set8(REPEAT, count);
            claw_begin(x);
        }
        6 => series_end(x, 0x9e062),
        _ => {}
    });
}

/// 0x73964 : griffe puis bond griffu.
fn claw_leap(x: &mut Cx) {
    phased(x, RESUME_ATTACK, |x, n| match n {
        4 => claw_begin(x),
        6 => {
            key_sound(x, 0x9e062);
            if x.anim_step() {
                x.add8(0x7, 2);
                play_b(x, 5);
            }
        }
        8 => leap_strike_start(x, -0x200),
        n => leap_landing(x, n, 10),
    });
}
