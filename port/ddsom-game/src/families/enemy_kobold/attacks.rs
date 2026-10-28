//! Type 0, sous-état 4 : les coups. `+6` : 0 à 6 coups de massue en place, 8 double coup, 10 et 20 bond, 12 coup de patte,
//! 14 et 34 souffle, 16 et 36 pas de côté et demi-tour, 18 course et demi-tour, 22 à 28 série de coups,
//! 30 coup suivi d'un bond, 32 coup de patte court.
//! Chaque geste commence par `begin_wait` (étape 0) et `hold` (étape 2, poursuite en place).

use crate::families::cx::Banc;
use crate::families::eshl_creators;
use super::*;
use crate::objects::Courant;
use crate::objects::Obj;

pub(super) fn run(x: &mut Cx) {
    x.c.d[0] = x.u8(f::SUB2) as u32;
    // étapes communes : 0 attente, 2 poursuite en place (retour à l'état 4)
    match x.u8(f::SUB3) {
        0 => return begin_wait(x),
        2 => return hold(x, 0x0204_0000),
        _ => {}
    }
    match x.u8(f::SUB2) {
        0 => club(x),
        2 => club_b(x),
        4 => thrust(x),
        6 => combo(x),
        8 => double_hit(x),
        10 | 20 => jump_attack(x),
        12 => paw(x),
        14 | 34 => breath(x),
        16 | 36 => sidestep(x),
        18 => dash_turn(x),
        22 => series_a(x),
        24 => series_b(x),
        26 => series_c(x),
        28 => series_d(x),
        30 => strike_then_jump(x),
        32 => short_paw(x),
        _ => {}
    }
}

/// 0x557bc : prépare l'attente.
pub(super) fn begin_wait<R: Banc>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB3, 2);
    x.set8(VULNERABLE, 1);
    x.set8(LOOK, 0);
    mv(x, 0);
}

/// 0x5590e / 0x55902 : poursuite en place jusqu'à la fin du compteur `+0x77`, avec demi-tour si la cible change de
/// côté ; `resume` est l'état à reprendre après le demi-tour.
pub(super) fn hold<R: Banc>(x: &mut Cx<'_, R>, resume: u32) {
    x.set32(RESUME_STATE, resume);
    let n = x.u8(AI_LUNGE).wrapping_sub(1);
    x.set8(AI_LUNGE, n);
    if (n as i8) < 0 {
        return x.add8(f::SUB3, 2);
    }
    if must_turn(x) {
        let sub2 = x.u8(f::SUB2);
        x.set8(RESUME_SUB2, sub2);
        set_state_long(x, 0x0202_0200);
    }
    x.anim_step();
}

/// 0x57924 après un pas en arrière de 0x20 : le Kobold rétablit sa place devant le coup.
fn shift_back<R: Banc>(x: &mut Cx<'_, R>) {
    let dx: u16 = if x.u8(f::FLIP) == 0 { 0xffe0 } else { 0x20 };
    x.add16(Obj::X, dx);
}

/// Fin de coup : animation de marche, recul de 0x20, puis la suite.
fn end_strike(x: &mut Cx) {
    mv(x, 0);
    shift_back(x);
    after_strike(x);
}

/// 0x55a4e : fin du geste.
fn step_then_after(x: &mut Cx) {
    if x.anim_step() {
        after_strike(x);
    }
}

/// Clé d'animation : tir lancé `throw` (0x9dda8 ou 0x9e062 ; la clé est remise à zéro).
fn key_effect<R: Banc>(x: &mut Cx<'_, R>, throw: fn(&mut cps2::Cps2Bus, crate::objects::Obj)) {
    if x.u8(f::KEY_FRAME) != 0 {
        x.set8(f::KEY_FRAME, 0);
        throw(x.b, x.o);
    }
}

