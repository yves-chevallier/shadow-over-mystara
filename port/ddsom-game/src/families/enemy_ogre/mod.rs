//! Ennemi « Ogre » (ENEMY type 18, 0x703dc). Voir `docs/familles/enemy.md`.
//!
//! Deux sortes d'objets partagent ce gestionnaire : l'ogre (variantes `+3` = 0 à 2, ou 0xff dans les scènes 65 et 66)
//! et ses « bras » lancés (type 0x2a, hors de ce gestionnaire). Comme le Gargoyle il s'appuie sur la bibliothèque
//! `enemy_lib` ; la variante négative n'est pas saisissable (`tst.b (3,A0) ; bmi` saute le test de saisie).
//!
//! | `+5` | Fichier | Rôle (h) |
//! |---|---|---|
//! | 0 | `arrival` | arrivée (`+6` = 0 chute d'entrée, 2 apparition au sol, 4 attente d'un joueur, 6 apparition cachée) |
//! | 2 | `stance` | déplacements (`+6` = 0 à 10) |
//! | 4 | `attacks` | coups (`+6` = 0 à 8) |
//! | 6, 8, 10 | `attacks` | repos |
//! | état 4 | `hit` | réactions (table `0x715d4`), recul, chute, mort, fuite |
//!
//! Animations 0x177874 et 0x17788c (mouvements et coups), 0x1778a4 (mort).

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
    pub const ARM_OF: u32 = 0x46;
    pub const VULNERABLE: u32 = 0x67;
    pub const WANDER: u32 = 0x6b;
    pub const TURNS: u32 = 0x6e;
    pub const CHARGE: u32 = 0x71;
    pub const STRIKE_KIND_NEXT: u32 = 0x79;
    pub const NO_GRAB: u32 = 0x86;
    pub const LOOK: u32 = 0xa0;
    pub const LOOK_PREV: u32 = 0xa1;
    pub const SAVED_STATE: u32 = 0xa2;
    pub const TIMER: u32 = 0xa6;
    pub const TIMER2: u32 = 0xa8;
    pub const PHASE: u32 = 0xaa;
    pub const INVULNERABLE: u32 = 0xad;
    pub const ARM: u32 = 0xb0;
    pub const SIDE_TURN: u32 = 0xb2;
    pub const FLASH: u32 = 0xb3;
    pub const GOAL_X: u32 = 0xbc;
}
use g::*;
use crate::objects::pools;
use crate::objects::Courant;

pub(self) const SET_A: u32 = 0x177874;
pub(self) const SET_B: u32 = 0x17788c;
pub(self) const SET_DEATH: u32 = 0x1778a4;
pub(self) const SET_FLY_AGAINST: u32 = 0x177992;
pub(self) const SET_FLY_ALONG: u32 = 0x177a36;

pub fn run(x: &mut Cx) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    arm_dies_with_ogre(x);
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

/// 0x71dde : un objet de variante négative (le « bras ») meurt avec l'objet `+0x46` qu'il accompagne.
fn arm_dies_with_ogre<R: Banc>(x: &mut Cx<'_, R>) {
    if (x.u8(Obj::VARIANT) as i8) >= 0 || x.u16(f::STATE) == 0x0406 {
        return;
    }
    let owner = x.linked(ARM_OF);
    if (owner.u16(x.b, f::HP) as i16) > 0 {
        return;
    }
    set_state_long(x, 0x0406_0000);
    x.set8(0, 2);
}

// --- aides --------------------------------------------------------------------------------------

pub(self) fn play<R: Banc>(x: &mut Cx<'_, R>, set: u32, n: u32) {
    lib::start(x, set, n);
}

pub(self) fn mv<R: Banc>(x: &mut Cx<'_, R>, n: u32) {
    play(x, SET_A, n);
}

pub(self) fn act<R: Banc>(x: &mut Cx<'_, R>, n: u32) {
    play(x, SET_B, n);
}

pub(self) fn look<R: Banc>(x: &mut Cx<'_, R>, v: u8) {
    x.set8(LOOK, v);
    x.set8(LOOK_PREV, v);
}

