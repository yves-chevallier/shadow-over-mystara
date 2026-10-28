//! Ennemi « Axe Gnoll » (ENEMY type 13, 0x6ab52). Voir `docs/familles/enemy.md`.
//!
//! Même famille que le Goblin et le Troll (paramètres d'IA, variantes d'arrivée paires, gestes choisis dans des
//! rangées de 32 octets par niveau de difficulté) mais armé d'une hache : chaque coup fixe le type de coup `+0x44`
//! et sa force `+0x45`, l'arme `+0xa6` / `+0xbf` est posée ou ramassée par des routines du programme (0xa09ca,
//! 0xa114a), et plusieurs coups sont des sauts sur la cible.
//!
//! | `+5` | Fichier | Rôle (h) |
//! |---|---|---|
//! | 0 | `arrival` | arrivée selon la variante `+3` (0 marche, 2 et 6 chute, 4 saut de côté, 8 et 10 arrivée armée) |
//! | 2 | `stance` | debout : approche, demi-tour, attente |
//! | 4, 6, 8 | `attacks` | gestes (`+6` = 0 à 24), séries de coups de hache, sauts, téléportation |
//! | 10 | `stance` | attend tant qu'aucun joueur n'est en vie |
//! | état 4 | `hit` | réactions aux coups, recul, chute, saisie par un joueur |
//!
//! Animations 0x1649d0 (mouvements, `+0xa6` ajouté à l'animation de provocation) et 0x1649e8 (coups).

use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

mod arrival;
mod attacks;
mod hit;
mod stance;

pub(self) mod g {
    pub const STRIKE_KIND: u32 = 0x44;
    pub const STRIKE_POWER: u32 = 0x45;
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
    pub const CHOSEN: u32 = 0xa5;
    /// L'arme est en main (la provocation et le relevé utilisent l'animation +1).
    pub const WEAPON: u32 = 0xa6;
    /// Même drapeau, utilisé par l'état touché (animations de chute).
    pub const ARMED: u32 = 0xbf;
}
use g::*;
use crate::objects::pools;
use crate::objects::Courant;

pub(self) const SET_MOVE: u32 = 0x1649d0;
pub(self) const SET_STRIKE: u32 = 0x1649e8;
pub(self) const STANCE: u32 = 0x0202_0000;
pub(self) const IDLE: u32 = 0x020a_0000;

pub fn run(x: &mut Cx) {
    if suspended(x) {
        return;
    }
    if var::GAME_ON.get(x.b) != 0 {
        x.tick_counters();
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
pub(self) fn mv<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_MOVE, n as u16);
}

/// Démarre l'animation `n` du jeu des coups.
pub(self) fn strike(x: &mut Cx, n: u32) {
    x.anim_start(SET_STRIKE, n as u16);
}

/// Démarre le coup `n` : type de coup `kind` et force `power`.
pub(self) fn strike_with(x: &mut Cx, n: u32, kind: u8, power: u8) {
    x.set8(STRIKE_KIND, kind);
    x.set8(STRIKE_POWER, power);
    strike(x, n);
}

pub(self) fn set_speed<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>, n: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
}

/// 0x6ad12 : son puis animation 4.
pub(self) fn wind_up(x: &mut Cx) {
    lib::sound(x, 0xecfe4);
    mv(x, 4);
}

/// Octet tiré au hasard dans la rangée de difficulté de `table` (rangées de 32 octets).
pub(self) fn row_byte(x: &mut Cx, table: u32) -> u8 {
    let row = ((var::RANK.get(x.b) >> 1) << 5) as u32;
    x.c.d[0] = x.c.d[0] & 0xffff_0000 | row;
    let at = table.wrapping_add(row as u16 as i16 as i32 as u32);
    let r = rand_and(x, 0x1f);
    x.data8(at + r)
}

/// 0x6c01a : fin d'un geste ; debout, et si le tirage le veut, attente (`+5 = 10`).
pub(self) fn done_attack<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    set_state_long(x, STANCE);
    if lib::maybe_draw_6e(x) {
        x.set8(f::SUB, 0xa);
    }
}

