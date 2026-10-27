//! Type 15, sous-états 4, 6 et 8 : les gestes. Le sous-état 4 contient les séries de coups, la charge et le repos,
//! le 6 les pas de côté, sauts et téléportations, le 8 le choix qui suit une série.

use super::stance::{approach_start, walk_off};
use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

pub(super) fn run(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    match (x.u8(f::SUB), x.u8(f::SUB2)) {
        (4, 0) | (6, 0) => windup(x),
        (4, 2) | (6, 2) => turn_back(x),
        (4, 4) => series(x),
        (4, 6) | (4, 8) => side_hop(x),
        (4, 10) => maybe_series(x),
        (4, 12) => charge(x),
        (4, 14) => again(x, 0x26),
        (4, 16) => again(x, 0x1c),
        (4, 18) => rest(x),
        (4, 20) => walk_off(x),
        (6, 4) => done_attack(x),
        (6, 6) => depth_step(x),
        (6, 8) => dodge(x),
        (6, 10) => dodge_then_series(x),
        (6, 12) => teleport(x),
        (8, 0) => pause(x),
        (8, 2) => back_step(x),
        (8, 4) => lunge(x),
        (8, 6) => pick_again(x),
        _ => {}
    }
}

/// 0x6e228 : l'étape suivante lance l'animation 4.
fn next_and_start(x: &mut Cx) {
    x.add8(f::SUB3, 2);
    mv(x, 4);
}

/// 0x6e230 : étape suivante à la fin de l'animation.
fn next_when_done(x: &mut Cx) {
    if x.anim_step() {
        x.add8(f::SUB3, 2);
    }
}

/// Étape commune « s'accroupit » (0x6e27e, 0x6e32a, 0x6e502) : fin d'animation, esquive invulnérable `ticks` images.
fn crouch(x: &mut Cx, ticks: u8) {
    if !x.anim_step() {
        return;
    }
    x.add8(f::SUB3, 2);
    x.set8(0x3b, 0);
    x.set_bit(1, 1, true);
    x.set8(TIMER, ticks);
}

/// Étape commune « se déplace de `dist` » (0x6e100, 0x6e34a, 0x6e522) : après l'accroupi, saute de côté.
fn shift(x: &mut Cx, dist: i16) {
    let n = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, n);
    if n != 0 {
        return;
    }
    x.add8(f::SUB3, 2);
    let d = if x.u8(f::FLIP) != 0 { -dist } else { dist };
    let at = x.u16(Obj::X).wrapping_add(d as u16);
    x.set16(Obj::X, at);
    x.set_bit(1, 1, false);
    mv(x, 3);
}

/// Étape commune « tire un coup » (0x6e086, 0x6e3a4) : en cours, la clé d'animation déclenche l'effet.
fn swing_key(x: &mut Cx) -> bool {
    if x.anim_step() {
        return true;
    }
    if x.u8(f::KEY_FRAME) != 0 {
        x.set8(f::KEY_FRAME, 0);
        x.call(call::SWING);
    }
    false
}

// --- geste 0 : provocation, puis geste choisi --------------------------------------------------------

fn windup(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        lib::pick_byte(x);
        if x.u8(AI_LUNGE) == 0 {
            x.set8(f::SUB3, 0);
            let chosen = x.u8(CHOSEN);
            x.set8(f::SUB2, chosen);
        }
        return mv(x, 0);
    }
    x.anim_step();
    let n = x.u8(AI_LUNGE).wrapping_sub(1);
    x.set8(AI_LUNGE, n);
    if (n as i8) >= 0 {
        return;
    }
    x.set8(f::SUB3, 0);
    if faces_wrong(x) {
        x.add8(f::SUB2, 2);
    } else {
        let chosen = x.u8(CHOSEN);
        x.set8(f::SUB2, chosen);
    }
}

/// +6 = 2 : demi-tour puis geste 0xc (marche).
fn turn_back(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        return mv(x, 2);
    }
    if x.anim_step() {
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        x.set16(f::SUB2, 0xc00);
        mv(x, 0);
    }
}

// --- séries de coups ----------------------------------------------------------------------------------