pub(self) fn set_speed<R: Banc>(x: &mut Cx<'_, R>, n: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
}

pub(self) fn spark(x: &mut Cx, d5: u32) {
    lib::debris(x, d5);
}

pub(self) fn camera_x<R: Banc>(x: &mut Cx<'_, R>) -> u16 {
    if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA2_X.get(x.b) } else { var::CAMERA1_X.get(x.b) }
}

pub(self) fn count_down<R: Banc>(x: &mut Cx<'_, R>, off: u32) -> bool {
    let n = x.u16(off).wrapping_sub(1);
    x.set16(off, n);
    (n as i16) > 0
}

/// Orientation vers la droite (0x18) ou la gauche (8) selon `flip`.
pub(self) fn angle_by_flip<R: Banc>(x: &mut Cx<'_, R>) {
    let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
    x.set8(Obj::SPEED_INDEX, angle);
}

/// L'ogre peut être saisi par un joueur (pas les variantes négatives) : abandonne alors le geste.
pub(self) fn held_by_player<R: Banc>(x: &mut Cx<'_, R>) -> bool {
    if (x.u8(Obj::VARIANT) as i8) < 0 || !grabbed(x) {
        return false;
    }
    go::held(x);
    true
}

/// Plus de joueur en vie : retraite ; vrai si l'état a changé.
pub(self) fn nobody_left<R: Banc>(x: &mut Cx<'_, R>) -> bool {
    if lib::players_alive(x) {
        return false;
    }
    go::retire(x);
    true
}

/// 0x71d98 : ondulation verticale en vol selon la clé d'animation (table de longs `0x71dc6`).
pub(self) fn bob(x: &mut Cx) {
    let idx = ((x.u8(f::KEY_FRAME) & 0x7f) as u32) << 2;
    let mut d = x.rom32(0x71dc6 + idx) as i32;
    if (x.u16(TIMER2) as i16) <= 0x24 {
        d = d.wrapping_neg();
    }
    x.add32(Obj::Z, d as u32);
    if !count_down(x, TIMER2) {
        x.set16(TIMER2, 0x48);
    }
}

/// Étincelle de la clé d'animation : bit 7 de `+0x33` → effet 0x11a36, alterné par `+0xb3`.
pub(self) fn flash_key(x: &mut Cx) {
    if (x.u8(f::KEY_FRAME) as i8) < 0 {
        let k = x.u8(f::KEY_FRAME) & 0x7f;
        x.set8(f::KEY_FRAME, k);
        let f = x.u8(FLASH) ^ 1;
        x.set8(FLASH, f);
        lib::ground_debris(x, f);
    }
}

// --- transitions d'état : le geste suivant ----------------------------------------------------------

pub(self) mod go {
    use super::*;

