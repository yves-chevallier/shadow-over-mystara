//! Ennemi « Kobold » (ENEMY type 0, 0x54914). Voir `docs/familles/enemy.md`.
//!
//! Le plus gros des ennemis : quatorze variantes d'arrivée (`+3`), quatre profils d'IA (`+0x2f`) et une vingtaine de
//! gestes. Charpente des types 1 et 6 (squelette, chien) avec ses propres tables.
//!
//! | `+5` | Fichier | Rôle (h) |
//! |---|---|---|
//! | 0 | `arrival` | arrivée selon `+6` (course, saut, chute d'en haut, apparition, mort-vivant qui se relève) |
//! | 2 | `stance` | debout : approche et choix d'un geste, demi-tour, attente |
//! | 4 | `attacks` | coups (`+6` = 0 à 36) |
//! | 6, 8 | `moves` | bonds et déplacements |
//! | 10 | `moves` | promenade tant qu'aucun joueur n'est en vie |
//! | état 4 | `hit` | réactions aux coups, recul, chute, saisie par un joueur |
//!
//! Animations 0x15d4ae (mouvements) et 0x15d4ec (coups) ; `+0xa2/+0xa3` aspect courant et précédent.

use super::cx::{handlers, sub, Cx};
use super::enemy_common::*;

mod arrival;
mod attacks;
mod hit;
mod moves;
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
    pub const SPEED_X2: u32 = 0x1c;
    pub const SPEED_Y2: u32 = 0x1e;
    pub const ANGLE: u32 = 0x2d;
    pub const GROUP: u32 = 0x2f;
    pub const KEY: u32 = 0x33;
    pub const STRIKE_KIND: u32 = 0x44;
    pub const STRIKE_POWER: u32 = 0x45;
    pub const VULNERABLE: u32 = 0x67;
    pub const AI_WAIT: u32 = 0x6e;
    pub const AI_CHARGE: u32 = 0x71;
    pub const AI_WANDER: u32 = 0x6b;
    pub const AI_LUNGE: u32 = 0x77;
    pub const AI_SHOT: u32 = 0x78;
    pub const SHOT_KIND: u32 = 0x79;
    /// Minuterie (mot) d'un geste.
    pub const TIMER: u32 = 0xa0;
    pub const LOOK: u32 = 0xa2;
    pub const LOOK_PREV: u32 = 0xa3;
    pub const RESUME_SUB2: u32 = 0xa5;
    pub const RESUME_STATE: u32 = 0xa6;
    pub const TIMER_W: u32 = 0xaa;
    pub const REPEAT: u32 = 0xac;
    pub const GUARD: u32 = 0xad;
}
use g::*;
use crate::objects::pools;

pub(self) const SET_MOVE: u32 = 0x15d4ae;
pub(self) const SET_STRIKE: u32 = 0x15d4ec;
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
    pub const SCREEN_CHECK: u32 = 0x1e92;
    pub const SPARK: u32 = 0xb10c8;
}

pub fn run(x: &mut Cx) {
    if suspended(x) {
        return;
    }
    x.call(rom::TICK_COUNTERS);
    x.c.d[0] = x.u8(f::STATE) as u32;
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        st::DEAD => x.free(&pools::ENEMY),
        _ => {}
    }
}

// --- aides --------------------------------------------------------------------------------------

pub(self) fn mv(x: &mut Cx, n: u32) {
    x.anim_start(SET_MOVE, n);
}

pub(self) fn strike(x: &mut Cx, n: u32) {
    x.anim_start(SET_STRIKE, n);
}

/// L'aspect voulu : `+0xa2` et son double `+0xa3`.
pub(self) fn look(x: &mut Cx, v: u8) {
    x.set8(LOOK, v);
    x.set8(LOOK_PREV, v);
}

pub(self) fn set_speed(x: &mut Cx, n: u32) {
    x.c.d[0] = n;
    x.call(call::SPEED_TABLE);
}

/// Direction de marche : 8 (regarde à gauche) ou 0x18.
pub(self) fn set_angle(x: &mut Cx) {
    let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
    x.set8(ANGLE, angle);
}

/// Étape commune : attend la clé d'animation `+0x33` (0x54fae).
pub(self) fn wait_key(x: &mut Cx) {
    if x.u8(KEY) != 0 {
        x.add8(STEP, 2);
        x.set8(VULNERABLE, 0);
    }
    x.anim_step();
}

