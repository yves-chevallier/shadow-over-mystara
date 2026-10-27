//! Boss de type 6 (0x8430e), scène 24 : la Manticore (h). Voir `docs/familles/boss.md`.
//!
//! Même gabarit que le boss 17 (`boss_17`) : `+4` état (0 création, 2 actif, 4 touché, 6 libération), `+5`
//! groupe, `+6` et `+7` étapes ; les changements d'état se posent d'un coup (long `+4..+7`).
//!
//! | `+5` | Fichier | Rôle |
//! |---|---|---|
//! | 0 | `intro` | arrivée : bond depuis la droite, rugissement, attente de la caméra |
//! | 2 | `fight` | combat : attente, demi-tour, marche, charge (`+6` = 8), tir en bond (10) |
//! | 4 | `attacks` | cinq attaques (`+6` = 0 à 8) |
//! | 6 à 12 | ici | attente tant qu'aucun joueur n'est en vie |
//! | 14 | ici | reprise après un coup, sans phase |
//! | état 4 | `hit` | réactions aux coups, mort |
//!
//! Animations : 0x177874 (sol), 0x17788c (vol), 0x1778a4 (recul). Les variantes `+3` non nulles sont des
//! parties liées à l'hôte `+0x46` : elles meurent avec lui (`guard`).

use super::boss_common::*;
use super::cx::handlers;
use crate::objects::Obj;

mod attacks;
mod decide;
mod fight;
mod hit;
mod intro;

pub(self) mod g {
    pub const STUN: u32 = 0x56;
    pub const VULNERABLE: u32 = 0x67;
    pub const WALK_STEPS: u32 = 0x6b;
    pub const APPROACH: u32 = 0x6e;
    pub const WALK_DELAY: u32 = 0x71;
    pub const SHOT: u32 = 0x79;
    pub const TIMER: u32 = 0xa6;
    pub const COUNT: u32 = 0xa8;
    pub const ANIM_KIND: u32 = 0xa0;
    pub const ANIM_LAST: u32 = 0xa1;
    pub const SAVED_STATE: u32 = 0xa2;
    pub const PHASE: u32 = 0xaa;
    pub const INVULNERABLE: u32 = 0xad;
    /// Charge : 0 vers la caméra, 1 vers la position mémorisée de la cible.
    pub const CHARGE_AT_TARGET: u32 = 0xb2;
    /// Tir lié (mot) et bascule de l'animation de battement d'ailes.
    pub const CHILD: u32 = 0xb0;
    pub const FLAP: u32 = 0xb3;
    pub const MARK_X: u32 = 0xbc;
}
use g::*;
use crate::objects::pools;
use crate::objects::Courant;

pub(self) const SET_BODY: u32 = 0x177874;
pub(self) const SET_FLY: u32 = 0x17788c;
pub(self) const SET_HURT: u32 = 0x1778a4;

pub(self) fn body(x: &mut Cx, n: u32) {
    lib::start(x, SET_BODY, n);
}

pub(self) fn fly(x: &mut Cx, n: u32) {
    lib::start(x, SET_FLY, n);
}

pub fn run(x: &mut Cx) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    guard(x);
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        _ => x.free(&pools::BOSS),
    }
    if x.u8(INVULNERABLE) != 0 {
        x.set8(0x38, 0);
    }
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

/// 0x859b2 : une partie liée (variante non nulle) tombe avec son hôte (`+0x46`).
fn guard(x: &mut Cx) {
    if x.u8(Obj::VARIANT) == 0 || x.u16(f::STATE) == 0x0406 {
        return;
    }
    let host = x.linked(f::HOST);
    if host.u16(x.b, f::HP) as i16 > 0 {
        return;
    }
    x.set32(f::STATE, 0x0406_0000);
    x.set8(0, 2);
}

fn init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    x.set_a5_u8(0x50, 1);
    x.set_a5_u32(0x38, 0x0459_5900);
    pick_target(x);
    x.set8(0xab, 2);
    x.set8(f::BOX_SET, 0x26);
    lib::initial_hp_boss(x);
    var::SCENE_LEVEL.set(x.b, 0);
    lib::spread_index(x);
    lib::set_immunities(x);
    lib::clear_popup(x);
    lib::empty_popups(x);
    lib::clear_all_popups(x);
    for (off, v) in [(0x76, 0), (0x70, 0), (0x6a, 2), (0x75, 0)] {
        x.set8(off, v);
    }
    x.set_bit(0x1, 1, true);
    x.set8(0x93, 1);
    x.set8(0x9a, 0xff);
    for (off, v) in [(0x72, 1), (0x73, 1), (0x6c, 6), (0x6d, 1), (0x74, 7), (0x78, 0xff), (0x8e, 3), (0x68, 8), (0x69, 1), (VULNERABLE, 0)] {
        x.set8(off, v);
    }
    x.set8(ANIM_KIND, 0);
    x.set8(ANIM_LAST, 0xff);
    for (off, v) in [(0x7e, 0x4a), (0x7f, 0), (0x80, 0x21), (0x81, 0x1b)] {
        x.set8(off, v);
    }
    body(x, 0);
}

fn active(x: &mut Cx) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else if var::PAUSE.get(x.b) == 0 {
        match x.u8(f::SUB) {
            0 => intro::run(x),
            2 => fight::run(x),
            4 => attacks::run(x),
            6 | 8 | 10 | 12 => wait_for_players(x),
            _ => recovered(x),
        }
    }
    lib::keep_in_zone(x);
    lib::collide_terrain(x);
}

/// `+5` = 6 à 12 (0x851d6) : attend que des joueurs reviennent.
fn wait_for_players(x: &mut Cx) {
    if var::PLAYERS_ALIVE.get(x.b) as i8 > 0 {
        return decide::chase(x);
    }
    match x.u8(f::SUB2) {
        0 => {
            x.add8(f::SUB2, 2);
            x.set8(ANIM_LAST, 0xff);
            body(x, 0);
        }
        2 => {
            lib::step(x);
        }
        _ => {}
    }
}

/// `+5` = 14 (0x8520e) : reprise (le seul état de `+6` est la fin de relevé).
fn recovered(x: &mut Cx) {
    if x.u8(f::SUB2) == 0 {
        hit::recovered_tail(x);
    }
}

handlers! {
    0x8430e "boss_6" run,
    // relevé après une réaction commune (table 0x3e000 de `reactions.rs`)
    0x85490 "boss_6_releve" hit::recovered_tail,
}
