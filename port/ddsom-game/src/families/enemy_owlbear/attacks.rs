//! Type 4, coups de l'Owlbear : sous-état 4 (0x5e086, dix coups) et sous-état 6 (0x5e5d6, bonds).
//!
//! Chaque coup commence par la préparation (`+7 = 0`) et le suivi du regard (`+7 = 2`), puis ses
//! étapes propres. La fin du coup appelle `next_move`, qui enchaîne une charge ou remet la bête debout.

use crate::families::cx::Banc;
use super::stance::{next_move, pursue_tail, ready, walk_begin};
use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

/// Série de répétitions d'un coup, tirée dans une table par niveau de difficulté (0x5f5fa).
const SERIES_TABLE: u32 = 0x5f5fa;
/// États longs à reprendre après un demi-tour : coups (sous-état 4) ou seconde série (6).
const RESUME_STRIKES: u32 = 0x0204_0000;
const RESUME_LEAPS: u32 = 0x0206_0000;

pub(super) fn run(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => swipe(x),
        2 => swipe_walk(x),
        4 => leap(x),
        6 => run_leap(x),
        8 => charge(x),
        10 => charge_b(x),
        12 => leap_far(x),
        14 => leap_high(x),
        16 => rush(x),
        18 => rush_b(x),
        _ => {}
    }
}

pub(super) fn run_b(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            set_state_long(x, ACTIVE_ENGAGE);
            body(x, 2);
            speed(x, 0xb);
            pursue_tail(x);
        }
        2 => chain(x, RESUME_LEAPS, |x, n| match n {
            4 => hop_begin(x),
            6 => hop_wait(x),
            8 => hop_land(x),
            10 => hop_slide(x),
            12 => hop_end(x),
            _ => {}
        }),
        4 => chain(x, RESUME_LEAPS, |x, n| match n {
            4 => run_toward_begin(x),
            6 => run_toward(x),
            _ => {}
        }),
        _ => {}
    }
}

// --- étapes communes ----------------------------------------------------------------------------

/// Préparation (`+7 = 0`), suivi du regard (`+7 = 2`), puis `later(étape)`.
fn chain(x: &mut Cx, resume: u32, later: impl FnOnce(&mut Cx, u8)) {
    match x.u8(0x7) {
        0 => ready(x),
        2 => track(x, resume),
        n => later(x, n),
    }
}

/// 0x5e0ba / 0x5e624 : suit la cible du regard pendant `+0x77` images.
fn track<R: Banc>(x: &mut Cx<'_, R>, resume: u32) {
    let n = x.u8(AI_KICK).wrapping_sub(1);
    x.set8(AI_KICK, n);
    if (n as i8) < 0 {
        return x.add8(0x7, 2);
    }
    x.set32(RESUME_STATE, resume);
    let sub2 = x.u8(f::SUB2);
    x.set8(RESUME_STEP, sub2);
    turn_if_behind(x);
    step(x);
}

/// Joue l'animation `n` des coups sauf si la série de répétitions est finie.
fn strike_play(x: &mut Cx, want: u8, n: u32) {
    if !matches!(replay(x, want), Replay::End) {
        strike(x, n);
    }
}

/// Un temps d'attente avant de continuer (`+0xa0` images), puis étape suivante.
fn pause<R: Banc>(x: &mut Cx<'_, R>, frames: u8) {
    x.add8(0x7, 2);
    x.set8(TIMER, frames);
}

/// Attend la fin du délai en avançant (0x5e17e, 0x5e2ea).
fn walk_on<R: Banc>(x: &mut Cx<'_, R>) {
    let t = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, t);
    if t != 0 {
        step(x);
        move_xy(x);
    } else {
        x.add8(0x7, 2);
    }
}

/// Fin d'un coup : l'animation finie, le coup suivant.
fn finish(x: &mut Cx) {
    if x.anim_step() {
        next_move(x);
    }
}

// --- coup de patte (+6 = 0) ----------------------------------------------------------------------

/// Début d'un coup de patte (0x5e0e0).
fn swipe_begin(x: &mut Cx) {
    x.add8(0x7, 2);
    x.set8(VULNERABLE, 0);
    x.set8(STRIKE_KIND, 0xe);
    x.set8(STRIKE_POWER, 5);
    roll_series(x);
    strike_play(x, 0xa, 0);
}

