//! Ennemi « Gargoyle » (ENEMY type 8, 0x63fb0). Voir `docs/familles/enemy.md`.
//!
//! Créature volante qui se pose pour attaquer (variantes `+3` = 0 à 2 : arrivée au ras du sol, en vol ou au sol).
//! Elle s'appuie sur la bibliothèque commune `enemy_lib` (jeux d'animation avec type de coup `+0x44`).
//!
//! | `+5` | Fichier | Rôle (h) |
//! |---|---|---|
//! | 0 | `arrival` | arrivée en vol (`+6` = 0 variante 0, 2 variante 1) |
//! | 2 | `stance` | déplacements (`+6` = 0 à 10) : poursuite, demi-tour, plongeons, vols en cercle |
//! | 4 | `attacks` | coups (`+6` = 0 à 12) |
//! | 6, 8, 10 | `attacks` | repos, tir de fin de combat |
//! | 12, 14 | `attacks` | saisie d'un joueur par la créature |
//! | état 4 | `hit` | réactions aux coups (table `0x6518a` par code `+0x49`), chute, mort |
//!
//! `+0x50` est la cible (joueur), `+0x54` l'éventuelle victime saisie. Animations 0x162ebc (vol), 0x162eee (coups),
//! 0x163014 et 0x163098 (vol contre ou avec l'orientation).

use crate::families::cx::Banc;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use super::enemy_lib as lib;
use crate::objects::Obj;

mod arrival;
mod attacks;
mod hit;
mod stance;

pub(self) mod g {
    pub const ANIM_FLAGS: u32 = 0x32;
    pub const VICTIM: u32 = 0x54;
    pub const HIT_FLAGS: u32 = 0x57;
    pub const VULNERABLE: u32 = 0x67;
    pub const STRIKE_KIND_NEXT: u32 = 0x79;
    pub const WANDER: u32 = 0x6b;
    pub const TURNS: u32 = 0x6e;
    pub const CHARGE: u32 = 0x71;
    pub const NO_GRAB: u32 = 0x86;
    pub const LOOK: u32 = 0xa0;
    pub const LOOK_PREV: u32 = 0xa1;
    pub const SAVED_STATE: u32 = 0xa2;
    pub const TIMER: u32 = 0xa6;
    pub const TIMER2: u32 = 0xa8;
    pub const PHASE: u32 = 0xaa;
    pub const INVULNERABLE: u32 = 0xad;
    pub const GRABBED_ONCE: u32 = 0xaf;
    pub const SIDE_TURN: u32 = 0xb2;
    pub const COOLDOWN: u32 = 0xb4;
    pub const HIT_CODE_SAVED: u32 = 0xb7;
    pub const STATE_SAVED: u32 = 0xb8;
    pub const GOAL_X: u32 = 0xbc;
    pub const GOAL_DEPTH: u32 = 0xbe;
}
use g::*;
use crate::objects::pools;
use crate::objects::Courant;

pub(self) const SET_FLY: u32 = 0x162ebc;
pub(self) const SET_STRIKE: u32 = 0x162eee;
/// Jeux de vol selon que la vitesse suit ou non l'orientation (`lib::moving_forward`).
pub(self) const SET_FLY_AGAINST: u32 = 0x163014;
pub(self) const SET_FLY_ALONG: u32 = 0x163098;

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
    if x.u8(INVULNERABLE) != 0 {
        x.set8(0x38, 0);
    }
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

// --- aides --------------------------------------------------------------------------------------

pub(self) fn play<R: Banc>(x: &mut Cx<'_, R>, set: u32, n: u32) {
    lib::start(x, set, n);
}

pub(self) fn fly<R: Banc>(x: &mut Cx<'_, R>, n: u32) {
    play(x, SET_FLY, n);
}

pub(self) fn strike(x: &mut Cx, n: u32) {
    play(x, SET_STRIKE, n);
}

pub(self) fn look<R: Banc>(x: &mut Cx<'_, R>, v: u8) {
    x.set8(LOOK, v);
    x.set8(LOOK_PREV, v);
}

/// Table de vitesses `n` (0x1b64) pour l'angle `+0x2d`.
pub(self) fn set_speed<R: Banc>(x: &mut Cx<'_, R>, n: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
}

