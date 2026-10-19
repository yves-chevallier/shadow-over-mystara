//! Boss 8, groupe 4 : les attaques (`+6` = 0 à 18), chacune en étapes `+7` = 0, 2, 4...
//! À la fin d'une attaque, `aids::after_attack` choisit la suite.

use super::aids;
use super::*;

/// Clé d'animation `kind` (`+0xa2`, recopiée en `+0xa3`).
fn key(x: &mut Cx, kind: u8) {
    x.set8(ANIM_KIND, kind);
    x.set8(ANIM_LAST, kind);
}

/// Fin d'une étape de décompte : le décompte de `+0xa0` est-il écoulé ?
fn timer_done(x: &mut Cx) -> bool {
    sub16(x, TIMER, 1).mi()
}

/// Étape finale commune des attaques (0x8880a) : joue l'animation jusqu'au bout, puis choisit la suite.
fn finish(x: &mut Cx) {
    if x.anim_step() {
        aids::after_attack(x);
    }
}

pub(super) fn run(x: &mut Cx) {
    match dispatch(x, 0x887ba, f::SUB2) {
        0x887ce => slash(x),
        0x88e0e => lunge(x, true),
        0x888ac => throw_knives(x),
        0x8881a => combo(x),
        0x88a64 => charge(x),
        0x88b92 => leap(x),
        0x88cae => rain(x),
        0x88d86 => vanish(x),
        _ => lunge(x, false),
    }
}

fn step(x: &mut Cx, table: u32) -> u32 {
    dispatch(x, table, SUB3)
}

/// 0x887ce (`+6` = 0) : un seul coup.
fn slash(x: &mut Cx) {
    if step(x, 0x887dc) == 0x887e0 {
        addq8(x, SUB3, 2);
        x.set8(0x45, 5);
        x.set8(0x44, 0x44);
        key(x, 0xa);
        strike(x, 0);
    } else {
        finish(x)
    }
}

/// 0x8881a (`+6` = 8) : coup en trois temps (le boss ne frappe qu'aux images clés `+0x33`).
fn combo(x: &mut Cx) {
    match step(x, 0x88828) {
        0x88830 => {
            addq8(x, SUB3, 2);
            key(x, 0xb);
            x.set8(PHASE_TIMER, 0x1e);
            strike(x, 1);
        }
        0x88854 => {
            if x.u8(KEY) != 0 {
                addq8(x, SUB3, 2);
                x.set8(KEY, 0);
                x.call(0xe42c6);
            }
            x.call(call::STEP);
        }
        0x88870 => {
            if x.u8(KEY) != 0 {
                x.set8(VULNERABLE, 1);
                addq8(x, SUB3, 2);
                x.set8(KEY, 0);
                x.call(0xa5c9a);
            }
            x.call(call::STEP);
        }
        _ => {
            if x.u8(HIT_ACK) != 0 {
                x.set8(VULNERABLE, 0);
                aids::after_attack(x);
                x.set8(HIT_ACK, 0);
            }
            x.call(call::STEP);
        }
    }
}

/// Lance un projectile (0xb4f44, D1 = 5) vers un joueur choisi par `pick`.
fn shoot(x: &mut Cx, pick: fn(&mut Cx)) {
    pick(x);
    x.c.d[1] = 5;
    x.call(0xb4f44);
}

/// 0x888ac (`+6` = 4, 6) et 0x88cae (`+6` = 14) : rafale de projectiles, chacun vers un joueur.
fn volley_start(x: &mut Cx, kind: u8, extra: bool) {
    addq8(x, SUB3, 2);
    if !extra {
        x.set8(PHASE_TIMER, 0xa);
        x.set8(0x45, 5);
        x.set8(0x44, 0x45);
    } else {
        x.set8(0x44, 0x45);
    }
    key(x, kind);
    x.set8(PICKED, 0);
    if extra {
        x.set8(GRABBED, 2);
    }
    strike(x, 3);
}

/// Premier tir d'une rafale : sons, effet, puis un tir par joueur en vie.
fn volley_fire(x: &mut Cx, pick: fn(&mut Cx)) {
    x.c.d[0] = 0x30;
    x.call(call::SOUND_B);
    x.call(0xedd2c);
    addq8(x, SUB3, 2);
    x.set16(TIMER, 0x32);
    let n = x.a5_u8(var::PLAYERS_ALIVE);
    x.set8(VOLLEY, n);
    sub8(x, VOLLEY, 1);
    shoot(x, pick);
    strike(x, 4);
}

