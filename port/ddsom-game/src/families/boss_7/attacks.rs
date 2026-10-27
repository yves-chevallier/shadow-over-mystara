//! Boss 7, groupe 4 : les attaques (`+6` = 0 à 16), chacune en étapes `+7` = 0, 2, 4...
//! À la fin d'une attaque, `aids::after_attack` choisit la suite.

use super::aids;
use super::*;
use crate::objects::Courant;

/// Étape finale commune des attaques (0x86286) : joue l'animation jusqu'au bout, puis choisit la suite.
fn finish(x: &mut Cx) {
    if x.anim_step() {
        aids::after_attack(x);
    }
}

fn step(x: &mut Cx, table: u32) -> u32 {
    dispatch(x, table, f::SUB3)
}

pub(super) fn run(x: &mut Cx) {
    match dispatch(x, 0x86238, f::SUB2) {
        0x8624a => slash(x),
        0x86296 => combo(x),
        0x86334 => throw_knives(x),
        0x86400 => plunge(x),
        0x864f2 => charge(x),
        0x86620 => leap(x),
        0x8673c => rain(x),
        _ => vanish(x),
    }
}

/// 0x8624a (`+6` = 0) : un seul coup.
fn slash(x: &mut Cx) {
    if step(x, 0x86258) == 0x8625c {
        addq8(x, f::SUB3, 2);
        x.set8(0x45, 5);
        x.set8(0x44, 0x44);
        key(x, 0xa);
        strike(x, 0);
    } else {
        finish(x)
    }
}

/// 0x86296 (`+6` = 2) : coup en trois temps (le boss ne frappe qu'aux images clés `+0x33`).
fn combo(x: &mut Cx) {
    match step(x, 0x862a4) {
        0x862ac => {
            addq8(x, f::SUB3, 2);
            key(x, 0xb);
            x.set8(PHASE_TIMER, 0x1e);
            strike(x, 1);
        }
        0x862d0 => {
            if x.u8(f::KEY_FRAME) != 0 {
                addq8(x, f::SUB3, 2);
                x.set8(f::KEY_FRAME, 0);
                x.call(0xb4ef4);
                lib::sound(x, 0xed25a);
            }
            x.anim_step();
        }
        0x862f2 => {
            if x.u8(f::KEY_FRAME) != 0 {
                x.set8(VULNERABLE, 0);
                addq8(x, f::SUB3, 2);
                x.set8(f::KEY_FRAME, 0);
                eshl_burst_27(x);
                lib::sound(x, 0xed1f0);
            }
            x.anim_step();
        }
        _ => {
            if x.u8(HIT_ACK) != 0 {
                x.set8(VULNERABLE, 1);
                aids::after_attack(x);
                x.set8(HIT_ACK, 0);
            }
            x.anim_step();
        }
    }
}

/// Lance un projectile (0xb4f44, D1 = 1) vers un joueur choisi par `pick`.
fn shoot(x: &mut Cx, pick: fn(&mut Cx)) {
    pick(x);
    x.c.d[1] = 1;
    x.call(0xb4f44);
}

/// Début d'une rafale de projectiles (clé `kind`).
fn volley_start(x: &mut Cx, kind: u8, rain: bool) {
    addq8(x, f::SUB3, 2);
    if !rain {
        x.set8(PHASE_TIMER, 4);
        x.set8(0x45, 5);
    }
    x.set8(0x44, 0x45);
    key(x, kind);
    x.set8(PICKED, 0);
    if rain {
        x.set8(BURSTS, 2);
    }
    strike(x, 3);
}