    /// 0x71cac : debout.
    pub fn stand(x: &mut Cx) {
        x.set8(VULNERABLE, 1);
        set_state_long(x, 0x0202_0000);
    }
    /// 0x71cbc : demi-tour, avec mémorisation de l'état courant.
    pub fn turn_around<R: Banc>(x: &mut Cx<'_, R>) {
        x.set8(VULNERABLE, 0);
        let s = x.u32(f::STATE);
        x.set32(SAVED_STATE, s);
        set_state_long(x, 0x0202_0200);
    }
    pub fn wander<R: Banc>(x: &mut Cx<'_, R>) {
        x.set8(VULNERABLE, 1);
        x.set8(STRIKE_KIND_NEXT, 0x2c);
        set_state_long(x, 0x0202_0400);
    }
    pub fn wander_far<R: Banc>(x: &mut Cx<'_, R>) {
        x.set8(VULNERABLE, 1);
        x.set8(STRIKE_KIND_NEXT, 0x2d);
        set_state_long(x, 0x0202_0600);
    }
    pub fn dive<R: Banc>(x: &mut Cx<'_, R>) {
        x.set8(VULNERABLE, 0);
        x.set8(SIDE_TURN, 0);
        set_state_long(x, 0x0202_0800);
    }
    pub fn dive_side<R: Banc>(x: &mut Cx<'_, R>) {
        x.set8(VULNERABLE, 0);
        x.set8(SIDE_TURN, 1);
        set_state_long(x, 0x0202_0800);
    }
    pub fn leap<R: Banc>(x: &mut Cx<'_, R>) {
        x.set8(VULNERABLE, 0);
        set_state_long(x, 0x0202_0a00);
    }
    pub fn strike_0<R: Banc>(x: &mut Cx<'_, R>) {
        x.set8(VULNERABLE, 1);
        set_state_long(x, 0x0204_0000);
    }
    pub fn strike_2<R: Banc>(x: &mut Cx<'_, R>) {
        x.set8(VULNERABLE, 0);
        set_state_long(x, 0x0204_0200);
    }
    pub fn strike_4<R: Banc>(x: &mut Cx<'_, R>) {
        x.set8(VULNERABLE, 0);
        set_state_long(x, 0x0204_0400);
    }
    pub fn strike_6<R: Banc>(x: &mut Cx<'_, R>) {
        x.set8(INVULNERABLE, 1);
        x.set8(VULNERABLE, 0);
        set_state_long(x, 0x0204_0600);
    }
    pub fn strike_8<R: Banc>(x: &mut Cx<'_, R>) {
        x.set8(INVULNERABLE, 1);
        x.set8(VULNERABLE, 0);
        set_state_long(x, 0x0204_0800);
    }
    pub fn retire<R: Banc>(x: &mut Cx<'_, R>) {
        set_state_long(x, 0x020a_0000);
    }
    pub fn held<R: Banc>(x: &mut Cx<'_, R>) {
        set_state_long(x, 0x0408_0000);
    }
}

// --- état 0 : création --------------------------------------------------------------------------

/// 0x71e04 : PV initiaux ; la variante négative les calcule comme un type 6 (0x453c) puis reprend le type 0x12.
fn initial_hp<R: Banc>(x: &mut Cx<'_, R>) {
    if (x.u8(Obj::VARIANT) as i8) >= 0 {
        return lib::initial_hp(x);
    }
    x.set8(2, 6);
    lib::initial_hp_boss(x);
    x.set8(2, 0x12);
}

fn init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    x.set8(0xab, 2);
    x.set8(f::BOX_SET, 0x26);
    initial_hp(x);
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
        (0x78, 0xff),
        (0x8e, 3),
        (0x68, 8),
        (0x69, 1),
        (VULNERABLE, 0),
        (LOOK, 0),
        (LOOK_PREV, 0xff),
        (0x7e, 0x17),
        (0x7f, 0),
        (0x80, 0x21),
        (0x81, 0x1b),
    ] {
        x.set8(off, v);
    }
    pick_target(x);
    mv(x, 0);
    x.set8(INVULNERABLE, 1);
    let state = match x.u8(Obj::VARIANT) as i8 {
        v if v < 0 => 0x0200_0000,
        0 => 0x0200_0200,
        1 => 0x0200_0400,
        2 => 0x0200_0600,
        _ => return,
    };
    set_state_long(x, state);
}

// --- état 2 : actif -----------------------------------------------------------------------------

fn active(x: &mut Cx) {
    if x.u8(f::FREEZE) as i8 > 0 {
        x.add8(f::FREEZE, 0xff);
        zone_and_terrain(x);
        return;
    }
    if !paused(x) {
        x.c.d[0] = x.u8(f::SUB) as u32;
        match x.u8(f::SUB) {
            0 => arrival::run(x),
            2 => stance::run(x),
            4 => attacks::run(x),
            6 | 8 | 10 => attacks::rest(x),
            _ => {}
        }
        if x.u8(f::SUB) == 0 {
            return;
        }
    }
    zone_and_terrain(x);
}

handlers! {
    0x703dc "enemy_18" run,
    // relevé après une réaction commune (table 0x3e000 de `reactions.rs`)
    0x717ea "enemy_18_releve" hit::regroup,
}
