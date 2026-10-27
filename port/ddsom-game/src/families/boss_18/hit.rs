//! Boss 18, état 4 « touché » (0x98680). Groupes `+5` : 0 réaction (une routine par code de coup `+0x49`, neuf routines
//! propres), 2 recul (poursuite magique avortée), 4, 6, 8 mort, 10 relevé.
//!
//! Réactions (`+0x49` / 4) : `hit_a` (recul sur place, animation 0), `hit_b` (idem animation 1), `hit_c` (projection selon
//! le code, animation 0), `hit_d`, `hit_e`, `hit_f`, `hit_g` (projections, animation 1), `hit_h` (soulevé, bouffées d'effets),
//! `hit_i` (rebonds jusqu'à la mort).

use super::decide;
use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

/// Table des routines de réaction (mots relatifs à 0x986d6, indexés par `+0x49 / 2`).
const REACTIONS: u32 = 0x986d6;
/// Vitesses de projection (4 mots par ligne) et dérive verticale (0x993ec).
const THROW: u32 = 0x993ac;
const DRIFT: u32 = 0x993ec;

pub(super) fn run(x: &mut Cx) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else {
        match x.u8(f::SUB) {
            0 => reaction(x),
            2 => pushed_back(x),
            4 | 6 | 8 => death(x),
            10 => get_up(x),
            _ => {}
        }
    }
    x.call(IN_ZONE);
    x.call(SHADOW);
}

/// 0x99362 : plus de PV ? Passe alors au groupe 6 (mort) et rend vrai.
fn dead(x: &mut Cx) -> bool {
    if (x.u16(f::HP) as i16) > 0 {
        return false;
    }
    x.set8(f::SUB, 6);
    x.set8(0, 2);
    true
}

fn reaction(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        x.set8(INVULNERABLE, 0);
        set_abs8(x, 0xff8043, 0xff);
        x.call(0x10112);
    }
    let code = (x.u8(REACTION) as u32) >> 1;
    let offset = x.rom16(REACTIONS + code) as i16 as i32;
    let routine = (REACTIONS as i32 + offset) as u32;
    match routine {
        0x9872c => hit_a(x, 0),
        0x9874a => hit_a(x, 1),
        0x987b8 => hit_c(x),
        0x987fc => hit_d(x, 1, 0x20, false, false),
        0x9883a => hit_d(x, 1, 0x38, true, false),
        0x98882 => hit_d(x, 1, 0x2c, false, true),
        0x988c4 => hit_d(x, 1, 0x38, false, true),
        0x98982 => hit_h(x),
        0x98a1e => hit_i(x),
        t => unreachable!("boss 18 : réaction {t:x}"),
    }
}

// --- aides ------------------------------------------------------------------------------------------------

/// 0x99414 : orientation du boss opposée au coup (`+0x4a`) ; vitesses inversées si elle est non nulle.
fn face_away(x: &mut Cx) {
    let side = x.u8(0x4a) ^ 0x20;
    x.set8(f::FLIP, side);
    if side != 0 {
        let (a, b) = (x.u16(0x14).wrapping_neg(), x.u16(0x16).wrapping_neg());
        x.set16(0x14, a);
        x.set16(0x16, b);
    }
}

/// 0x9940c : le gel après le coup (`+0x53`) reprend la valeur d'étourdissement.
fn freeze_by_stun(x: &mut Cx) {
    let stun = x.u8(STUN);
    x.set8(f::FREEZE, stun);
}

/// 0x9937a : vitesses de projection de la ligne `row` des tables.
fn throw_speeds(x: &mut Cx, row: u32) {
    let i = row.wrapping_mul(2) & 0xffff;
    for (off, table) in [(0x14u32, THROW), (0x16, THROW + 2), (0x18, THROW + 4), (0x1a, THROW + 6)] {
        let v = x.rom16(table + i);
        x.set16(off, v);
    }
    let r = (random(x) & 0xf) * 2;
    let v = x.rom16(DRIFT + r);
    x.set16(0x1c, v);
    x.set16(0x1e, 0);
}

fn begin_throw(x: &mut Cx) {
    x.add8(f::SUB2, 2);
    face_away(x);
    freeze_by_stun(x);
    x.move_xyz();
}

// --- hit_a, hit_b : recul sur place --------------------------------------------------------------------------

fn hit_a(x: &mut Cx, anim: u32) {
    if x.u8(f::SUB2) != 0 {
        return slide_back(x);
    }
    if dead(x) {
        return death(x);
    }
    x.anim_start(SET_HURT, anim as u16);
    x.add8(f::SUB2, 2);
    x.set8(SHAKE, 0x10);
    x.set16(0x14, 0x180);
    x.set16(0x16, (-0x30i16) as u16);
    face_away(x);
    freeze_by_stun(x);
}