/// L'animation finie, le Kobold se met debout, tourné vers sa cible (0x54e9a).
pub(self) fn stand_when_done(x: &mut Cx) {
    if !x.anim_step() {
        return;
    }
    set_state_long(x, STANCE);
    mv(x, 0);
}

/// Un coup de poussière (0xb10c8) toutes les `mask + 1` images (le compteur `A5+0x6e` est masqué en place).
pub(self) fn puff(x: &mut Cx, mask: u16, d1: u16, d2: i16, d3: i16) {
    let v = x.a5_u16(0x6e) & mask;
    x.set_a5_u16(0x6e, v);
    if v == 0 {
        x.c.d[0] = 0;
        set_dw(x, 1, d1);
        set_dw(x, 2, d2 as u16);
        set_dw(x, 3, d3 as u16);
        x.call(call::SPARK);
    }
}

/// 0x57b9c : l'ennemi doit se retourner pour faire face à sa cible (faux sans cible ni aspect mémorisé).
pub(self) fn must_turn(x: &mut Cx) -> bool {
    if x.u8(LOOK_PREV) == 0 || x.u16(f::TARGET) == 0 {
        x.c.d[0] = 0;
        return false;
    }
    let target = x.linked(f::TARGET);
    x.c.a[1] = target.0 | 0xff00_0000;
    let dx = target.u16(x.b, X).wrapping_sub(x.u16(X)) as i16;
    let want = if dx > 0 { 0x20 } else { 0 };
    let turn = want != x.u8(f::FLIP);
    x.c.d[0] = turn as u32;
    turn
}

/// 0x57bee : la cible est à plus de 0x30 de profondeur et à plus de 0xa0 en X.
pub(self) fn target_far_in_line(x: &mut Cx) -> bool {
    if x.u16(f::TARGET) == 0 {
        x.c.d[0] = 0;
        return false;
    }
    let target = x.linked(f::TARGET);
    let dz = target.u16(x.b, DEPTH).wrapping_sub(x.u16(DEPTH)).wrapping_add(0x30);
    if dz <= 0x60 {
        x.c.d[0] = 0;
        return false;
    }
    let dx = target.u16(x.b, X).wrapping_sub(x.u16(X)).wrapping_add(0xa0);
    let far = dx > 0x140;
    x.c.d[0] = far as u32;
    far
}

/// 0x57c34 : force du coup (`+0x45`) d'après le type de coup `+0x44` et le profil `+0x2f` (table 0x57c52).
pub(self) fn set_power(x: &mut Cx) {
    let kind = x.u8(STRIKE_KIND).wrapping_mul(4) as u32;
    let group = x.u8(GROUP) as u32;
    let p = x.data8(0x57c52 + kind + group);
    x.set8(STRIKE_POWER, p);
}

/// 0x57c66 : l'ennemi est dans la moitié droite de l'écran (à 0xc0 de la caméra).
pub(self) fn on_right_half(x: &mut Cx) -> bool {
    let cam = if x.a5_u8(0x36) != 0 { x.a5_u16(0x556) } else { x.a5_u16(0x506) };
    let d1 = cam.wrapping_add(0xc0).wrapping_sub(x.u16(X));
    let right = (d1 as i16) <= 0;
    x.c.d[0] = right as u32;
    right
}

/// 0x56c70 / 0x56c78 : choisit le coup suivant (`+6`) dans la ligne de la difficulté du profil `+0x2f` ;
/// `bias` = 0x14 pour la série lointaine.
pub(self) fn pick_attack(x: &mut Cx, bias: u8) {
    let profile = x.u8(GROUP).wrapping_mul(4) as u32;
    let table = if bias == 0 { 0x56d04 } else { 0x56d14 };
    let list = x.data32(table + profile);
    let row = ((x.a5_u16(0x7f92) >> 1) << 5) as u32;
    let r = rand_and(x, 0x1f);
    let mv = x.data8(list + row + r);
    x.set8(f::SUB2, mv.wrapping_add(bias));
    x.set16(f::STATE, 0x0204);
    x.set8(STEP, 0);
    x.c.d[0] = 0;
    x.call(call::SCREEN_CHECK);
    if !x.c.cond(5) && matches!(x.u8(f::SUB2), 0x16 | 0x18 | 0x1a | 0x1c | 0x1e) {
        x.set8(f::SUB2, 0x14);
    }
}