/// Début d'un coup : l'aspect `look` est posé ; s'il l'était déjà, `+0xad` décompte et ramène debout à zéro ;
/// sinon l'animation de coups `anim` démarre.
fn strike_look(x: &mut Cx, look: u8, anim: u32) {
    x.set8(LOOK, look);
    if x.u8(LOOK) == x.u8(LOOK_PREV) {
        let g = x.u8(GUARD).wrapping_sub(1);
        x.set8(GUARD, g);
        if g == 0 {
            return set_state_long(x, STANCE);
        }
    }
    x.set8(LOOK_PREV, look);
    strike(x, anim);
}

/// 0x5596a : coup de massue.
fn club_begin(x: &mut Cx, look: u8, kind: u8, anim: u32) {
    x.add8(f::SUB3, 2);
    x.set8(VULNERABLE, 0);
    x.set8(LOOK, look);
    x.set8(STRIKE_KIND, kind);
    set_power(x);
    strike_look(x, look, anim);
}

// --- +6 = 0 à 6 -------------------------------------------------------------------------------------------

fn club(x: &mut Cx) {
    match x.u8(f::SUB3) {
        4 => club_begin(x, 0x2d, 0, 5),
        6 => {
            if x.anim_step() {
                x.set8(AI_SHOT, 0xff);
                end_strike(x);
            }
        }
        _ => {}
    }
}

fn club_b(x: &mut Cx) {
    match x.u8(f::SUB3) {
        4 => club_begin(x, 0x28, 1, 0),
        6 => step_then_after(x),
        _ => {}
    }
}

fn thrust(x: &mut Cx) {
    match x.u8(f::SUB3) {
        4 => {
            x.add8(f::SUB3, 2);
            x.set16(TIMER, 0x14);
            x.set8(STRIKE_KIND, 3);
            set_power(x);
            x.set8(VULNERABLE, 1);
            x.set8(LOOK, 0x16);
            set_angle(x);
            mv(x, 0x16);
            set_speed(x, 0x17);
        }
        6 => {
            let n = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, n);
            if (n as i16) >= 0 {
                x.anim_step();
                x.move_by_table_xy();
                return;
            }
            x.add8(f::SUB3, 2);
            x.set16(Obj::VX, 0x180);
            x.set16(Obj::AX, -0x10i16 as u16);
            if x.u8(f::FLIP) != 0 {
                negate(x, Obj::VX);
                negate(x, Obj::AX);
            }
            strike(x, 0);
        }
        8 => {
            if x.u8(Obj::VX) != 0 {
                x.move_x();
            }
            if x.anim_step() {
                x.set32(Obj::VX, 0);
                after_strike(x);
            }
        }
        _ => {}
    }
}

/// +6 = 6 : coups répétés (`+0xac` fois).
fn combo(x: &mut Cx) {
    match x.u8(f::SUB3) {
        4 => {
            if x.u8(GROUP) != 1 {
                x.set8(AI_SHOT, 2);
            }
            let r = rand_and(x, 1);
            x.set8(REPEAT, 2 + r as u8);
            club_begin(x, 0x2d, 0, 5);
        }
        6 => {
            if !x.anim_step() {
                return;
            }
            mv(x, 0);
            shift_back(x);
            let n = x.u8(REPEAT).wrapping_sub(1);
            x.set8(REPEAT, n);
            if n == 0 {
                return after_strike(x);
            }
            x.set8(AI_SHOT, 0xff);
            strike(x, 5);
        }
        _ => {}
    }
}