/// Un tir de plus tant qu'il reste des joueurs à viser.
fn volley_next(x: &mut Cx, pick: fn(&mut Cx)) {
    let f = sub8(x, VOLLEY, 1);
    if !f.eq() && !f.mi() {
        shoot(x, pick);
    }
}

fn throw_knives(x: &mut Cx) {
    match step(x, 0x888ba) {
        0x888c2 => volley_start(x, 0xc, false),
        0x888f6 => {
            if x.anim_step() {
                volley_fire(x, aids::pick_new);
            }
        }
        0x8893e => {
            volley_next(x, aids::pick_new);
            if timer_done(x) {
                addq8(x, SUB3, 2);
                strike(x, 5);
            }
        }
        _ => finish(x),
    }
}

/// 0x88cae (`+6` = 14) : pluie de projectiles, répétée jusqu'à épuisement de `+0xaa`.
fn rain(x: &mut Cx) {
    match step(x, 0x88cbc) {
        0x88cc4 => volley_start(x, 0x11, true),
        0x88cf2 => {
            if x.anim_step() {
                volley_fire(x, aids::pick_any);
            }
        }
        0x88d3a => {
            volley_next(x, aids::pick_any);
            if !timer_done(x) {
                return;
            }
            if (sub8(x, GRABBED, 1)).mi() {
                addq8(x, SUB3, 2);
                strike(x, 5);
            } else {
                sub8(x, SUB3, 2);
                strike(x, 0xb);
            }
        }
        _ => finish(x),
    }
}

/// 0x88a64 (`+6` = 10) : charge latérale avec bouffées de poussière, puis arrêt.
fn charge(x: &mut Cx) {
    match step(x, 0x88a72) {
        0x88a7c => {
            addq8(x, SUB3, 2);
            x.set8(PHASE_TIMER, 0xa);
            x.set8(0x45, 5);
            x.set8(0x44, 0x46);
            key(x, 0xf);
            strike(x, 7);
        }
        0x88aac => {
            if x.u8(KEY) == 0 {
                return x.call(call::STEP);
            }
            addq8(x, SUB3, 2);
            x.set8(KEY, 0);
            x.call(0xecdba);
            let (mut d0, mut d1) = (-0x800i16, 0x20i16);
            if x.u8(f::FLIP) != 0 {
                d0 = d0.wrapping_neg();
                d1 = d1.wrapping_neg();
            }
            setw(x, 0, d0 as u16);
            setw(x, 1, d1 as u16);
            x.set16(0x14, d0 as u16);
            x.set16(0x16, d1 as u16);
        }
        0x88ae6 => {
            dust(x);
            x.call(call::MOVE_X);
            if x.anim_step() {
                addq8(x, SUB3, 2);
                x.set16(TIMER, 0x14);
                strike(x, 8);
            }
        }
        0x88b2c => {
            dust(x);
            if sub16(x, TIMER, 1).mi() {
                addq8(x, SUB3, 2);
                x.set16(TIMER, 1);
                let v = x.u16(0x14).wrapping_neg();
                x.set16(0x14, v);
            }
            x.call(call::MOVE_X);
            x.call(call::STEP);
        }
        _ => {
            if sub16(x, TIMER, 1).mi() {
                x.set16(TIMER, 0);
                x.set16(0x14, 0);
                x.set16(0x16, 0);
                x.set8(0, 1);
                return aids::after_attack(x);
            }
            x.call(call::MOVE_X);
            x.call(call::STEP);
        }
    }
}

/// Toutes les 4 images, une bouffée de poussière sous les pieds (`andi.w #3,(0x6e,A5) ; beq`).
fn dust(x: &mut Cx) {
    let v = x.a5_u16(0x6e) & 3;
    x.set_a5_u16(0x6e, v);
    if v == 0 {
        x.c.d[0] = 1;
        x.c.d[1] = 1;
        setw(x, 2, (-0x20i16) as u16);
        setw(x, 3, 0xffff);
        x.call(call::BURST);
    }
}

/// 0x88b92 (`+6` = 12) : bond avec chute, atterrissage et gerbe d'effets.
fn leap(x: &mut Cx) {
    match dispatch(x, 0x88ba0, SUB3) {
        0x88bac => leap_start(x, -0x400, 0x10, 9),
        t => leap_rest(x, t),
    }
}