/// 0x5f5d8 : tire le nombre de coups de la série dans la table du niveau de difficulté.
fn roll_series(x: &mut Cx) {
    let level = (var::RANK.get(x.b) >> 1 << 4) as u32;
    let r = random(x) & 0xf;
    let n = x.data8(SERIES_TABLE + level + r);
    x.set8(SERIES, n);
}

/// Fin d'un coup de patte (0x5e132) : coup suivant, ou un coup de plus tant que la série dure.
fn swipe_end(x: &mut Cx) {
    if x.anim_step() {
        let n = x.u8(SERIES).wrapping_sub(1);
        x.set8(SERIES, n);
        if n == 0 {
            return next_move(x);
        }
        strike(x, 0);
    }
}

fn swipe(x: &mut Cx) {
    chain(x, RESUME_STRIKES, |x, n| match n {
        4 => swipe_begin(x),
        6 => swipe_end(x),
        _ => {}
    });
}

/// +6 = 2 (0x5e158) : avance, puis coup de patte.
fn swipe_walk(x: &mut Cx) {
    chain(x, RESUME_STRIKES, |x, n| match n {
        4 => walk_begin(x),
        6 => pause(x, 0x15),
        8 => walk_on(x),
        10 => swipe_begin(x),
        12 => swipe_end(x),
        _ => {}
    });
}

// --- bonds (+6 = 4, 6, 12, 14) -------------------------------------------------------------------

/// Début d'un bond (0x5e1b2 / 0x5e1b8 / 0x5e1bc) : vitesse en X `vx` déjà posée par l'appelant.
pub(super) fn leap_begin(x: &mut Cx) {
    x.add8(0x7, 2);
    leap_body(x);
}

/// Suite du début d'un bond, l'étape étant déjà avancée.
fn leap_body(x: &mut Cx) {
    x.set8(STRIKE_KIND, 0xf);
    x.set8(STRIKE_POWER, 8);
    x.set8(VULNERABLE, 1);
    if x.u8(f::FLIP) != 0 {
        let vx = x.u16(0x14).wrapping_neg();
        x.set16(0x14, vx);
    }
    x.set16(0x16, 0);
    x.set16(0x18, 0x400);
    x.set16(0x1a, (-0x38i16) as u16);
    strike_play(x, 0xc, 1);
}

fn leap_at(x: &mut Cx, vx: i16) {
    x.set16(0x14, vx as u16);
    leap_begin(x);
}

/// 0x5e222 : attend l'image-clé du saut.
fn leap_wait<R: Banc>(x: &mut Cx<'_, R>) {
    if x.u8(f::KEY_FRAME) != 0 {
        x.add8(0x7, 2);
        x.set8(VULNERABLE, 0);
    }
    step(x);
}

/// Fin d'un saut avec choc au sol (0x5e238) : poussière et recul.
fn leap_land<R: Banc>(x: &mut Cx<'_, R>) {
    if !x.move_xz().gt() {
        lib::sound(x, 0xecb26);
        lib::sound(x, 0xecdba);
        x.set32(0xc, 0);
        x.add8(0x7, 2);
        x.set16(0x30, 1);
        x.set8(VULNERABLE, 1);
        let v = if x.u8(f::FLIP) != 0 { -0x1c } else { 0x1c };
        x.set16(0x16, v as u16);
    }
    step(x);
}

/// Poussière de réception : 0xb10c8 avec ses quatre paramètres.
fn dust(x: &mut Cx, d2: i16) {
    lib::shock(x, 1, 1, d2 as u16, 0xffff);
}

/// Glissade du recul (0x5e27a) : poussière tous les quatre pas, puis arrêt.
fn slide(x: &mut Cx) {
    let low = x.a5_u16(0x6e) & 3;
    x.set_a5_u16(0x6e, low);
    if low == 0 {
        dust(x, -0x18);
    }
    if x.u8(0x14) != 0 {
        x.move_x();
        return;
    }
    x.add8(0x7, 2);
    x.set32(0x14, 0);
}

/// Fin d'un coup à terre (0x5e2ae).
fn leap_end(x: &mut Cx) {
    finish(x);
}

