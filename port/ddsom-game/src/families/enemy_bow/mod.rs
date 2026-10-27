//! Ennemi « Bow Gnoll » (ENEMY type 3, 0x5bda6). Voir `docs/familles/enemy.md`.
//!
//! Un archer : il marche vers sa cible, tire (`+0x44` = 0xa, `+0x45` = 4 pour la flèche, le tir lui-même est créé
//! par la clé d'animation avec 0x9dbd6), recule, esquive et saute. Un Bow Gnoll créé par un autre objet (`+0x42`)
//! ne suit pas l'IA : il tire en place (`spawned`). Un `+0x43` non nul donne une durée de vie (`+0xa8`).
//!
//! | `+5` | Fichier | Rôle (h) |
//! |---|---|---|
//! | 0 | `arrival` | arrivée selon la variante `+3` (0 et 2 marche, 4 chute, 6 apparition, 8 course) |
//! | 2 | `stance` | debout : approche, marche, choix d'un geste |
//! | 4, 6 | `shoot` | tirs, pas de côté, sauts |
//! | 8, 10, 12 | `stance` | repos, attente, demi-tour |
//! | état 4 | `hit` | réactions aux coups, chute, saisie par un joueur |
//!
//! Animations 0x16032c (mouvements) et 0x160350 (tirs). Le Bow Gnoll n'a pas de test « joueur en vie » : l'attente
//! (`+5` = 10) revient au sous-état 2 quand le compte `+0x6e` est écoulé.

use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

mod arrival;
mod hit;
mod shoot;
mod spawned;
mod stance;

pub(self) mod g {
    pub const GROUP: u32 = 0x2f;
    /// Non nul : créé par un autre objet, tire en place (`spawned`).
    pub const SPAWNED: u32 = 0x42;
    /// Non nul : durée de vie limitée par `LIFETIME`.
    pub const TIMED: u32 = 0x43;
    pub const STRIKE_KIND: u32 = 0x44;
    pub const STRIKE_POWER: u32 = 0x45;
    pub const VULNERABLE: u32 = 0x67;
    pub const AI_WANDER: u32 = 0x6b;
    pub const AI_WAIT: u32 = 0x6e;
    pub const AI_CHARGE: u32 = 0x71;
    pub const AI_SHOT: u32 = 0x78;
    pub const SHOT_KIND: u32 = 0x79;
    pub const FLAGS: u32 = 0x8a;
    /// État à reprendre après un demi-tour (long).
    pub const RESUME_STATE: u32 = 0xa0;
    pub const RESUME_SUB2: u32 = 0xa2;
    pub const TIMER: u32 = 0xa4;
    pub const REPEAT: u32 = 0xa5;
    pub const SAVED_STEP: u32 = 0xa6;
    /// Non nul : le Bow Gnoll est saisi (durée de vie écoulée ou créé pour l'être).
    pub const EXPIRED: u32 = 0xa7;
    pub const LIFETIME: u32 = 0xa8;
    /// Non nul : l'état touché ne dessine ni zone ni ombre.
    pub const NO_ZONE: u32 = 0xac;
}
use g::*;
use crate::objects::pools;
use crate::objects::Courant;

pub(self) const SET_MOVE: u32 = 0x16032c;
pub(self) const SET_SHOT: u32 = 0x160350;
pub(self) const STANCE: u32 = 0x0202_0000;
pub(self) const HELD: u32 = 0x0408_0000;
pub(self) const IDLE: u32 = 0x020a_0000;

pub(self) mod call {
    pub const SWING: u32 = 0x9dbd6;
}

pub fn run(x: &mut Cx) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    if x.u16(f::TARGET) == 0 {
        pick_target(x);
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
    x.anim_start(SET_MOVE, n as u16);
}

/// Démarre l'animation `n` du jeu des tirs.
pub(self) fn shot(x: &mut Cx, n: u32) {
    x.anim_start(SET_SHOT, n as u16);
}

pub(self) fn set_speed(x: &mut Cx, n: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
}

/// 0x5ce22 : direction de marche selon l'orientation (0xc à gauche, 4 à droite).
pub(self) fn face_angle(x: &mut Cx) {
    let angle = if x.u8(f::FLIP) != 0 { 4 } else { 0xc };
    x.set8(Obj::SPEED_INDEX, angle);
}

/// Direction de marche : 8 (regarde à gauche) ou 0x18.
pub(self) fn walk_angle(x: &mut Cx) {
    let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
    x.set8(Obj::SPEED_INDEX, angle);
}

/// Octet tiré au hasard dans la rangée de difficulté de `table` (rangées de 32 octets).
pub(self) fn row_byte(x: &mut Cx, table: u32) -> u8 {
    let row = ((var::RANK.get(x.b) >> 1) << 5) as u32;
    x.c.d[0] = x.c.d[0] & 0xffff_0000 | row;
    let at = table.wrapping_add(row as u16 as i16 as i32 as u32);
    let r = rand_and(x, 0x1f);
    x.data8(at + r)
}

