//! Boss de type 17 (0x952b4), scènes 13 et 44 (h : « Tel'Arin » dans les noms de debug de l'atlas). Voir
//! `docs/familles/boss.md`.
//!
//! Combattant à trois variantes (`+3`) : 0 est le boss de la scène 13 (arrivée en scène, combat, mort et
//! cinématique de fin), 1 et 2 des apparitions annexes (scène 44). Charpente des ennemis (`+4` état, `+5`
//! groupe, `+6` et `+7` étapes) ; états actifs (`+5`) :
//!
//! | `+5` | Fichier | Rôle |
//! |---|---|---|
//! | 0 | `intro` | cinématiques : arrivée (`+6` = 0 et 2), mort et fin de scène (4), variantes 1 et 2 (6, 8) |
//! | 2 | `fight` | combat : marche vers un joueur (`+6` = 4, 6), charge (8, 10), tout est décidé par `decide` |
//! | 4 | `attacks` | neuf attaques (`+6` = 0 à 16) |
//! | 6, 8, 10 | ici | attente tant qu'aucun joueur n'est en vie |
//! | état 4 | `hit` | réactions aux coups, mort |
//!
//! Animations : 0x18076e (mouvements), 0x1807a2 (coups), 0x1807c0 (recul). `+0xa0` est le jeu courant
//! (0x30 à 0x34 : séquences spéciales, sans ombre ni test de zone) et `+0xa1` l'animation en cours.

use super::boss_common::*;
use super::cx::handlers;

mod attacks;
mod decide;
mod fight;
mod hit;
mod intro;

/// Champs propres à ce type.
pub(self) mod g {
    pub const VARIANT: u32 = 0x3;
    /// Direction de déplacement (index de table de vitesses).
    pub const ANGLE: u32 = 0x2d;
    /// Image-clé de l'animation en cours, posée par les scripts d'animation.
    pub const KEY: u32 = 0x33;
    /// Valeur d'étourdissement (et recopie en `+0x53` : gel après un coup).
    pub const STUN: u32 = 0x56;
    /// Vulnérable (1) ou non (0).
    pub const VULNERABLE: u32 = 0x67;
    /// Pas d'une marche restants (`+0x6b`), images avant le prochain pas (`+0x71`), minuterie d'approche (`+0x6e`).
    pub const WALK_STEPS: u32 = 0x6b;
    pub const APPROACH: u32 = 0x6e;
    pub const WALK_DELAY: u32 = 0x71;
    /// Type du tir lancé par `0xe76c` (indice de table).
    pub const SHOT: u32 = 0x79;
    /// Minuterie de l'étape (mot) et compteur (mot).
    pub const TIMER: u32 = 0xa6;
    pub const COUNT: u32 = 0xa8;
    /// Jeu d'animations courant et animation en cours.
    pub const ANIM_KIND: u32 = 0xa0;
    pub const ANIM_LAST: u32 = 0xa1;
    /// État mémorisé (long) à reprendre après une interruption.
    pub const SAVED_STATE: u32 = 0xa2;
    pub const PHASE: u32 = 0xaa;
    pub const PHASES: u32 = 0xab;
    /// Octet non nul : le boss est invulnérable (`+0x38` remis à zéro à chaque image).
    pub const INVULNERABLE: u32 = 0xad;
    /// Nombre d'objets lâchés encore en jeu.
    pub const SPAWNED: u32 = 0xaf;
    /// Objet lié (mot) : tir ou message.
    pub const CHILD: u32 = 0xb0;
    /// Attaques restantes de chaque sorte (`0xb2`, `0xb3`, `0xb4`).
    pub const BUDGET_A: u32 = 0xb2;
    pub const BUDGET_B: u32 = 0xb3;
    pub const BUDGET_C: u32 = 0xb4;
    /// Ralenti (`0xbb` : 0 normal, 1 ou 2 mode) et ses compteurs.
    pub const SLOW_TICK: u32 = 0xb5;
    pub const STRIKES: u32 = 0xb6;
    pub const STRIKE_LIMIT: u32 = 0xb7;
    pub const SLOW_TIME: u32 = 0xb8;
    pub const SLOW_COUNT: u32 = 0xba;
    pub const SLOW: u32 = 0xbb;
    /// Position mémorisée de la cible.
    pub const MARK_X: u32 = 0xbc;
    pub const MARK_Y: u32 = 0xbe;
}
use g::*;
use crate::objects::pools;

/// Jeux d'animations.
pub(self) const SET_BODY: u32 = 0x18076e;
pub(self) const SET_STRIKE: u32 = 0x1807a2;
pub(self) const SET_HURT: u32 = 0x1807c0;

/// Lance l'animation `n` du jeu des mouvements / des coups par l'IA (`0x103d6`).
pub(self) fn body(x: &mut Cx, n: u32) {
    play(x, SET_BODY, n);
}

