//! Ennemi « Goblin » (ENEMY type 15, 0x6da20). Voir `docs/familles/enemy.md`.
//!
//! Un petit combattant à épée : il marche vers le joueur, choisit un geste dans des rangées de 32 octets (une par
//! niveau de difficulté `A5+0x7f92 / 2`), enchaîne des séries de coups et se place à côté de sa cible.
//!
//! | `+5` | Fichier | Rôle (h) |
//! |---|---|---|
//! | 0 | `arrival` | arrivée selon la variante `+3` (0 marche, 2 et 4 chute, 6 course d'entrée) |
//! | 2 | `stance` | debout : approche, demi-tour, attente, sortie d'écran |
//! | 4, 6, 8 | `attacks` | gestes (`+6` = 0 à 20), séries de coups, pas de côté, saut |
//! | 10 | `stance` | attend tant qu'aucun joueur n'est en vie |
//! | état 4 | `hit` | réactions aux coups, recul, chute, saisie par un joueur |
//!
//! Animations 0x165ee6 (mouvements) et 0x165ef4 (coups). L'état d'initialisation enchaîne directement sur le corps
//! de l'état actif (la même image).

use super::cx::{handlers, sub, Cx};
use super::enemy_common::*;

mod arrival;
mod attacks;
mod hit;
mod stance;

pub(self) mod g {
    pub const VARIANT: u32 = 0x3;
    pub const STEP: u32 = 0x7;
    pub const X: u32 = 0x8;
    pub const HEIGHT: u32 = 0xc;
    pub const DEPTH: u32 = 0x10;
    pub const SPEED_X: u32 = 0x14;
    pub const SPEED_Y: u32 = 0x16;
    pub const SPEED_Z: u32 = 0x18;
    pub const GRAVITY: u32 = 0x1a;
    pub const ANGLE: u32 = 0x2d;
    pub const KEY: u32 = 0x33;
    pub const VULNERABLE: u32 = 0x67;
    pub const AI_WANDER: u32 = 0x6b;
    pub const AI_WAIT: u32 = 0x6e;
    pub const AI_CHARGE: u32 = 0x71;
    pub const AI_LUNGE: u32 = 0x77;
    pub const AI_SHOT: u32 = 0x78;
    pub const SHOT_KIND: u32 = 0x79;
    pub const LOOK_PREV: u32 = 0xa1;
    /// Minuterie (octet) d'un geste.
    pub const TIMER: u32 = 0xa2;
    pub const COOLDOWN: u32 = 0xa4;
    /// Geste choisi (valeur de `+6` à reprendre).
    pub const CHOSEN: u32 = 0xaa;
    /// Non nul : pas de test de zone (arrivée par la course d'entrée).
    pub const NO_ZONE: u32 = 0xae;
}
use g::*;
use crate::objects::pools;

pub(self) const SET_MOVE: u32 = 0x165ee6;
pub(self) const SET_STRIKE: u32 = 0x165ef4;
pub(self) const STANCE: u32 = 0x0202_0000;
pub(self) const HELD: u32 = 0x0408_0000;
pub(self) const IDLE: u32 = 0x020a_0000;

pub(self) mod call {
    pub const FACE_TARGET: u32 = 0xa470;
    pub const TURN_TO_TARGET: u32 = 0x788c;
    pub const RAND_TARGET_CHECK: u32 = 0x79ce;
    pub const ANNOUNCE: u32 = 0xac3a;
    pub const SPEED_TABLE: u32 = 0x1b64;
    pub const WALK_STEP: u32 = 0xe76c;
    pub const REMOVE: u32 = 0x27ee;
    pub const SCREEN_CHECK: u32 = 0x1e90;
    pub const SWING: u32 = 0x9dbd6;
    pub const AI_GATE_A: u32 = 0xa4b4;
    pub const AI_GATE_B: u32 = 0xa4c2;
}

pub fn run(x: &mut Cx) {
    if suspended(x) {
        return;
    }
    if x.a5_u8(0x30) != 0 {
        x.call(rom::TICK_COUNTERS);
    }
    x.c.d[0] = x.u8(f::STATE) as u32;
    match x.u8(f::STATE) {
        st::INIT => {
            if init(x) {
                active(x);
            }
        }
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        st::DEAD => x.free(&pools::ENEMY),
        _ => {}
    }
}

// --- aides --------------------------------------------------------------------------------------

/// Démarre l'animation `n` du jeu des mouvements.
pub(self) fn mv(x: &mut Cx, n: u32) {
    x.anim_start(SET_MOVE, n);
}

/// Démarre l'animation `n` du jeu des coups.
pub(self) fn strike(x: &mut Cx, n: u32) {
    x.anim_start(SET_STRIKE, n);
}

pub(self) fn set_speed(x: &mut Cx, n: u32) {
    x.c.d[0] = n;
    x.call(call::SPEED_TABLE);
}

/// Octet tiré au hasard dans la rangée de difficulté de `table` (rangées de 32 octets).
pub(self) fn row_byte(x: &mut Cx, table: u32) -> u8 {
    let row = ((x.a5_u16(0x7f92) >> 1) << 5) as u32;
    x.c.d[0] = x.c.d[0] & 0xffff_0000 | row;
    let at = table.wrapping_add(row as u16 as i16 as i32 as u32);
    let r = rand_and(x, 0x1f);
    x.data8(at + r)
}

/// 0x6eb04 : fin d'un geste ; debout, et si le tirage le veut, attente (`+5 = 10`).
pub(self) fn done_attack(x: &mut Cx) {
    set_state_long(x, STANCE);
    x.call(call::RAND_TARGET_CHECK);
    if !x.c.cond(7) {
        x.set8(f::SUB, 0xa);
    }
}