fn leap(x: &mut Cx) {
    chain(x, RESUME_STRIKES, |x, n| match n {
        4 => leap_at(x, -0x200),
        6 => leap_wait(x),
        8 => leap_land(x),
        10 => slide(x),
        12 => leap_end(x),
        _ => {}
    });
}

/// +6 = 6 (0x5e2be) : élan à pied puis bond.
fn run_leap(x: &mut Cx) {
    chain(x, RESUME_STRIKES, |x, n| match n {
        4 => walk_begin(x),
        6 => pause(x, 9),
        8 => walk_on(x),
        10 => leap_at(x, -0x200),
        12 => leap_wait(x),
        14 => leap_land(x),
        16 => slide(x),
        18 => leap_end(x),
        _ => {}
    });
}

/// +6 = 12 (0x5e3e8) : grand bond.
fn leap_far(x: &mut Cx) {
    chain(x, RESUME_STRIKES, |x, n| match n {
        4 => {
            x.add8(0x7, 2);
            x.set16(0x14, (-0x400i16) as u16);
            leap_body(x);
        }
        6 => leap_wait(x),
        8 => leap_land(x),
        10 => slide(x),
        12 => leap_end(x),
        _ => {}
    });
}

/// +6 = 14 (0x5e410) : saut en hauteur, deux retombées.
fn leap_high(x: &mut Cx) {
    chain(x, RESUME_STRIKES, |x, n| match n {
        4 => {
            x.add8(0x7, 2);
            x.set8(STRIKE_KIND, 0x10);
            x.set8(STRIKE_POWER, 0xa);
            x.set16(0x14, (-0x400i16) as u16);
            x.set8(VULNERABLE, 1);
            if x.u8(f::FLIP) != 0 {
                let vx = x.u16(0x14).wrapping_neg();
                x.set16(0x14, vx);
            }
            x.set16(0x16, 0);
            x.set16(0x18, 0x300);
            x.set16(0x1a, (-0x38i16) as u16);
            strike_play(x, 0xe, 3);
        }
        6 | 10 => leap_wait(x),
        8 => {
            if !x.move_xz().gt() {
                lib::sound(x, 0xecb26);
                dust(x, -0x18);
                x.set32(0xc, 0);
                x.add8(0x7, 2);
                x.set16(0x30, 1);
                x.set8(VULNERABLE, 1);
                x.set16(0x18, 0x200);
                x.set16(0x1a, (-0x38i16) as u16);
            }
            step(x);
        }
        12 => leap_land(x),
        14 => slide(x),
        16 => leap_end(x),
        _ => {}
    });
}

// --- courses (+6 = 8, 10, 16, 18) ------------------------------------------------------------------

/// 0x5e318 : prend son élan en courant vers le côté voulu.
fn run_begin<R: Banc>(x: &mut Cx<'_, R>) {
    x.add8(0x7, 2);
    x.set8(TIMER, 0x42);
    x.set8(VULNERABLE, 1);
    set_angle(x, 0x18, 8);
    pose(x, 0x16, SET_BODY, 4);
    speed(x, 8);
}

/// Revient en garde et teste la portée de la cible (0x5e36a et semblables).
fn back_on_guard(x: &mut Cx) {
    set_state_long(x, ACTIVE_ENGAGE);
    body(x, 2);
    speed(x, 0xb);
    super::stance::engage(x);
}

fn run_wait(x: &mut Cx) {
    let t = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, t);
    if t == 0 {
        return back_on_guard(x);
    }
    step(x);
    move_xy(x);
}

/// +6 = 8 (0x5e304) : course.
fn charge(x: &mut Cx) {
    chain(x, RESUME_STRIKES, |x, n| match n {
        4 => run_begin(x),
        6 => run_wait(x),
        _ => {}
    });
}

/// +6 = 10 (0x5e3a0) : course puis cri.
fn charge_b(x: &mut Cx) {
    chain(x, RESUME_STRIKES, |x, n| match n {
        4 => run_begin(x),
        6 => {
            let t = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, t);
            if t == 0 {
                x.add8(0x7, 2);
                x.set8(WANT_ANIM, 0x1c);
                x.set8(CUR_ANIM, 0x1c);
                return body(x, 0xc);
            }
            step(x);
            move_xy(x);
        }
        8 => leap_end(x),
        _ => {}
    });
}