/// Début du bond : élan horizontal `vx` (inversé si le boss regarde à gauche), clé d'animation `kind`.
pub(super) fn leap_start(x: &mut Cx, vx: i16, kind: u8, anim: u32) {
    addq8(x, SUB3, 2);
    x.set8(VULNERABLE, 1);
    key(x, kind);
    x.set8(0x44, 0x45);
    x.set16(0x14, vx as u16);
    if x.u8(f::FLIP) != 0 {
        let v = x.u16(0x14).wrapping_neg();
        x.set16(0x14, v);
    }
    x.set16(0x16, 0);
    x.set16(0x18, 0x900);
    x.set16(0x1a, (-0x88i16) as u16);
    strike(x, anim);
}

/// Étapes communes du bond (0x88bf8, 0x88c0e, 0x88c34, 0x88c7e, 0x88c9a).
pub(super) fn leap_rest(x: &mut Cx, t: u32) {
    match t {
        0x88bf8 => {
            if x.u8(KEY) != 0 {
                addq8(x, SUB3, 2);
                x.set8(VULNERABLE, 0);
            }
            x.call(call::STEP);
        }
        0x88c0e => {
            x.call(call::MOVE_XZ);
            if !Fl::sub16(x.u16(0x18), 0xfd00).gt() || true {
                // cmpi.w #-0x300,(0x18,A0) ; bgt : l'élan vertical se réduit
            }
            if (x.u16(0x18) as i16) <= -0x300 {
                x.set16(0x1a, (-0x98i16) as u16);
                addq8(x, SUB3, 2);
                x.set16(0x30, 1);
            }
            x.call(call::STEP);
        }
        0x88c34 => {
            x.call(call::MOVE_XZ);
            if x.c.cond(14) {
                return x.call(call::STEP);
            }
            x.c.d[0] = 0x14;
            x.call(call::SOUND_B);
            x.call(0xecb98);
            aids::burst_small(x);
            x.set32(0xc, 0);
            addq8(x, SUB3, 2);
            x.set8(VULNERABLE, 1);
            x.set16(0x30, 1);
            x.set16(TIMER, 0x14);
            strike(x, 0xa);
        }
        0x88c7e => {
            if timer_done(x) {
                addq8(x, SUB3, 2);
                strike(x, 5);
            }
        }
        _ => {
            if x.anim_step() {
                x.set16(TIMER, 0);
                aids::after_attack(x);
            }
        }
    }
}

/// 0x88d86 (`+6` = 16) : disparition, réapparition à côté.
fn vanish(x: &mut Cx) {
    match step(x, 0x88d94) {
        0x88d9c => {
            addq8(x, SUB3, 2);
            x.set8(PHASE_TIMER, 0x10);
            key(x, 0x12);
            x.call(0xecf7a);
            body(x, 4);
        }
        0x88dc6 => {
            if x.anim_step() {
                addq8(x, SUB3, 2);
                aids::random_sidestep(x);
                x.set_bit(0x1, 1, true);
                x.set16(TIMER, 8);
                body(x, 5);
            }
        }
        0x88df4 => {
            if timer_done(x) {
                addq8(x, SUB3, 2);
                x.set_bit(0x1, 1, false);
                x.call(0xecf7a);
            }
        }
        _ => finish(x),
    }
}

/// 0x88e0e (`+6` = 2, `first`) et 0x88e50 (`+6` = 18) : bond en avant puis coup (0xa4e84 reçoit D4).
fn lunge(x: &mut Cx, first: bool) {
    let t = step(x, if first { 0x88e1c } else { 0x88e5e });
    match t {
        0x88e66 => {
            addq8(x, SUB3, 2);
            x.set8(PHASE_TIMER, 0xa);
            key(x, 0x13);
            strike(x, 0xc);
        }
        0x88e24 | 0x88e8a => {
            if x.anim_step() {
                addq8(x, SUB3, 2);
                x.set16(TIMER, 0x20);
                x.c.d[4] = first as u32;
                x.call(0xa4e84);
                strike(x, 0xd);
            }
        }
        0x88eb6 => {
            if timer_done(x) {
                addq8(x, SUB3, 2);
                strike(x, 0xe);
            } else {
                x.call(call::STEP);
            }
        }
        _ => finish(x),
    }
}