/// 0x6eb34 : un joueur est-il dans la boîte `(d0, d1, d2)` et accepté par les portes d'IA (0xa4b4, et 0xa4c2 s'il
/// regarde dans le même sens) ?
fn box_check(x: &mut Cx, d0: i16, d1: u16, d2: u16) -> bool {
    set_dw(x, 0, d0 as u16);
    set_dw(x, 1, d1);
    set_dw(x, 2, d2);
    x.call(0xb5e0);
    if x.c.cond(7) {
        x.c.d[0] = 0;
        return false;
    }
    let target = load_target(x);
    let facing = target.u8(x.b, 0x2c);
    x.c.d[1] = x.c.d[1] & !0xff | facing as u32;
    if facing == x.u8(f::FLIP) {
        x.call(call::AI_GATE_B);
        if x.c.cond(7) {
            x.c.d[0] = 0;
            return false;
        }
    }
    x.call(call::AI_GATE_A);
    if x.c.cond(7) {
        x.c.d[0] = 0;
        return false;
    }
    x.c.d[0] = 1;
    true
}

/// 0x6eb2a : joueur tout près.
pub(self) fn near_box(x: &mut Cx) -> bool {
    x.c.d[0] = 0;
    box_check(x, 0, 0x70, 0x10)
}

/// 0x6eb1c : joueur à une courte distance, devant.
pub(self) fn far_box(x: &mut Cx) -> bool {
    box_check(x, -0xd0, 0x60, 0x18)
}

/// 0x6eb62 : le Goblin tourne le dos à sa cible.
pub(self) fn faces_wrong(x: &mut Cx) -> bool {
    if x.u16(f::TARGET) == 0 {
        x.c.d[0] = 0;
        return false;
    }
    let target = load_target(x);
    let dx = x.u16(X).wrapping_sub(target.u16(x.b, X));
    let want = if (dx as i16) < 0 { 0x20 } else { 0 };
    x.c.d[1] = want;
    let wrong = want as u8 != x.u8(f::FLIP);
    x.c.d[0] = wrong as u32;
    wrong
}

pub(self) const NEAR_TABLE: u32 = 0x6ebc0;
pub(self) const FAR_TABLE: u32 = 0x6edc0;
/// Rangées de durées de séries de coups.
pub(self) const SERIES_TABLE: u32 = 0x6efc0;
/// Rangées de gestes qui suivent une série (sous-état 8).
pub(self) const FOLLOW_TABLE: u32 = 0x6e904;

/// 0x6eb8a (série lointaine) / 0x6eb90 (série proche) : choisit le geste (`+5 = 4`, `+6 = 0`).
pub(self) fn pick_attack(x: &mut Cx, table: u32) -> u8 {
    x.c.d[2] = 0;
    let b = row_byte(x, table);
    x.c.d[0] = x.c.d[0] & !0xff | b as u32;
    x.set8(f::SUB, 4);
    x.set16(f::SUB2, 0);
    x.set8(CHOSEN, b);
    b
}

/// 0x6e8d6 : après une série, choisit le geste de repli (sous-état 8).
pub(self) fn follow_up(x: &mut Cx) {
    x.set8(AI_SHOT, 0xff);
    let b = row_byte(x, FOLLOW_TABLE);
    x.set8(f::SUB, 8);
    x.set8(f::SUB2, b);
    x.set8(STEP, 0);
}

/// 0x6e656 : le Goblin est saisi par un joueur : animation de lâcher, puis il disparaît.
pub(self) fn grab(x: &mut Cx) {
    if paused(x) {
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        return mv(x, 4);
    }
    if x.anim_step() {
        x.call(call::REMOVE);
        x.add8(f::STATE, 2);
    }
}

// --- état 0 : création --------------------------------------------------------------------------

/// Faux si le jeu est en pause (rien n'est fait) ; sinon le corps de l'état actif suit dans la même image.
fn init(x: &mut Cx) -> bool {
    if paused(x) {
        return false;
    }
    x.add8(f::STATE, 2);
    x.set8(f::BOX_SET, 0x1b);
    x.call(rom::INITIAL_HP);
    x.call(rom::IMMUNITIES);
    x.c.d[0] = 0;
    for (off, v) in [
        (0x68, 2),
        (0x69, 4),
        (0x70, 0),
        (0x6a, 1),
        (0x6c, 3),
        (0x6d, 3),
        (0x72, 3),
        (0x73, 2),
        (0x74, 3),
        (0x75, 0x38),
        (0x76, 3),
        (TIMER, 0x20),
        (LOOK_PREV, 0),
        (COOLDOWN, 2),
        (VULNERABLE, 1),
        (0x8e, 0),
        (0x7f, 0),
        (0x7e, 0xe),
    ] {
        x.set8(off, v);
    }
    x.set32(0x8a, 0);
    x.set8(AI_SHOT, 0xff);
    x.set8(0x80, 0xe);
    x.set8(0x81, 0xc);
    x.set8(0xab, 0x20);
    x.set8(NO_ZONE, 0);
    true
}

// --- état 2 : actif -----------------------------------------------------------------------------

fn active(x: &mut Cx) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else if !paused(x) {
        x.c.d[0] = x.u8(f::SUB) as u32;
        match x.u8(f::SUB) {
            0 => arrival::run(x),
            2 => stance::run(x),
            4 | 6 | 8 => attacks::run(x),
            10 => stance::idle(x),
            _ => {}
        }
    }
    if x.u8(f::SUB) != 0 {
        zone_shadow(x);
    }
    x.reinsert();
}

handlers! {
    0x6da20 "enemy_15" run,
}