/// +6 = 16 et 18 : arrêt sur place puis cri.
fn rush_gen(x: &mut Cx, last: impl FnOnce(&mut Cx)) {
    chain(x, RESUME_STRIKES, |x, n| match n {
        4 => walk_begin(x),
        6 => pause(x, 0x1f),
        8 => {
            let t = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, t);
            if t == 0 {
                x.add8(0x7, 2);
                x.set8(TIMER, 0x1e);
                x.set8(VULNERABLE, 1);
                x.set8(WANT_ANIM, 6);
                x.set8(CUR_ANIM, 6);
                return body(x, 0);
            }
            step(x);
            move_xy(x);
        }
        10 => last(x),
        _ => {}
    });
}

fn rush(x: &mut Cx) {
    rush_gen(x, back_on_guard);
}

fn rush_b(x: &mut Cx) {
    rush_gen(x, |x| {
        let t = x.u8(TIMER).wrapping_sub(1);
        x.set8(TIMER, t);
        if t == 0 {
            next_move(x);
        } else {
            step(x);
        }
    });
}

// --- seconde série (sous-état 6) ----------------------------------------------------------------

/// 0x5e64a : petit bond en avant.
fn hop_begin<R: Banc>(x: &mut Cx<'_, R>) {
    x.add8(0x7, 2);
    let vx = if x.u8(f::FLIP) != 0 { -0x200 } else { 0x200 };
    x.set16(0x14, vx as u16);
    x.set8(VULNERABLE, 1);
    x.set8(WANT_ANIM, 0x1a);
    x.set8(CUR_ANIM, 0x1a);
    x.set16(0x16, 0);
    x.set16(0x18, 0x200);
    x.set16(0x1a, (-0x38i16) as u16);
    body(x, 0xd);
}

/// 0x5e690 : attend l'image-clé du petit bond.
fn hop_wait<R: Banc>(x: &mut Cx<'_, R>) {
    if x.u8(f::KEY_FRAME) != 0 {
        x.add8(0x7, 2);
    }
    step(x);
}

/// 0x5e6da : glissade après le petit bond ; elle dure tant que la vitesse en X reste positive.
fn hop_slide(x: &mut Cx) {
    let low = x.a5_u16(0x6e) & 3;
    x.set_a5_u16(0x6e, low);
    if low == 0 {
        dust(x, -0x18);
    }
    if (x.u8(0x14) as i8) >= 0 {
        x.move_x();
        return;
    }
    x.add8(0x7, 2);
    x.set32(0x14, 0);
}

/// 0x5e6a2 : retombée du petit bond, toujours suivie de la glissade.
fn hop_land(x: &mut Cx) {
    x.move_xz();
    lib::sound(x, 0xecb26);
    lib::sound(x, 0xecdba);
    x.set32(0xc, 0);
    x.add8(0x7, 2);
    x.set16(0x30, 1);
    let v = if x.u8(f::FLIP) != 0 { 0x1c } else { -0x1c };
    x.set16(0x16, v as u16);
    step(x);
}

/// 0x5e70e : fin du petit bond, retour en garde.
fn hop_end(x: &mut Cx) {
    if x.anim_step() {
        set_state_long(x, ACTIVE_ENGAGE);
        body(x, 2);
        speed(x, 0xb);
        pursue_tail(x);
    }
}

/// 0x5e750 : course vers le côté de la cible.
fn run_toward_begin<R: Banc>(x: &mut Cx<'_, R>) {
    x.add8(0x7, 2);
    let right = match target_dx(x) {
        Some(dx) => dx >= 0,
        None => false,
    };
    x.set8(Obj::SPEED_INDEX, if right { 8 } else { 0x18 });
    x.set8(TIMER, 0x42);
    x.set8(VULNERABLE, 1);
    pose(x, 0x16, SET_BODY, 4);
    speed(x, 8);
}

/// 0x5e7b4 : la course dure `+0xa0` images, puis retour en garde et reprise de la poursuite.
fn run_toward(x: &mut Cx) {
    let t = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, t);
    if t == 0 {
        set_state_long(x, ACTIVE_ENGAGE);
        body(x, 2);
        speed(x, 0xb);
        return pursue_tail(x);
    }
    step(x);
    move_xy(x);
}