/// Tables de rangées de difficulté : premiers gestes, nombre de tirs, deuxièmes gestes, suite.
pub(self) const FIRST_ACTIONS: u32 = 0x5d47a;
pub(self) const SHOT_REPEATS: u32 = 0x5d07a;
pub(self) const SECOND_ACTIONS: u32 = 0x5d67a;
pub(self) const FOLLOW_TABLE: u32 = 0x5ce72;

/// 0x5cdc6 : tirage au hasard (dix chances sur seize) qui décide d'un tir ; D0 = 0 ou 1.
pub(self) fn shoot_roll(x: &mut Cx) -> bool {
    rand_and(x, 0xf);
    let mask = x.rom16(0x5cde0);
    let hit = (mask >> (x.c.d[0] & 0xf)) & 1 != 0;
    x.c.d[0] = hit as u32;
    hit
}

/// Boîtes de portée (0xb5e0) ; D5 est posé comme dans l'original.
fn range_box(x: &mut Cx, d5: u32, d0: i16, d1: u16, d2: u16) -> bool {
    x.c.d[5] = d5; // relu : D5 relu par la suite (comme pour in_front)
    lib::target_in_box(x, d0, d1, d2)
}

/// 0x5ce14 : joueur tout près (assez pour un tir à bout portant).
pub(self) fn point_blank(x: &mut Cx) -> bool {
    range_box(x, 2, -0x28, 0x28, 0xc)
}

/// 0x5cdf6 : joueur devant, à courte distance.
pub(self) fn in_front(x: &mut Cx) -> bool {
    x.c.d[5] = 0; // relu : D5 relu par la suite (DDSOM_POISON : covb_long3p_02)
    lib::target_in_box(x, -0xd0, 0x60, 0x18)
}

/// 0x5ce06 : joueur près, sur la même ligne.
pub(self) fn on_line(x: &mut Cx) -> bool {
    range_box(x, 2, 0, 0x38, 0x10)
}

/// 0x5ce36 : fin d'un tir, choix du geste de repli (sous-état 8) ; saisi si `+0xa7`.
pub(self) fn follow_up(x: &mut Cx) {
    x.set8(AI_SHOT, 0xff);
    if x.u8(EXPIRED) != 0 {
        set_state_long(x, HELD);
    }
    let b = row_byte(x, FOLLOW_TABLE);
    x.set8(f::SUB, 8);
    x.set8(f::SUB2, b);
    x.set8(f::SUB3, 0);
}

// --- état 0 : création --------------------------------------------------------------------------

/// Faux si le jeu est en pause (rien n'est fait) ; sinon le corps de l'état actif suit dans la même image.
fn init(x: &mut Cx) -> bool {
    if paused(x) {
        return false;
    }
    x.add8(f::STATE, 2);
    x.set8(f::BOX_SET, 0xf);
    lib::initial_hp(x);
    for (off, v) in [
        (0x80, 8),
        (VULNERABLE, 0),
        (0x70, 0),
        (0x68, 2),
        (0x69, 2),
        (0x6a, 1),
        (0x6c, 2),
        (0x6d, 4),
        (0x72, 2),
        (0x73, 2),
        (0x74, 2),
        (0x76, 3),
        (AI_SHOT, 0xff),
        (0x75, 0),
    ] {
        x.set8(off, v);
    }
    lib::set_immunities(x);
    x.set32(FLAGS, 0);
    x.set8(0x7f, 0);
    x.set8(0x7e, 0xc);
    x.set8(0x81, 7);
    if x.u8(TIMED) != 0 {
        x.set16(LIFETIME, 0x80);
    }
    face_angle(x);
    true
}

// --- état 2 : actif -----------------------------------------------------------------------------

fn active(x: &mut Cx) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else if !paused(x) {
        if x.u8(TIMED) != 0 {
            let n = x.u16(LIFETIME).wrapping_sub(1);
            x.set16(LIFETIME, n);
            if (n as i16) < 0 {
                x.set16(LIFETIME, 0xffff);
                x.set8(EXPIRED, 1);
            }
        }
        if x.u8(SPAWNED) != 0 {
            return spawned::run(x);
        }
        x.c.d[0] = x.u8(f::SUB) as u32;
        match x.u8(f::SUB) {
            0 => arrival::run(x),
            2 => stance::run(x),
            4 => shoot::run(x),
            6 => shoot::run_b(x),
            8 => stance::rest(x),
            10 => stance::idle(x),
            12 => stance::turn_around(x),
            _ => {}
        }
    }
    if x.u8(f::SUB) != 0 {
        zone_and_terrain(x);
    }
    x.reinsert();
}

handlers! {
    0x5bda6 "enemy_3" run,
}