/// 0x98786 : tremble puis glisse jusqu'à l'arrêt.
fn slide_back(x: &mut Cx) {
    if x.u8(SHAKE) != 0 {
        let mut d: i16 = 2;
        sub8(x, SHAKE, 1);
        if x.u8(SHAKE) & 1 != 0 {
            d = -d;
        }
        if x.u8(f::FLIP) != 0 {
            d = -d;
        }
        x.add16(0x8, d as u16);
        return;
    }
    if x.u8(0x14) == 0 {
        return decide::back_on_feet(x);
    }
    x.move_x();
}

// --- hit_c : projection selon le code ---------------------------------------------------------------------------

fn hit_c(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            if dead(x) {
                return death(x);
            }
            x.anim_start(SET_HURT, 0);
            let code = x.u8(REACTION);
            let row = if code >= 0x58 { 0 } else { (code as u32).wrapping_sub(4) };
            throw_speeds(x, row);
            begin_throw(x);
        }
        _ => thrown(x),
    }
}

/// Étapes communes des projections (`+6` = 2, 4, 6) : montée, retombée, glissade.
fn thrown(x: &mut Cx) {
    match x.u8(f::SUB2) {
        2 => {
            x.move_xyz();
            if (x.u16(0x18) as i16) >= 0 {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(0, 2);
            x.set16(0x16, 0);
        }
        4 => {
            x.move_xyz();
            if (x.u16(0xc) as i16) > 0 {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(0xc, 0);
            x.set16(TIMER, 0x14);
            let v = ((x.u16(0x14) as i16) >> 5).wrapping_neg();
            x.set16(0x16, v as u16);
            x.anim_start(SET_BODY, 5);
        }
        6 => {
            if !sub16(x, TIMER, 1).gt() {
                return decide::back_on_feet(x);
            }
            x.move_xy();
            x.call(0x24a8);
            if !zero(x) {
                return decide::back_on_feet(x);
            }
            x.anim_step();
        }
        _ => {}
    }
}

// --- hit_d, hit_e, hit_f, hit_g : projections ---------------------------------------------------------------------

/// `anim` : animation du jeu de réactions ; `base` : retrait du code pour la ligne de vitesses ; `slow_rise` : divise la vitesse
/// verticale par 4 ; `grounded` : le boss passe à l'état 2 pendant la projection.
fn hit_d(x: &mut Cx, anim: u32, base: u32, slow_rise: bool, grounded: bool) {
    if x.u8(f::SUB2) != 0 {
        return thrown(x);
    }
    if dead(x) {
        return death(x);
    }
    if grounded {
        x.set8(0, 2);
    }
    x.anim_start(SET_HURT, anim as u16);
    let row = (x.u8(REACTION) as u32).wrapping_sub(base);
    throw_speeds(x, row);
    if slow_rise {
        let v = x.u16(0x18) >> 2;
        x.set16(0x18, v);
    }
    begin_throw(x);
}

// --- hit_h : soulevé --------------------------------------------------------------------------------------------------

fn hit_h(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            if (x.u16(0xc) as i16) > 0 {
                x.set16(0x18, 0);
                x.set16(0x1a, (-0x40i16) as u16);
                x.set32(0x14, 0);
                x.set8(f::SUB2, 0);
            }
            x.add8(f::SUB2, 2);
            x.set8(0x86, 0);
            x.set8(0x87, 0);
            x.set8(SHAKE, 0xf0);
            play(x, SET_HURT, 1);
            x.c.d[0] = 4;
            x.c.d[1] = 5;
            loop {
                x.call(call::BURST);
                if carry(x) {
                    x.add8(f::SUB2, 2);
                    return;
                }
                let d1 = (x.c.d[1] as u16).wrapping_sub(1);
                x.c.d[1] = x.c.d[1] & 0xffff_0000 | d1 as u32;
                if d1 == 0xffff {
                    return;
                }
            }
        }
        2 => {
            x.move_xz();
            if (x.u16(0xc) as i16) > 0 {
                return;
            }
            x.set32(0xc, 0);
            x.add8(f::SUB2, 2);
        }
        4 => {
            step(x);
        }
        6 => {
            step(x);
            if sub8(x, SHAKE, 1).eq() {
                decide::back_on_feet(x);
            }
        }
        _ => {}
    }
}

// --- hit_i : rebonds jusqu'à la mort --------------------------------------------------------------------------------

/// Durées (mots) des rebonds successifs (0x98ab8, un par valeur de `+0xa6`).
const BOUNCE_TIMES: u32 = 0x98ab8;

