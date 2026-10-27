//! Ennemi « WarMachine » (ENEMY type 27, 0x77d12). Voir `docs/familles/enemy.md`.
//!
//! Machine de siège roulante : elle fonce sur le joueur (sous-état 2, `+6` = 8), tire des projectiles (sous-état 4,
//! gestes `+6` = 0 à 12), saute, et ses points de vie la font passer par des paliers (`lib::hp_threshold`). Comme le
//! Gargoyle et l'Ogre elle s'appuie sur la bibliothèque `enemy_lib` (0x103d6-0x105b6). Les décisions d'IA passent par
//! de petites routines qui lisent des tables d'octets puis de mots (adresse de saut relative) et se terminent sur
//! l'un des « talons » d'état (`go::*`, 0x791a0-0x792d6).
//!
//! | `+5` | Fichier | Rôle (h) |
//! |---|---|---|
//! | 0 | `arrival` | arrivée : roule depuis le bord (`+6` = 0), attend un joueur (2 et 4) |
//! | 2 | `stance` | roule vers le joueur, choisit un tir (`decide`), demi-tour, charge |
//! | 4 | `attacks` | tirs et sauts (`+6` = 0 à 12) |
//! | 6, 8, 10 | `attacks` | tourne sur place tant qu'aucun joueur n'est en vie |
//! | état 4 | `hit` | réactions, mort en cinq temps, fuite |

use super::cx::{handlers, Cx};
use super::enemy_common::*;
use super::enemy_lib as lib;

mod arrival;
mod attacks;
mod decide;
mod hit;
mod stance;

pub(self) mod g {
    pub const GROUP: u32 = 0x2f;
    pub const VULNERABLE: u32 = 0x67;
    pub const AI_WANDER: u32 = 0x6b;
    pub const AI_WAIT: u32 = 0x6e;
    pub const AI_CHARGE: u32 = 0x71;
    pub const AI_SHOT: u32 = 0x78;
    pub const SHOT_KIND: u32 = 0x79;
    /// Aspect courant et précédent.
    pub const LOOK: u32 = 0xa0;
    pub const LOOK_PREV: u32 = 0xa1;
    /// État à reprendre après un demi-tour (long).
    pub const RESUME_STATE: u32 = 0xa2;
    pub const TIMER: u32 = 0xa6;
    pub const TIMER2: u32 = 0xa8;
    pub const PHASE: u32 = 0xaa;
    pub const FLAG: u32 = 0xad;
    pub const COUNT: u32 = 0xae;
    /// Non nul : la charge vise la position `GOAL_X` au lieu du bord de l'écran.
    pub const AIMED: u32 = 0xb2;
    pub const GOAL_X: u32 = 0xbc;
}
use g::*;
use crate::objects::pools;
use crate::objects::Courant;

pub(self) const SET_MOVE: u32 = 0x171a52;
pub(self) const SET_WEAPON: u32 = 0x171a7e;
pub(self) const SET_FLY_A: u32 = 0x171b76;
pub(self) const SET_FLY_B: u32 = 0x171c1a;

pub fn run(x: &mut Cx) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    x.c.d[0] = x.u8(f::STATE) as u32;
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        st::DEAD => x.free(&pools::ENEMY),
        _ => {}
    }
    if x.u8(FLAG) != 0 {
        x.set8(0x38, 0);
    }
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

// --- aides --------------------------------------------------------------------------------------

/// Démarre l'animation `n` du jeu des mouvements.
pub(self) fn mv(x: &mut Cx, n: u32) {
    lib::start(x, SET_MOVE, n);
}

/// Démarre l'animation `n` du jeu de l'arme.
pub(self) fn weapon(x: &mut Cx, n: u32) {
    lib::start(x, SET_WEAPON, n);
}

/// Aspect (`+0xa0`) et son double (`+0xa1`).
pub(self) fn look(x: &mut Cx, v: u8) {
    x.c.d[0] = x.c.d[0] & !0xff | v as u32;
    x.set8(LOOK, v);
    x.set8(LOOK_PREV, v);
}

pub(self) fn set_speed(x: &mut Cx, n: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
}

/// Étincelle (0x1172e) numéro `n`.
pub(self) fn spark(x: &mut Cx, n: u32) {
    lib::debris(x, n);
}

/// X de la caméra (écran unique ou deuxième écran).
pub(self) fn camera_x(x: &mut Cx) -> u16 {
    if var::CAMERA_SELECT.get(x.b) != 0 {
        var::CAMERA2_X.get(x.b)
    } else {
        var::CAMERA1_X.get(x.b)
    }
}

/// Orientation : `forward` quand l'orientation est nulle donne 0x18, sinon 8.
pub(self) fn heading(x: &mut Cx) -> u8 {
    if x.u8(f::FLIP) != 0 {
        8
    } else {
        0x18
    }
}