/// 0x6c04a : un joueur est-il dans la boîte `(d0, d1, d2)` et accepté par les portes d'IA (0xa4b4, et 0xa4c2 s'il
/// regarde dans le même sens) ?
fn box_check(x: &mut Cx, d0: i16, d1: u16, d2: u16) -> bool {
    if !lib::target_in_box(x, d0, d1, d2) {
        return false;
    }
    let target = load_target(x);
    let facing = target.u8(x.b, 0x2c);
    if facing == x.u8(f::FLIP) {
        if !lib::decision_b(x) {
            return false;
        }
    }
    if !lib::decision_a(x) {
        return false;
    }
    true
}

/// 0x6c040 : joueur tout près.
pub(self) fn near_box(x: &mut Cx) -> bool {
    box_check(x, 0, 0x70, 0x10)
}

/// 0x6c032 : joueur à une courte distance, devant.
pub(self) fn far_box(x: &mut Cx) -> bool {
    box_check(x, -0x80, 0x30, 0x10)
}

/// 0x6c078 : le Gnoll tourne le dos à sa cible.
pub(self) fn faces_wrong(x: &mut Cx) -> bool {
    if x.u16(f::TARGET) == 0 {
        return false;
    }
    let target = load_target(x);
    let dx = x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X));
    let want = if (dx as i16) < 0 { 0x20 } else { 0 };
    let wrong = want as u8 != x.u8(f::FLIP);
    wrong
}

pub(self) const NEAR_TABLE: u32 = 0x6c0d6;
pub(self) const FAR_TABLE: u32 = 0x6c2d6;
/// Rangées de gestes qui suivent une série (sous-état 8).
pub(self) const FOLLOW_TABLE: u32 = 0x6be1a;

/// 0x6c0a6 (série proche) / 0x6c0a0 (série lointaine) : choisit le geste (`+5 = 4`, `+6 = 0`).
pub(self) fn pick_attack(x: &mut Cx, table: u32) -> u8 {
    let b = row_byte(x, table);
    x.c.d[0] = x.c.d[0] & !0xff | b as u32;
    x.set8(f::SUB, 4);
    x.set16(f::SUB2, 0);
    x.set8(CHOSEN, b);
    b
}

/// 0x6bdec : après une série, choisit le geste de repli (sous-état 8).
pub(self) fn follow_up(x: &mut Cx) {
    x.set8(AI_SHOT, 0xff);
    let b = row_byte(x, FOLLOW_TABLE);
    x.set8(f::SUB, 8);
    x.set8(f::SUB2, b);
    x.set8(f::SUB3, 0);
}

/// 0x6baf2 : le Gnoll est saisi par un joueur : animation de lâcher, puis il disparaît.
pub(self) fn grab(x: &mut Cx) {
    if paused(x) {
        return;
    }
    if x.u8(f::SUB2) == 0 {
        x.add8(f::SUB2, 2);
        return wind_up(x);
    }
    if x.anim_step() {
        detach_from_host(x);
        x.add8(f::STATE, 2);
    }
}

// --- état 0 : création --------------------------------------------------------------------------

/// Faux si le jeu est en pause (rien n'est fait) ; sinon le corps de l'état actif suit dans la même image.
fn init<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) -> bool {
    if paused(x) {
        return false;
    }
    x.add8(f::STATE, 2);
    x.set8(f::BOX_SET, 0x19);
    lib::initial_hp(x);
    lib::set_immunities(x);
    for (off, v) in [
        (0x68, 4),
        (0x69, 1),
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
    x.set8(WEAPON, 0);
    x.set8(ARMED, 0);
    x.set8(AI_SHOT, 0xff);
    x.set8(0x80, 0xd);
    x.set8(0x81, 0xb);
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
    super::boss_common::push_out_of_boss(x);
    zone_and_terrain(x);
    x.reinsert();
}

handlers! {
    0x6ab52 "enemy_13" run,
}