pub(self) fn strike(x: &mut Cx, n: u32) {
    play(x, SET_STRIKE, n);
}

/// Routines du moteur appelées par ce boss.
pub(self) mod call {
    pub const MOVE_XY: u32 = 0x1d62;
    pub const MOVE_X: u32 = 0x1d78;
    pub const MOVE_XYZ: u32 = 0x1d90;
    pub const MOVE_XZ: u32 = 0x1da6;
    pub const MOVE_TABLE_XY: u32 = 0x1cb4;
    pub const REINSERT_FOREGROUND: u32 = 0x1ef6c;
    pub const NEW_MESSAGE: u32 = 0xc8312;
    pub const SPAWN_RING: u32 = 0x57c8c;
}

pub fn run(x: &mut Cx) {
    if suspended(x) {
        return;
    }
    loop {
        x.call(rom::TICK_COUNTERS);
        match x.u8(f::STATE) {
            st::INIT => init(x),
            st::ACTIVE => active(x),
            st::HIT => hit::run(x),
            _ => x.free(&pools::BOSS),
        }
        if x.u8(INVULNERABLE) != 0 {
            x.set8(0x38, 0);
        }
        if x.u8(0) == 0 {
            return;
        }
        // ralenti : tant que `+0xba` reste positif, le boss rejoue sa logique dans la même image
        if x.u32(f::STATE) & 0xffff_ff00 != decide::WAIT && x.u8(SLOW) != 0 {
            let t = sub8(x, SLOW_COUNT, 1);
            if t.pl() {
                continue;
            }
            if sub16(x, SLOW_TIME, 1).gt() {
                let tick = x.u8(SLOW);
                x.set8(SLOW_COUNT, tick);
                attacks::slow_tick(x);
            } else {
                x.set8(SLOW, 0);
                decide::pick_strike_limit(x);
            }
        }
        break;
    }
    if cmp8(x, ANIM_KIND, 0x30).eq() {
        x.call(call::REINSERT_FOREGROUND);
    } else {
        x.reinsert();
    }
}

// --- état 0 : création --------------------------------------------------------------------------

fn init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    x.set_a5_u8(0x50, 1);
    x.set_a5_u32(0x38, 0x0459_5900);
    pick_target(x);
    x.set8(PHASES, 4);
    x.set8(f::BOX_SET, 0x31);
    x.call(0x453c);
    x.call(0x1059e);
    x.set_a5_u8(0x5a, 0);
    x.call(0x19e10);
    x.call(0x19db4);
    x.call(0x19eb0);
    decide::pick_strike_limit(x);
    x.set8(0, 2);
    x.set_bit(0x1, 1, true);
    if x.u8(VARIANT) != 0 {
        x.set8(0x93, 1);
        x.set8(0x9a, 0xff);
    }
    for off in [0x76, 0x70, 0x6a, 0x75] {
        x.set8(off, 0);
    }
    for (off, v) in [(0x72, 1), (0x73, 1), (0x6c, 6), (0x6d, 6), (0x74, 7), (0x78, 0xff), (0x8e, 3), (0x68, 0xb), (0x69, 1), (VULNERABLE, 0)] {
        x.set8(off, v);
    }
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(ANIM_KIND, 0);
    x.set8(ANIM_LAST, 0xff);
    x.set8(BUDGET_A, 0xc);
    x.set8(BUDGET_B, 0xc);
    x.set8(BUDGET_C, 3);
    body(x, 0);
    match x.u8(VARIANT) {
        0 => decide::start_intro(x),
        1 => decide::start_variant1(x),
        _ => decide::start_variant2(x),
    }
}

// --- état 2 : actif -----------------------------------------------------------------------------

fn active(x: &mut Cx) {
    let freeze = tst8(x, f::FREEZE);
    if freeze.gt() {
        sub8(x, f::FREEZE, 1);
    } else if x.a5_u8(var::PAUSE) == 0 {
        match x.u8(f::SUB) {
            0 => intro::run(x),
            2 => fight::run(x),
            4 => attacks::run(x),
            _ => wait_for_players(x),
        }
    }
    // les séquences spéciales (jeu 0x30 et au-delà) n'ont ni zone de jeu ni ombre
    if !cmp8(x, ANIM_KIND, 0x30).ge() {
        x.call(IN_ZONE);
        x.call(SHADOW);
    }
}

/// `+5` = 6, 8, 10 (0x96958) : attend que des joueurs reviennent.
fn wait_for_players(x: &mut Cx) {
    if x.a5_u8(var::PLAYERS_ALIVE) as i8 > 0 {
        return decide::chase(x);
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(ANIM_LAST, 0xff);
            body(x, 0);
        }
        2 => {
            step(x);
        }
        _ => {}
    }
}

handlers! {
    0x952b4 "boss_17" run,
    // relevé après une réaction commune (table 0x3e000 de `reactions.rs`)
    0x96ce0 "boss_17_releve" hit::recovered_tail,
}