/// 0x57924 : fin d'un geste : le variant 0 se remet debout ; les autres, si la cible est dans une boîte de portée,
/// enchaînent un coup de la série `0x5799c`.
pub(self) fn after_strike(x: &mut Cx) {
    if x.u8(VARIANT) == 0 || x.u16(f::TARGET) == 0 {
        return set_state_long(x, STANCE);
    }
    let close = target_in_box(x, -0x30, 0x30, 8) || target_in_box(x, -0xd0, 0x70, 8);
    if !close {
        return set_state_long(x, STANCE);
    }
    let row = ((x.a5_u16(0x7f92) >> 1) << 5) as u32;
    let r = rand_and(x, 0x1f);
    let mv = x.data8(0x5799c + row + r);
    x.set8(f::SUB2, mv);
    x.set16(f::STATE, 0x0208);
    x.set8(STEP, 0);
}

// --- état 0 : création --------------------------------------------------------------------------

fn init(x: &mut Cx) {
    if paused(x) {
        return;
    }
    if matches!(x.a5_u8(0x34), 6 | 0xc) {
        x.set8(0x2a, 1);
        x.set8(0x2b, 0x19);
    }
    x.add8(f::STATE, 2);
    x.set8(f::BOX_SET, 0xc);
    x.call(rom::INITIAL_HP);
    for (off, v) in [(0x70, 0), (0x7e, 8), (0x7f, 0), (0x8e, 0), (0x78, 0xff)] {
        x.set8(off, v);
    }
    x.call(rom::IMMUNITIES);
    // profils d'IA : 68 69 6a 6c 6d 72 73 74 75 76 81 80
    let params: [u8; 12] = match x.u8(GROUP) {
        0 => [1, 1, 1, 1, 1, 1, 1, 1, 0x14, 1, 0, 1],
        1 => [2, 2, 1, 2, 2, 2, 2, 2, 0x18, 2, 1, 2],
        2 => [2, 2, 1, 2, 3, 3, 3, 2, 0x1c, 3, 2, 3],
        _ => [2, 2, 1, 3, 4, 4, 4, 3, 0x20, 3, 3, 4],
    };
    for (off, v) in [0x68u32, 0x69, 0x6a, 0x6c, 0x6d, 0x72, 0x73, 0x74, 0x75, 0x76, 0x81, 0x80].into_iter().zip(params) {
        x.set8(off, v);
    }
    if !pick_target(x) {
        return set_state_long(x, IDLE);
    }
    x.call(call::FACE_TARGET);
    x.call(call::TURN_TO_TARGET);
    // état de départ, aspect et animation selon la variante
    let (state, look_v, anim, active) = match x.u8(VARIANT) {
        0 => (STANCE, 0xc, 0xc, false),
        1 => (0x0200_0e00, 0x16, 0x16, false),
        2 => (0x0200_0a00, 0x19, 0x19, false),
        3 => (0x0200_0400, 6, 6, false),
        4 => (0x0200_0600, 0, 0, true),
        5 => (0x0200_0800, 0, 0, true),
        6 => (0x0200_0000, 0x17, 0x17, false),
        7 => (0x0200_0c00, 0x1a, 0x1a, false),
        8 => (0x0200_0200, 0, 0, false),
        9 => (0x0200_1000, 0x17, 0x17, false),
        10 => (0x0200_1200, 0xc, 0xc, false),
        11 => (0x0200_1400, 0xc, 0xc, false),
        12 => (0x0200_0600, 0, 0, true),
        13 => (0x0200_0800, 0, 0, true),
        _ => return,
    };
    set_state_long(x, state);
    if active {
        x.set8(0, 2);
    }
    look(x, look_v);
    x.set8(VULNERABLE, 1);
    mv(x, anim);
}

// --- état 2 : actif -----------------------------------------------------------------------------

fn active(x: &mut Cx) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else {
        if freeze < 0 {
            x.set8(f::FREEZE, 0);
        }
        if !paused(x) {
            x.c.d[0] = x.u8(f::SUB) as u32;
            match x.u8(f::SUB) {
                0 => arrival::run(x),
                2 => stance::run(x),
                4 => attacks::run(x),
                6 => moves::run_b(x),
                8 => moves::run_c(x),
                10 => moves::idle(x),
                _ => {}
            }
        }
    }
    if x.u8(f::SUB) != 0 && x.u8(f::SUB2) != 0xe {
        zone_shadow(x);
    }
    x.reinsert();
}

handlers! {
    0x54914 "enemy_0" 900 run,
}