/// 0x6e048 : lance la série (+6 = 4).
fn series_begin(x: &mut Cx) {
    x.set16(f::SUB2, 0x400);
    series(x);
}

/// +6 = 4 : série de coups de la durée tirée dans la rangée de difficulté, puis geste de repli.
fn series(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        let t = row_byte(x, SERIES_TABLE);
        x.set8(TIMER, t);
        return strike(x, 0);
    }
    if !swing_key(x) {
        return;
    }
    let n = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, n);
    if n != 0 {
        return strike(x, 0);
    }
    follow_up(x);
}

/// +6 = 10 : au hasard, retour à la marche (0x1c) ou série.
fn maybe_series(x: &mut Cx) {
    if rand_and(x, 1) == 0 {
        return again(x, 0x1c);
    }
    series_begin(x);
}

/// +6 = 14, 16 (et 10 au tirage) : retour debout et reprise de la marche, `kind` = type de tir mémorisé.
fn again(x: &mut Cx, kind: u8) {
    x.set8(SHOT_KIND, kind);
    set_state_long(x, STANCE);
    approach_start(x);
}

/// +6 = 6 et 8 : pas de côté invulnérable, puis série (6) ou retour à la marche (8).
fn side_hop(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 | 8 => next_when_done(x),
        4 => {
            x.add8(f::SUB3, 2);
            x.set_bit(1, 1, true);
            x.set8(0x3b, 0);
            x.set8(TIMER, 0x14);
        }
        6 => shift(x, 0x40),
        10 => {
            if x.u8(f::SUB2) != 8 {
                return series_begin(x);
            }
            set_state_long(x, STANCE);
            approach_start(x);
        }
        _ => {}
    }
}

/// +6 = 12 : charge vers la cible pendant `+0xa2` images.
fn charge(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(TIMER, 0x40);
        let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
        x.set8(Obj::SPEED_INDEX, angle);
        set_speed(x, 0x12);
        return mv(x, 1);
    }
    x.move_by_table_xy();
    x.anim_step();
    let n = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, n);
    if (n as i8) < 0 {
        x.set16(f::SUB2, 0x400);
    }
}

/// +6 = 18 : repos. L'original sort du repos dès la deuxième image (le test de fin de minuterie est inversé).
fn rest(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(TIMER, 0x1e);
        return mv(x, 0);
    }
    x.anim_step();
    let n = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, n);
    if n != 0 {
        follow_up(x);
    }
}

// --- sous-état 6 --------------------------------------------------------------------------------------

/// +6 = 6 : pas en profondeur.
fn depth_step(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 => next_when_done(x),
        4 => {
            let r = rand_and(x, 1);
            let d: i16 = if r != 0 { -0x18 } else { 0x18 };
            let at = x.u16(Obj::Y).wrapping_add(d as u16);
            x.set16(Obj::Y, at);
            x.add8(f::SUB3, 2);
            mv(x, 3);
        }
        6 => {
            if x.anim_step() {
                done_attack(x);
            }
        }
        _ => {}
    }
}

/// +6 = 8 : esquive sur place.
fn dodge(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 => crouch(x, 0xa),
        4 => {
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if n != 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set_bit(1, 1, false);
            mv(x, 3);
        }
        6 => {
            if x.anim_step() {
                done_attack(x);
            }
        }
        _ => {}
    }
}

/// +6 = 10 : esquive avec saut de côté, puis série de coups terminée par le retour debout.
fn dodge_then_series(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 => crouch(x, 0xa),
        4 => shift(x, 0x40),
        6 => {
            if x.anim_step() {
                x.add8(f::SUB2, 2);
                let t = row_byte(x, SERIES_TABLE);
                x.set8(TIMER, t);
                strike(x, 0);
            }
        }
        8 => {
            if !swing_key(x) {
                return;
            }
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if n != 0 {
                return strike(x, 0);
            }
            done_attack(x);
        }
        _ => {}
    }
}

/// Positions d'arrivée (X puis profondeur) de la téléportation, relatives à la cible.
const TELEPORT_X: u32 = 0x6e44c;
const TELEPORT_Y: u32 = 0x6e43e;