fn hit_i(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.anim_start(SET_HURT, 2);
            x.set32(0x14, 0);
            x.set32(0x18, 0);
            x.set32(0x1c, 0);
            x.set16(TIMER, 6);
            let right_of = x.a5_u16(var::CAMERA1_X).wrapping_add(0xc0);
            let side = if Fl::sub16(right_of, x.u16(0x8)).ge() { 0x20 } else { 0 };
            x.set8(f::FLIP, side);
            bounce_start(x);
        }
        2 => {
            if sub16(x, COUNT, 1).gt() {
                x.move_xz();
                if (x.u16(0xc) as i16) < 0 {
                    x.set16(0xc, 0);
                }
                return;
            }
            let side = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, side);
            if sub16(x, TIMER, 1).pl() {
                return bounce_start(x);
            }
            x.set8(f::FREEZE, 0);
            x.set16(f::HP, 0);
            x.set32(f::STATE, 0x0406_0000);
            set_abs8(x, 0xff804f, 0xff);
        }
        _ => {}
    }
}

/// 0x98a68 : prépare un rebond : élan horizontal de 0x800 vers le côté regardé, hauteur selon l'altitude de la caméra.
fn bounce_start(x: &mut Cx) {
    x.set8(f::SUB2, 2);
    let (mut vx, mut vz) = (0x800i16, -0x40i16);
    if x.u8(f::FLIP) == 0 {
        vx = -vx;
        vz = -vz;
    }
    x.set16(0x14, vx as u16);
    x.set16(0x16, vz as u16);
    let d1 = x.u16(0xc).wrapping_add(x.u16(0x10));
    let d0 = x.a5_u16(var::CAMERA1_Y).wrapping_add(0x80);
    if Fl::sub16(d1, d0).lt() {
        let v = x.u16(0x18).wrapping_add(0x80);
        x.set16(0x18, v);
    } else {
        let v = x.u16(0x18).wrapping_neg();
        x.set16(0x18, v);
    }
    let i = (x.u16(TIMER) as u32).wrapping_mul(2) & 0xffff;
    let n = x.rom16(BOUNCE_TIMES + i);
    x.set16(COUNT, n);
}

// --- +5 = 2 : recul (poursuite magique avortée) ------------------------------------------------------------------

fn pushed_back(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.set16(TIMER, 0x14);
            x.set16(0x1e, 0);
            let (angle, vz) = if x.u8(f::FLIP) == 0 { (8, -0x20i16) } else { (0x18, 0x20) };
            x.set8(Obj::SPEED_INDEX, angle);
            x.set16(0x16, vz as u16);
            x.call(call::RANDOM_TURN);
            speed(x, 0x17);
            x.set8(ANIM_KIND, 0x20);
            x.set8(ANIM_LAST, 0x20);
            x.add8(f::SUB2, 2);
            body(x, 7);
        }
        2 => {
            if sub16(x, TIMER, 1).gt() {
                x.move_xy();
                return step_only(x);
            }
            x.add8(f::SUB2, 2);
            x.set16(TIMER, 0x14);
            x.set16(0x16, 0);
            x.move_xy();
            step_only(x);
        }
        4 => {
            if sub16(x, TIMER, 1).gt() {
                x.move_xy();
                return step_only(x);
            }
            x.set32(0x58, 0);
            decide::attack_6(x);
        }
        _ => {}
    }
}

fn step_only(x: &mut Cx) {
    step(x);
}

// --- mort -------------------------------------------------------------------------------------------------------------

fn death(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set_a5_u8(0x42, 1);
            x.set_a5_u8(0x50, 0);
            let kind = x.u8(0x2);
            set_abs8(x, 0xff2a84, kind);
            set_abs8(x, 0xff2a85, 0);
            let bar = x.u8(0x7e);
            set_abs8(x, 0xff2a86, bar);
            for addr in [0x103ba, 0xe0a10, 0xe3b6c, 0xe5f86, 0xb780, 0xbad6, 0xe2f6] {
                x.call(addr);
            }
            x.set8(0, 2);
            x.set16(0x16, 0);
            x.set16(0x18, 0x400);
            x.set16(0x1a, (-0x28i16) as u16);
            let (mut vx, mut side) = (0x180i16, 0x20u8);
            let edge = x.a5_u16(var::CAMERA1_X).wrapping_add(0xc0);
            if !Fl::sub16(edge, x.u16(0x8)).ge() {
                vx = -vx;
                side = 0;
            }
            x.set16(0x14, vx as u16);
            x.set8(f::FLIP, side);
            x.move_xz();
            x.anim_start(SET_HURT, 2);
        }
        2 => {
            x.move_xz();
            if (x.u16(0xc) as i16) > 0 {
                return;
            }
            x.add8(f::SUB2, 2);
            x.set16(0xc, 0);
        }
        4 => {
            if !x.anim_step() {
                return;
            }
            x.add8(f::SUB2, 2);
            x.call(0xecabc);
        }
        _ => {}
    }
}

// --- +5 = 10 : relevé -------------------------------------------------------------------------------------------------

fn get_up(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        x.add8(0x7, 2);
        return body(x, 6);
    }
    if !step(x) {
        return;
    }
    x.call(0x10474);
    if zero(x) {
        return decide::wait(x);
    }
    decide::back_on_feet(x);
}