/// Premier tir d'une rafale : sons, effet, puis un tir par joueur en vie.
fn volley_fire(x: &mut Cx, pick: fn(&mut Cx)) {
    if !x.anim_step() {
        return;
    }
    lib::shake_58(x, 0x30);
    lib::sound(x, 0xedd2c);
    addq8(x, f::SUB3, 2);
    x.set16(TIMER, 0x32);
    x.call(0xb4f60);
    let n = var::PLAYERS_ALIVE.get(x.b);
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

/// 0x86334 (`+6` = 4, 8) : rafale de couteaux, un vers chaque joueur.
fn throw_knives(x: &mut Cx) {
    match step(x, 0x86342) {
        0x8634a => volley_start(x, 0xc, false),
        0x8637e => volley_fire(x, aids::pick_new),
        0x863cc => {
            volley_next(x, aids::pick_new);
            if timer_done(x) {
                addq8(x, f::SUB3, 2);
                strike(x, 5);
            }
        }
        _ => finish(x),
    }
}

/// 0x8673c (`+6` = 14) : pluie de projectiles, répétée tant que `+0xaa` le permet.
fn rain(x: &mut Cx) {
    match step(x, 0x8674a) {
        0x86752 => volley_start(x, 0x11, true),
        0x86780 => volley_fire(x, aids::pick_any),
        0x867ce => {
            volley_next(x, aids::pick_any);
            if !timer_done(x) {
                return;
            }
            if sub8(x, BURSTS, 1).mi() {
                addq8(x, f::SUB3, 2);
                strike(x, 5);
            } else {
                sub8(x, f::SUB3, 2);
                strike(x, 0xb);
            }
        }
        _ => finish(x),
    }
}

/// 0x86400 (`+6` = 6) : coup plongeant sur la position retenue de la cible.
fn plunge(x: &mut Cx) {
    match step(x, 0x8640e) {
        0x86414 => {
            if x.u16(f::TARGET) == 0 {
                return;
            }
            addq8(x, f::SUB3, 2);
            let t = load_target(x);
            let (tx, ty) = (t.u16(x.b, 0x8), t.u16(x.b, 0x10));
            x.set16(TARGET_X, tx);
            x.set16(TARGET_Y, ty);
            key(x, 0xd);
            strike(x, 2);
        }
        0x8644a => {
            if x.u8(f::KEY_FRAME) != 0 {
                addq8(x, f::SUB3, 2);
                x.set8(f::KEY_FRAME, 0);
                x.c.d[1] = 3;
                x.call(0xb4f44);
            }
            x.anim_step();
        }
        _ => finish(x),
    }
}

/// 0x864f2 (`+6` = 10) : charge latérale avec bouffées de poussière, puis arrêt.
fn charge(x: &mut Cx) {
    match step(x, 0x86500) {
        0x8650a => {
            addq8(x, f::SUB3, 2);
            x.set8(PHASE_TIMER, 4);
            x.set8(0x45, 5);
            x.set8(0x44, 0x46);
            key(x, 0xf);
            strike(x, 7);
        }
        0x8653a => {
            if x.u8(f::KEY_FRAME) == 0 {
                x.anim_step();
                return;
            }
            addq8(x, f::SUB3, 2);
            x.set8(f::KEY_FRAME, 0);
            lib::sound(x, 0xecdba);
            let (mut d0, mut d1) = (-0x800i16, 0x20i16);
            if x.u8(f::FLIP) != 0 {
                d0 = d0.wrapping_neg();
                d1 = d1.wrapping_neg();
            }
            x.set16(0x14, d0 as u16);
            x.set16(0x16, d1 as u16);
        }
        0x86574 => {
            dust(x, 0);
            x.move_x();
            if x.anim_step() {
                addq8(x, f::SUB3, 2);
                x.set16(TIMER, 0x14);
                strike(x, 8);
            }
        }
        0x865ba => {
            dust(x, 1);
            if timer_done(x) {
                addq8(x, f::SUB3, 2);
                x.set16(TIMER, 1);
                let v = x.u16(0x14).wrapping_neg();
                x.set16(0x14, v);
            }
            x.move_x();
            x.anim_step();
        }
        _ => {
            if timer_done(x) {
                x.set16(TIMER, 0);
                x.set16(0x14, 0);
                x.set16(0x16, 0);
                x.set8(0, 1);
                return aids::after_attack(x);
            }
            x.move_x();
            x.anim_step();
        }
    }
}

/// Toutes les 4 images, une bouffée de poussière sous les pieds (`andi.w #3,(0x6e,A5) ; beq`), de sorte `kind`.
fn dust(x: &mut Cx, kind: u32) {
    let v = x.a5_u16(0x6e) & 3;
    x.set_a5_u16(0x6e, v);
    if v == 0 {
        lib::shock(x, kind, 1, 0xffe0, 0xffff);
    }
}

/// 0x86620 (`+6` = 12) : bond vers l'avant avec chute, atterrissage et gerbe d'effets.
fn leap(x: &mut Cx) {
    match step(x, 0x8662e) {
        0x8663a => leap_start(x, -0x400),
        t => leap_rest(x, t),
    }
}

/// Début du bond : élan horizontal `vx` (inversé si le boss regarde à gauche).
pub(super) fn leap_start(x: &mut Cx, vx: i16) {
    addq8(x, f::SUB3, 2);
    x.set8(VULNERABLE, 1);
    key(x, 0x10);
    x.set8(0x44, 0x45);
    x.set16(0x14, vx as u16);
    if x.u8(f::FLIP) != 0 {
        let v = x.u16(0x14).wrapping_neg();
        x.set16(0x14, v);
    }
    x.set16(0x16, 0);
    x.set16(0x18, 0x900);
    x.set16(0x1a, (-0x88i16) as u16);
    strike(x, 9);
}

/// Étapes communes du bond (0x86686, 0x8669c, 0x866c2, 0x8670c, 0x86728).
pub(super) fn leap_rest(x: &mut Cx, t: u32) {
    match t {
        0x86686 => {
            if x.u8(f::KEY_FRAME) != 0 {
                addq8(x, f::SUB3, 2);
                x.set8(VULNERABLE, 0);
            }
            x.anim_step();
        }
        0x8669c => {
            x.move_xz();
            if (x.u16(0x18) as i16) <= -0x300 {
                x.set16(0x1a, (-0x98i16) as u16);
                addq8(x, f::SUB3, 2);
                x.set16(0x30, 1);
            }
            x.anim_step();
        }
        0x866c2 => {
            if x.move_xz().gt() {
                x.anim_step();
                return;
            }
            lib::shake_58(x, 0x14);
            lib::sound(x, 0xecb98);
            aids::burst_small(x);
            x.set32(0xc, 0);
            addq8(x, f::SUB3, 2);
            x.set8(VULNERABLE, 1);
            x.set16(0x30, 1);
            x.set16(TIMER, 0x14);
            strike(x, 0xa);
        }
        0x8670c => {
            if timer_done(x) {
                addq8(x, f::SUB3, 2);
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

/// 0x8681a (`+6` = 16) : disparition, pas de côté, réapparition.
fn vanish(x: &mut Cx) {
    match step(x, 0x86828) {
        0x86830 => {
            addq8(x, f::SUB3, 2);
            x.set8(0, 2);
            key(x, 0x12);
            lib::sound(x, 0xecf7a);
            body(x, 4);
        }
        0x86858 => {
            if x.anim_step() {
                addq8(x, f::SUB3, 2);
                aids::random_sidestep(x);
                x.set_bit(0x1, 1, true);
                x.set16(TIMER, 8);
                body(x, 5);
            }
        }
        0x86886 => {
            if timer_done(x) {
                addq8(x, f::SUB3, 2);
                x.set_bit(0x1, 1, false);
                x.set8(0, 1);
                lib::sound(x, 0xecf7a);
            }
        }
        _ => finish(x),
    }
}