/// +6 = 12 : le Goblin réapparaît près de sa cible.
fn teleport(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 | 6 => next_when_done(x),
        4 => {
            x.add8(f::SUB3, 2);
            let r = rand_and(x, 3) * 2;
            let target = load_target(x);
            let off = x.rom16(TELEPORT_X + r);
            let at = off.wrapping_add(target.u16(x.b, Obj::X));
            x.set16(Obj::X, at);
            let r = rand_and(x, 7) * 2;
            let target = load_target(x);
            let off = x.rom16(TELEPORT_Y + r);
            let at = off.wrapping_add(target.u16(x.b, Obj::Y));
            x.set16(Obj::Y, at);
            x.set8(f::FLIP, 0x20);
            let dx = x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X));
            if (dx as i16) < 0 {
                x.set8(f::FLIP, 0);
            }
            mv(x, 3);
        }
        8 => {
            if x.anim_step() {
                done_attack(x);
            }
        }
        _ => {}
    }
}

// --- sous-état 8 --------------------------------------------------------------------------------------

/// +6 = 0 : courte pause debout.
fn pause(x: &mut Cx) {
    if x.u8(f::SUB3) == 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 1);
        x.set8(TIMER, 0x10);
        return mv(x, 0);
    }
    let n = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, n);
    if (n as i8) < 0 {
        set_state_long(x, STANCE);
    }
    x.anim_step();
}

/// +6 = 2 : pas en arrière.
fn back_step(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => next_and_start(x),
        2 => crouch(x, 0xa),
        4 => shift(x, 0x30),
        6 => {
            if x.anim_step() {
                done_attack(x);
            }
        }
        _ => {}
    }
}

/// +6 = 4 : bond (ou course) vers la cible.
fn lunge(x: &mut Cx) {
    match x.u8(f::SUB3) {
        0 => {
            if rand_and(x, 1) != 0 {
                return next_and_start(x);
            }
            x.add8(f::SUB3, 0xc);
            x.set8(TIMER, 0x10);
            set_speed(x, 0xd);
            x.set8(Obj::SPEED_INDEX, 0);
            let target = load_target(x);
            let d = x.u16(Obj::Y).wrapping_sub(target.u16(x.b, Obj::Y));
            if (d as i16) < 0 {
                x.set8(Obj::SPEED_INDEX, 0x10);
            }
            mv(x, 1);
        }
        2 | 6 => next_when_done(x),
        4 => {
            x.add8(f::SUB3, 2);
            let target = load_target(x);
            let d = x.u16(Obj::Y).wrapping_sub(target.u16(x.b, Obj::Y));
            let step: i16 = if (d as i16) < 0 { -0x32 } else { 0x32 };
            let at = x.u16(Obj::Y).wrapping_add(step as u16);
            x.set16(Obj::Y, at);
            mv(x, 3);
        }
        8 => {
            x.move_xz();
            if (x.u16(Obj::Z) as i16) > 0 {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set32(Obj::Z, 0);
            mv(x, 5);
        }
        10 => {
            if x.anim_step() {
                done_attack(x);
            }
        }
        12 => {
            let n = x.u8(TIMER).wrapping_sub(1);
            x.set8(TIMER, n);
            if (n as i8) < 0 {
                set_state_long(x, STANCE);
            }
            x.move_by_table_xy();
            x.anim_step();
        }
        _ => {}
    }
}

/// +6 = 6 : rechoisit un geste ; ne change de geste que si le nouveau diffère du précédent ou après un délai.
fn pick_again(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    set_state_long(x, IDLE);
    let table = if near_box(x) {
        NEAR_TABLE
    } else if far_box(x) {
        FAR_TABLE
    } else {
        return;
    };
    let b = pick_attack(x, table);
    if b != x.u8(LOOK_PREV) {
        x.set8(LOOK_PREV, b);
        x.set8(COOLDOWN, 2);
        return;
    }
    let n = x.u8(COOLDOWN).wrapping_sub(1);
    x.set8(COOLDOWN, n);
    if (n as i8) < 0 {
        x.set8(COOLDOWN, 2);
    }
}