/// Éclat de décor (0x1172e) au point d'impact `d5`.
pub(self) fn spark(x: &mut Cx, d5: u32) {
    lib::debris(x, d5);
}

pub(self) fn camera_x<R: Banc>(x: &mut Cx<'_, R>) -> u16 {
    if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA2_X.get(x.b) } else { var::CAMERA1_X.get(x.b) }
}

/// Décompte d'une minuterie mot `+off` : vrai tant qu'elle reste positive.
pub(self) fn count_down<R: Banc>(x: &mut Cx<'_, R>, off: u32) -> bool {
    let n = x.u16(off).wrapping_sub(1);
    x.set16(off, n);
    (n as i16) > 0
}

/// 0x65a9e : se tourne vers la cible.
pub(self) fn face_target(x: &mut Cx) {
    let target = x.linked(f::TARGET);
    let d1 = target.u16(x.b, Obj::X);
    let flip = if (d1 as i16) < (x.u16(Obj::X) as i16) { 0 } else { 0x20 };
    x.c.d[0] = flip;
    x.set8(f::FLIP, flip as u8);
}

/// 0x65a30 : ondulation verticale en vol selon la clé d'animation (table de longs `0x65a5a`).
pub(self) fn bob(x: &mut Cx) {
    let idx = (x.u8(f::KEY_FRAME) as u32) << 2;
    let mut d = x.rom32(0x65a5a + idx) as i32;
    if (x.u16(TIMER2) as i16) <= 0x24 {
        d = d.wrapping_neg();
    }
    x.add32(Obj::Z, d as u32);
    if !count_down(x, TIMER2) {
        x.set16(TIMER2, 0x28);
    }
}

/// 0x65a72 : montée progressive (table de longs `0x65a86`).
pub(self) fn rise<R: Banc>(x: &mut Cx<'_, R>) {
    let idx = (x.u8(f::KEY_FRAME) as u32) << 2;
    let d = rom32(x, 0x65a86 + idx);
    x.add32(Obj::Z, d);
}

// --- transitions d'état : le geste suivant ----------------------------------------------------------

pub(self) mod go {
    use super::*;

    /// Sous-état 2 (déplacements) : `+6` = 2, saisie de l'état courant pour y revenir.
    pub fn turn_around<R: Banc>(x: &mut Cx<'_, R>) {
        x.set8(VULNERABLE, 0);
        let s = x.u32(f::STATE);
        x.set32(SAVED_STATE, s);
        set_state_long(x, 0x0202_0200);
    }
    pub fn wander<R: Banc>(x: &mut Cx<'_, R>) {
        x.set8(VULNERABLE, 0);
        x.set8(STRIKE_KIND_NEXT, 0x3b);
        set_state_long(x, 0x0202_0400);
    }
    pub fn wander_far<R: Banc>(x: &mut Cx<'_, R>) {
        x.set8(VULNERABLE, 0);
        x.set8(STRIKE_KIND_NEXT, 0x3c);
        set_state_long(x, 0x0202_0600);
    }
    pub fn dive<R: Banc>(x: &mut Cx<'_, R>) {
        x.set8(VULNERABLE, 0);
        x.set8(SIDE_TURN, 0);
        set_state_long(x, 0x0202_0800);
    }
    pub fn circle<R: Banc>(x: &mut Cx<'_, R>) {
        x.set8(INVULNERABLE, 1);
        x.set8(VULNERABLE, 0);
        set_state_long(x, 0x0202_0a00);
    }
    /// Sous-état 4 (coups), `+6` = 0 ; `grab` : la créature a déjà saisi sa victime.
    pub fn strike_0<R: Banc>(x: &mut Cx<'_, R>, grab: bool) {
        if grab {
            x.set8(HIT_FLAGS, 0);
        }
        x.set8(GRABBED_ONCE, grab as u8);
        x.set8(VULNERABLE, 0);
        set_state_long(x, 0x0204_0000);
    }
    pub fn strike_2<R: Banc>(x: &mut Cx<'_, R>, grab: bool) {
        if grab {
            x.set8(HIT_FLAGS, 0);
        }
        x.set8(GRABBED_ONCE, grab as u8);
        x.set8(VULNERABLE, 1);
        set_state_long(x, 0x0204_0200);
    }
    pub fn strike_4<R: Banc>(x: &mut Cx<'_, R>, grab: bool) {
        x.set8(GRABBED_ONCE, grab as u8);
        x.set8(VULNERABLE, 0);
        set_state_long(x, 0x0204_0400);
    }
    pub fn strike_6<R: Banc>(x: &mut Cx<'_, R>) {
        x.set8(VULNERABLE, 0);
        set_state_long(x, 0x0204_0600);
    }
    pub fn strike_8<R: Banc>(x: &mut Cx<'_, R>) {
        x.set8(INVULNERABLE, 1);
        x.set8(VULNERABLE, 0);
        set_state_long(x, 0x0204_0800);
    }
    pub fn strike_10<R: Banc>(x: &mut Cx<'_, R>) {
        x.set8(VULNERABLE, 0);
        set_state_long(x, 0x0204_0a00);
    }
    pub fn strike_12<R: Banc>(x: &mut Cx<'_, R>) {
        x.set8(VULNERABLE, 0);
        set_state_long(x, 0x0204_0c00);
    }
    /// Plus de joueur en vie : la créature se retire (sous-état 10).
    pub fn retire<R: Banc>(x: &mut Cx<'_, R>) {
        set_state_long(x, 0x020a_0000);
    }
    /// Saisie par un joueur : état touché 8.
    pub fn held<R: Banc>(x: &mut Cx<'_, R>) {
        set_state_long(x, 0x0408_0000);
    }
}