/// Les « talons » d'état : petites routines qui posent l'état suivant (0x791a0-0x792d6) ; les décisions de l'IA y
/// aboutissent par saut.
pub(self) mod go {
    use super::*;

    pub fn arrive(x: &mut Cx, s6: u32) {
        x.set8(FLAG, 1);
        set_state_long(x, 0x0200_0000 | s6 << 8);
    }
    /// 0x791d0.
    pub fn stand(x: &mut Cx) {
        x.set8(VULNERABLE, 1);
        set_state_long(x, 0x0202_0000);
    }
    /// 0x791e0 : demi-tour (état à reprendre mémorisé).
    pub fn turn(x: &mut Cx) {
        x.set8(VULNERABLE, 0);
        let state = x.u32(f::STATE);
        x.set32(RESUME_STATE, state);
        set_state_long(x, 0x0202_0200);
    }
    /// 0x791f4 : tir A (type de tir 0x34, ou 0x36 selon le profil).
    pub fn fire_a(x: &mut Cx) {
        let kind = if x.u8(GROUP) != 0 { 0x36 } else { 0x34 };
        x.set8(SHOT_KIND, kind);
        x.set8(VULNERABLE, 1);
        set_state_long(x, 0x0202_0400);
    }
    /// 0x79216 : tir B (0x35 ou 0x37).
    pub fn fire_b(x: &mut Cx) {
        let kind = if x.u8(GROUP) != 0 { 0x37 } else { 0x35 };
        x.set8(SHOT_KIND, kind);
        x.set8(VULNERABLE, 1);
        set_state_long(x, 0x0202_0600);
    }
    /// 0x79238 (`aimed` = 0) et 0x7924a (`aimed` = 1) : charge.
    pub fn charge(x: &mut Cx, aimed: u8) {
        x.set8(VULNERABLE, 0);
        x.set8(AIMED, aimed);
        set_state_long(x, 0x0202_0800);
    }
    /// 0x7925e à 0x792be : geste du sous-état 4 (`s6` = 0, 2, 4, 6, 8, 0xa, 0xc) ; `flag` : pose `+0xad` (talons
    /// 0x79288 et 0x792aa).
    pub fn attack(x: &mut Cx, s6: u32, flag: bool) {
        if flag {
            x.set8(FLAG, 1);
        }
        x.set8(VULNERABLE, 0);
        set_state_long(x, 0x0204_0000 | s6 << 8);
    }
    /// 0x792cc.
    pub fn idle(x: &mut Cx) {
        set_state_long(x, 0x020a_0000);
    }
    /// 0x792d6.
    pub fn held(x: &mut Cx) {
        set_state_long(x, 0x0408_0000);
    }
}

// --- état 0 : création --------------------------------------------------------------------------

fn init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    x.set8(0xab, 2);
    x.set8(f::BOX_SET, 0x20);
    lib::initial_hp(x);
    lib::spread_index(x);
    x.set8(0, 1);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set_bit(1, 1, true);
    for (off, v) in [
        (0x76, 0),
        (0x70, 0),
        (0x6a, 2),
        (0x75, 0),
        (0x72, 1),
        (0x73, 1),
        (0x6c, 6),
        (0x6d, 1),
        (0x74, 7),
        (AI_SHOT, 0xff),
        (0x8e, 3),
        (0x68, 0xb),
        (0x69, 1),
        (VULNERABLE, 0),
        (LOOK, 0),
        (LOOK_PREV, 0xff),
        (0x7e, 0xc),
        (0x7f, 0),
        (0x80, 0x1e),
        (0x81, 0x19),
    ] {
        x.set8(off, v);
    }
    pick_target(x);
    mv(x, 4);
    // l'état de départ dépend de la variante : 0, 2 ou 4 pour `+6`
    match x.u8(3) {
        0 => go::arrive(x, 0),
        1 => go::arrive(x, 2),
        2 => go::arrive(x, 4),
        _ => {}
    }
}

// --- état 2 : actif -----------------------------------------------------------------------------

fn active(x: &mut Cx) {
    if (x.u8(f::FREEZE) as i8) > 0 {
        let n = x.u8(f::FREEZE).wrapping_sub(1);
        x.set8(f::FREEZE, n);
    } else if !paused(x) {
        x.c.d[0] = x.u8(f::SUB) as u32;
        match x.u8(f::SUB) {
            0 => arrival::run(x),
            2 => stance::run(x),
            4 => attacks::run(x),
            6 | 8 | 10 => attacks::spin(x),
            _ => {}
        }
        if x.u8(f::SUB) == 0 {
            return;
        }
    }
    zone_and_terrain(x);
}

handlers! {
    0x77d12 "enemy_27" run,
    // relevé après une réaction commune (table 0x3e000 de `reactions.rs`)
    0x78c3c "enemy_27_releve" hit::recover,
}