/// +6 = 8 : double coup (animations 2 puis 3).
fn double_hit(x: &mut Cx) {
    match x.u8(f::SUB3) {
        4 => {
            x.add8(f::SUB3, 2);
            x.set8(VULNERABLE, 0);
            x.set8(STRIKE_POWER, 5);
            if x.u8(GROUP) != 1 {
                x.set8(AI_SHOT, 2);
            }
            x.set8(LOOK, 0x2a);
            strike(x, 2);
            let r = rand_and(x, 1);
            if r != 0 {
                x.add8(f::SUB3, 2);
                x.set8(LOOK, 0x2b);
                strike(x, 3);
            }
            if x.u8(LOOK) == x.u8(LOOK_PREV) {
                let g = x.u8(GUARD).wrapping_sub(1);
                x.set8(GUARD, g);
                if g == 0 {
                    return set_state_long(x, STANCE);
                }
            }
            let l = x.u8(LOOK);
            x.set8(LOOK_PREV, l);
        }
        6 => step_then_after(x),
        8 => {
            if x.anim_step() {
                x.set8(AI_SHOT, 0xff);
                end_strike(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 10, 20, 30 : bond ---------------------------------------------------------------------------------

/// 0x55c68 : bond d'attaque, vitesse horizontale `vx`.
fn jump_begin(x: &mut Cx, vx: i16, kind: u8) {
    x.set16(Obj::VX, vx as u16);
    x.set8(STRIKE_KIND, kind);
    set_power(x);
    x.add8(f::SUB3, 2);
    x.set8(VULNERABLE, 1);
    x.set8(LOOK, 0x2f);
    if x.u8(f::FLIP) != 0 {
        negate(x, Obj::VX);
    }
    x.set16(Obj::AX, 0);
    x.set16(Obj::VZ, 0x6c0);
    x.set16(Obj::AZ, -0x38i16 as u16);
    if x.u8(LOOK) == x.u8(LOOK_PREV) {
        let g = x.u8(GUARD).wrapping_sub(1);
        x.set8(GUARD, g);
        if g == 0 {
            return set_state_long(x, STANCE);
        }
    }
    let l = x.u8(LOOK);
    x.set8(LOOK_PREV, l);
    strike(x, 7);
}

fn jump_attack(x: &mut Cx) {
    let (vx, kind) = if x.u8(f::SUB2) == 10 { (-0x100, 2) } else { (-0x2a0, 3) };
    jump_series(x, vx, kind);
}

/// Étapes du bond : 4 départ, 6 attend la clé, 8 retombée, 10 fin.
fn jump_series(x: &mut Cx, vx: i16, kind: u8) {
    match x.u8(f::SUB3) {
        4 => jump_begin(x, vx, kind),
        6 => wait_key(x),
        8 => land(x),
        10 => step_then_after(x),
        _ => {}
    }
}

/// 0x54fc4 : retombée d'un saut → animation 0x18 d'atterrissage.
fn land<R: Banc>(x: &mut Cx<'_, R>) {
    if x.move_xz().gt() {
        x.anim_step();
        return;
    }
    x.set32(Obj::Z, 0);
    x.add8(f::SUB3, 2);
    x.set8(VULNERABLE, 1);
    look(x, 0x18);
    mv(x, 0x18);
}

fn strike_then_jump(x: &mut Cx) {
    match x.u8(f::SUB3) {
        4 => series_strike(x),
        6 => {
            key_effect(x, eshl_creators::throw_2);
            if x.anim_step() {
                x.add8(f::SUB3, 2);
                strike(x, 7);
                shift_back(x);
            }
        }
        8 => jump_begin(x, -0x200, 3),
        10 => wait_key(x),
        12 => land(x),
        14 => step_then_after(x),
        _ => {}
    }
}

// --- +6 = 12 : coup de patte --------------------------------------------------------------------------------

/// 0x565d2 : coup de patte avec élan ou en courant (deux variantes tirées au hasard).
pub(super) fn paw_begin(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    let r = rand_and(x, 1);
    if r == 0 {
        x.add8(f::SUB3, 4);
        x.set16(Obj::VX, 0x180);
        if x.u8(f::FLIP) != 0 {
            negate(x, Obj::VX);
        }
        x.set16(Obj::AX, 0);
        x.set16(Obj::VZ, 0x3c0);
        x.set16(Obj::AZ, -0x38i16 as u16);
        look(x, 0x1c);
        return mv(x, 0x1c);
    }
    x.add8(f::SUB3, 2);
    let mut angle = 0x18;
    if x.u16(f::TARGET) != 0 {
        let target = x.linked(f::TARGET);
        let d = target.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X)) as i16;
        if d >= 0 {
            angle = 8;
        }
    }
    x.set8(Obj::SPEED_INDEX, angle);
    look(x, 0x1d);
    x.set16(TIMER, 0x24);
    mv(x, 0x1d);
    set_speed(x, 0xd);
}

fn paw(x: &mut Cx) {
    match x.u8(f::SUB3) {
        4 => paw_begin(x),
        6 => paw_run(x),
        8 => wait_key(x),
        10 => land(x),
        12 => {
            if x.anim_step() {
                back_on_guard(x);
            }
        }
        _ => {}
    }
}

/// 0x55ce2 : course du coup de patte.
fn paw_run(x: &mut Cx) {
    let n = x.u16(TIMER).wrapping_sub(1);
    x.set16(TIMER, n);
    if (n as i16) >= 0 {
        x.move_by_table_xy();
        x.anim_step();
        return;
    }
    back_on_guard(x);
}

/// Le geste fini, le Kobold reprend l'approche (état 2, `+6` = 0, étape 2) et revérifie les portées (0x555da).
pub(super) fn back_on_guard(x: &mut Cx) {
    set_state_long(x, 0x0202_0002);
    x.set8(LOOK, 0xc);
    mv(x, 0xc);
    stance::reach_boxes(x);
}

// --- +6 = 14 et 34 : souffle ---------------------------------------------------------------------------------

fn breath(x: &mut Cx) {
    match x.u8(f::SUB3) {
        4 => {
            x.add8(f::SUB3, 2);
            x.set8(VULNERABLE, 1);
            x.set8(LOOK, 7);
            let r = rand_and(x, 0x1f);
            let n = x.data8(0x55de6 + r);
            x.set8(REPEAT, n);
            if x.u8(LOOK) == x.u8(LOOK_PREV) {
                let g = x.u8(GUARD).wrapping_sub(1);
                x.set8(GUARD, g);
                if g == 0 {
                    return set_state_long(x, STANCE);
                }
            }
            let l = x.u8(LOOK);
            x.set8(LOOK_PREV, l);
            mv(x, 7);
        }
        6 => breath_tick(x),
        _ => {}
    }
}

/// 0x55e06 : la clé d'animation tire parfois un projectile (0xe327a) ; en fin de script, un souffle de moins.
pub(super) fn breath_tick(x: &mut Cx) {
    if x.u8(f::KEY_FRAME) != 0 {
        x.set8(f::KEY_FRAME, 0);
        let r = rand_and(x, 0xf);
        let flag = x.rom8(0x55e50 + r);
        if flag != 0 {
            lib::sound(x, 0xe327a);
        }
    }
    if !x.anim_step() {
        return;
    }
    let n = x.u8(REPEAT).wrapping_sub(1);
    x.set8(REPEAT, n);
    if n == 0 {
        return after_strike(x);
    }
    mv(x, 7);
}

// --- +6 = 16 et 36 : pas de côté ---------------------------------------------------------------------------------

fn sidestep(x: &mut Cx) {
    match x.u8(f::SUB3) {
        4 => {
            x.add8(f::SUB3, 2);
            x.set8(VULNERABLE, 1);
            mv(x, 5);
        }
        6 => {
            if !x.anim_step() {
                return;
            }
            x.add8(f::SUB3, 2);
            x.set8(LOOK, 8);
            x.set16(TIMER, 0x3c);
            if x.u8(LOOK) == x.u8(LOOK_PREV) {
                let g = x.u8(GUARD).wrapping_sub(1);
                x.set8(GUARD, g);
                if g == 0 {
                    return set_state_long(x, STANCE);
                }
            }
            mv(x, 8);
        }
        8 => {
            let n = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, n);
            if (n as i16) >= 0 {
                x.anim_step();
                return;
            }
            x.add8(f::SUB3, 2);
            let flip = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
            mv(x, 5);
        }
        10 => {
            if x.anim_step() {
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                mv(x, 0);
                after_strike(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 18 : course et demi-tour ------------------------------------------------------------------------------

fn dash_turn(x: &mut Cx) {
    match x.u8(f::SUB3) {
        4 => {
            x.add8(f::SUB3, 2);
            x.set8(VULNERABLE, 1);
            let target = x.u16(f::TARGET) != 0;
            let mut angle = 8;
            if target {
                let t = x.linked(f::TARGET);
                let d = t.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X)) as i16;
                angle = if d >= 0 { 0x18 } else { 8 };
            }
            x.set8(Obj::SPEED_INDEX, angle);
            mv(x, 5);
        }
        6 => {
            if !x.anim_step() {
                return;
            }
            x.add8(f::SUB3, 2);
            let flip = x.u8(f::FLIP) ^ 0x20;
            x.set8(f::FLIP, flip);
            x.set8(LOOK, 0x16);
            x.set16(TIMER, 0x2a);
            mv(x, 0x16);
            set_speed(x, 0x17);
        }
        8 => {
            let n = x.u16(TIMER).wrapping_sub(1);
            x.set16(TIMER, n);
            if (n as i16) >= 0 {
                x.anim_step();
                x.move_by_table_xy();
                return;
            }
            x.add8(f::SUB3, 2);
            mv(x, 5);
        }
        10 => {
            if x.anim_step() {
                let flip = x.u8(f::FLIP) ^ 0x20;
                x.set8(f::FLIP, flip);
                back_on_guard(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 22 à 28 : séries de coups ----------------------------------------------------------------------------

/// 0x5603a : coup avec décompte (aspect 0x2e, animation 6).
fn series_strike(x: &mut Cx) {
    x.add8(f::SUB3, 2);
    x.set8(LOOK, 0x2e);
    x.set8(VULNERABLE, 1);
    strike_look(x, 0x2e, 6);
}

fn series_a(x: &mut Cx) {
    match x.u8(f::SUB3) {
        4 => series_strike(x),
        6 => {
            key_effect(x, eshl_creators::throw_1);
            if x.anim_step() {
                end_strike(x);
            }
        }
        _ => {}
    }
}

/// 0x560d2 : prologue des séries : `+0x78`, nombre de coups.
fn series_prologue(x: &mut Cx) {
    if x.u8(GROUP) != 1 {
        x.set8(AI_SHOT, 3);
    }
    x.set8(REPEAT, 1);
    let r = rand_and(x, 1);
    if r != 0 {
        x.set8(REPEAT, 2);
    }
    series_strike(x);
}

fn series_b(x: &mut Cx) {
    match x.u8(f::SUB3) {
        4 => series_strike(x),
        6 => {
            key_effect(x, eshl_creators::throw_2);
            if x.anim_step() {
                x.set8(AI_SHOT, 0xff);
                end_strike(x);
            }
        }
        _ => {}
    }
}

fn series_c(x: &mut Cx) {
    match x.u8(f::SUB3) {
        4 => {
            x.set8(REPEAT, 1);
            series_strike(x);
        }
        6 => {
            key_effect(x, eshl_creators::throw_1);
            if x.anim_step() {
                strike(x, 6);
                let n = x.u8(REPEAT).wrapping_sub(1);
                x.set8(REPEAT, n);
                if n == 0 {
                    mv(x, 0);
                }
                shift_back(x);
                after_strike(x);
            }
        }
        _ => {}
    }
}

fn series_d(x: &mut Cx) {
    match x.u8(f::SUB3) {
        4 => series_prologue(x),
        6 => {
            key_effect(x, eshl_creators::throw_2);
            if x.anim_step() {
                strike(x, 6);
                let n = x.u8(REPEAT).wrapping_sub(1);
                x.set8(REPEAT, n);
                if n == 0 {
                    mv(x, 0);
                }
                shift_back(x);
                after_strike(x);
            }
        }
        _ => {}
    }
}

// --- +6 = 32 : coup de patte court -------------------------------------------------------------------------------

fn short_paw(x: &mut Cx) {
    match x.u8(f::SUB3) {
        4 => {
            x.add8(f::SUB3, 2);
            x.set8(VULNERABLE, 1);
            x.set16(TIMER, 0xe);
            mv(x, 0xc);
            set_speed(x, 0xb);
        }
        6 => paw_run(x),
        _ => {}
    }
}