// --- état 0 : création --------------------------------------------------------------------------

fn init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    x.set8(0xab, 2);
    x.set8(f::BOX_SET, 0x14);
    lib::initial_hp(x);
    lib::spread_index(x);
    x.set8(0, 1);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set_bit(1, 1, true);
    for (off, v) in [
        (0x76, 0),
        (0x70, 0),
        (0x6a, 0),
        (0x75, 0),
        (0x72, 1),
        (0x73, 1),
        (0x6c, 6),
        (0x6d, 1),
        (0x74, 7),
        (0x78, 0xff),
        (0x8e, 3),
        (0x68, 8),
        (0x69, 1),
        (VULNERABLE, 0),
        (LOOK, 0),
        (LOOK_PREV, 0xff),
        (0x7e, 0x11),
        (0x7f, 0),
        (0x80, 0x11),
        (0x81, 0x10),
    ] {
        x.set8(off, v);
    }
    pick_target(x);
    fly(x, 0);
    match x.u8(Obj::VARIANT) {
        0 => {
            x.set8(INVULNERABLE, 1);
            set_state_long(x, 0x0200_0000);
        }
        1 => {
            x.set8(INVULNERABLE, 1);
            set_state_long(x, 0x0200_0200);
        }
        2 => set_state_long(x, 0x0200_0400),
        _ => {}
    }
}

// --- état 2 : actif -----------------------------------------------------------------------------

fn active(x: &mut Cx) {
    if x.u8(f::FREEZE) as i8 > 0 {
        x.add8(f::FREEZE, 0xff);
    } else if !paused(x) {
        x.c.d[0] = x.u8(f::SUB) as u32;
        match x.u8(f::SUB) {
            0 => arrival::run(x),
            2 => stance::run(x),
            4 => attacks::run(x),
            6 | 8 | 10 => attacks::rest(x),
            12 => attacks::grab(x),
            14 => attacks::done(x),
            _ => {}
        }
    }
    if x.u8(f::SUB) != 0 {
        zone_and_terrain(x);
    }
    let s = x.u32(f::STATE);
    x.set32(STATE_SAVED, s);
    cooldown(x);
}

/// 0x65ab6 : décompte de la période de repos (`+0xb4`) ; à zéro, le drapeau `+0x58` retombe.
pub(self) fn cooldown<R: Banc>(x: &mut Cx<'_, R>) {
    if (x.u16(COOLDOWN) as i16) <= 0 {
        return;
    }
    if !count_down(x, COOLDOWN) {
        x.set8(0x58, 0);
    }
}

handlers! {
    0x63fb0 "enemy_8" run,
    // relevé après une réaction commune (table 0x3e000 de `reactions.rs`)
    0x653b2 "enemy_8_releve" attacks::regroup,
}
