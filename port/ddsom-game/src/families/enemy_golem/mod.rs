//! Ennemi « Mud Golem » (ENEMY type 11, 0x67854). Voir `docs/familles/enemy.md`.
//!
//! Un colosse lent dont le code est un amas de gestes : coups de poing (animations 0x15eef4), sauts, bonds
//! latéraux, marche. Quand il se tient debout (`+5` = 0 ou 2), il est piloté par les touches du quatrième joueur
//! (`A5+3`, mode de mise au point) ; l'IA proprement dite part des sous-états 4 (coups), 6 et 8 (repli), choisis par
//! la routine 0x690ce dans des rangées de 32 octets (0x69146).
//!
//! | `+5` | Fichier | Rôle (h) |
//! |---|---|---|
//! | 0, 2 | `control` | marche pilotée par les touches, saut |
//! | 4 | `attacks` | coups (`+6` = 0 à 20) et leurs étapes `+7` |
//! | 6, 8 | `moves` | bonds, esquives, sortie d'écran |
//! | état 4 | `hit` | réactions aux coups, chute, fuite |
//!
//! Animations 0x15eed8 (mouvements) et 0x15eef4 (coups). Beaucoup de gestes partagent trois étapes communes :
//! `next_anim` (0x6935c), `wait` (0x69346, attente de `+0x77` images) et `turn` (0x6899a, demi-tour).

use super::cx::{handlers, sub, Cx};
use super::enemy_common::*;

mod attacks;
mod control;
mod hit;
mod moves;

pub(self) mod g {
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
    /// Temps (mot) de l'animation courante.
    pub const ANIM_TIME: u32 = 0x30;
    /// Drapeau de fin (mot) de l'animation courante.
    pub const ANIM_END: u32 = 0x32;
    pub const KEY: u32 = 0x33;
    pub const STRIKE_KIND: u32 = 0x44;
    pub const STRIKE_POWER: u32 = 0x45;
    pub const AI_WAIT: u32 = 0x77;
    pub const AI_SHOT: u32 = 0x78;
    pub const HIT_FLAG: u32 = 0x8f;
    pub const TIMER: u32 = 0xa2;
    pub const TIMER2: u32 = 0xa4;
    /// Orientation de la cible vue par les boîtes (0, 2) ou numéro de la boîte.
    pub const SIDE: u32 = 0xa6;
}
use g::*;
use crate::objects::pools;

pub(self) const SET_MOVE: u32 = 0x15eed8;
pub(self) const SET_STRIKE: u32 = 0x15eef4;
pub(self) const STANCE: u32 = 0x0202_0000;

pub(self) mod call {
    pub const REMOVE: u32 = 0x27ee;
    pub const SCREEN_CHECK: u32 = 0x1e90;
}

pub fn run(x: &mut Cx) {
    if suspended(x) {
        return;
    }
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

/// Démarre l'animation `n` du jeu des mouvements.
pub(self) fn mv(x: &mut Cx, n: u32) {
    x.anim_start(SET_MOVE, n);
}

/// Démarre l'animation `n` du jeu des coups.
pub(self) fn strike(x: &mut Cx, n: u32) {
    x.anim_start(SET_STRIKE, n);
}

/// Fixe le type de coup et sa force.
pub(self) fn set_strike(x: &mut Cx, kind: u8, power: u8) {
    x.set8(STRIKE_KIND, kind);
    x.set8(STRIKE_POWER, power);
}

/// 0x6935c : étape suivante, animation 0.
pub(self) fn next_anim(x: &mut Cx) {
    x.add8(STEP, 2);
    mv(x, 0);
}

/// 0x69346 : attend `+0x77` images (compte à rebours), puis étape suivante.
pub(self) fn wait(x: &mut Cx) {
    let n = x.u8(AI_WAIT);
    if n != 0 {
        let m = n.wrapping_sub(1);
        x.set8(AI_WAIT, m);
        if (m as i8) > 0 {
            return;
        }
    }
    x.add8(STEP, 2);
}

/// 0x6899a : demi-tour selon `+0xa6` (0 ou 1 : rien, 2 : animation 1 puis 3 : attend la fin et se retourne).
pub(self) fn turn(x: &mut Cx) {
    let side = x.u8(SIDE) as i8;
    if side < 2 {
        return x.add8(STEP, 2);
    }
    if side == 2 {
        x.add8(SIDE, 1);
        return mv(x, 1);
    }
    if (x.u8(ANIM_END) as i8) < 0 {
        x.add8(STEP, 2);
        let flip = x.u8(f::FLIP) ^ 0x20;
        x.set8(f::FLIP, flip);
        return mv(x, 0);
    }
    x.anim_step();
}

/// 0x689e6 : mémorise dans `+0xa6` si la cible est derrière (2) ou devant (0) ; vrai si derrière.
pub(self) fn target_behind(x: &mut Cx) -> bool {
    let mut d1 = 0u32;
    if x.u16(f::TARGET) != 0 {
        let target = load_target(x);
        let mine = x.u16(X);
        let theirs = target.u16(x.b, X);
        if mine != theirs {
            let want = if mine < theirs { 0x20 } else { 0 };
            x.c.d[2] = want;
            if want as u8 != x.u8(f::FLIP) {
                d1 = 2;
            }
        }
    }
    x.c.d[1] = d1;
    x.set8(SIDE, d1 as u8);
    d1 != 0
}

/// 0x68910 : drapeau de coup reçu `+0x8f` : rend vrai quand l'état doit continuer normalement.
pub(self) fn hit_gate(x: &mut Cx) -> bool {
    let v = x.u8(HIT_FLAG);
    x.c.d[6] = v as u32;
    if v == 0 {
        return true;
    }
    x.c.d[6] = v as u32 * 2;
    match v {
        1 => {
            if x.u16(HEIGHT) != 0 {
                return true;
            }
            x.set8(HIT_FLAG, 0);
            x.set8(0x49, 0x58);
            if x.u32(f::STATE) == 0x0404_0200 {
                x.set16(ANIM_END, 0xffff);
                set_state_long(x, 0x0406_0200);
            } else {
                set_state_long(x, 0x0400_0000);
            }
            false
        }
        2 => {
            set_state_long(x, 0x0400_0000);
            x.set8(HIT_FLAG, 0);
            let code = if x.u16(HEIGHT) == 0 { 0xc } else { 0x38 };
            x.set8(0x49, code);
            false
        }
        3 => {
            if x.u16(HEIGHT) != 0 {
                return true;
            }
            x.set8(HIT_FLAG, 0);
            set_state_long(x, 0x0206_0a00);
            false
        }
        _ => false,
    }
}

/// Octet tiré au hasard dans la rangée de difficulté de `table`.
pub(self) fn row_byte(x: &mut Cx, table: u32) -> u8 {
    let row = ((x.a5_u16(0x7f92) >> 1) << 5) as u32;
    x.c.d[0] = x.c.d[0] & 0xffff_0000 | row;
    let at = table.wrapping_add(row as u16 as i16 as i32 as u32);
    let r = rand_and(x, 0x1f);
    x.data8(at + r)
}

/// 0x690ce : choisit le geste suivant (`+5` = 8) d'après la distance de la cible, ou se remet debout.
pub(self) fn choose(x: &mut Cx) {
    x.set8(AI_SHOT, 0xff);
    if x.u16(f::TARGET) == 0 {
        return set_state_long(x, STANCE);
    }
    load_target(x);
    set_dw(x, 0, -8i16 as u16);
    set_dw(x, 1, 0x68);
    set_dw(x, 2, 8);
    x.call(0xb5e0);
    if x.c.cond(7) {
        set_dw(x, 0, -0xa0i16 as u16);
        set_dw(x, 1, 0x30);
        set_dw(x, 2, 8);
        x.call(0xb5e0);
        if x.c.cond(7) {
            return set_state_long(x, STANCE);
        }
    }
    let d0 = x.c.d[0] as u8;
    x.set8(SIDE, d0);
    let b = row_byte(x, 0x69146);
    x.set8(f::SUB2, b);
    x.set16(f::STATE, 0x0208);
    x.set8(STEP, 0);
}

/// 0x67ea4 : relance l'animation 2 en reprenant le temps et le drapeau de fin de l'animation précédente
/// (au plus huit images d'avance), marque le coup et mémorise la position.
pub(self) fn restart_walk(x: &mut Cx) {
    let time = x.u16(ANIM_TIME);
    let end = x.u16(ANIM_END);
    mv(x, 2);
    let mut keep = time;
    let mut found = false;
    for _ in 0..8 {
        if x.u16(ANIM_END) == end {
            found = true;
            break;
        }
        x.set16(ANIM_END, 1);
        x.anim_step();
    }
    if !found {
        keep = x.u16(ANIM_TIME);
    }
    x.set16(ANIM_TIME, keep);
    x.set32(0x4c, 0x1c7336);
    x.set8(ANGLE, 8);
    if x.u8(f::FLIP) != 0 {
        x.add8(ANGLE, 0x10);
    }
    let at = x.u16(X);
    x.set16(TIMER2, at);
}

// --- état 0 : création --------------------------------------------------------------------------

fn init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    x.call(rom::INITIAL_HP);
    let (bx, by) = if x.a5_u8(0x36) != 0 { (0x556, 0x55a) } else { (0x506, 0x50a) };
    let cam_x = x.a5_u16(bx);
    let cam_y = x.a5_u16(by);
    x.set16(X, cam_x.wrapping_add(0xc0));
    x.set32(HEIGHT, 0);
    x.set16(DEPTH, cam_y.wrapping_add(0x50));
    x.set8(f::FLIP, 0);
    x.c.d[0] = 0;
    set_state_long(x, STANCE);
    mv(x, 0);
    x.reinsert();
}

// --- état 2 : actif -----------------------------------------------------------------------------

fn active(x: &mut Cx) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else if !paused(x) {
        x.c.d[0] = x.u8(f::SUB) as u32;
        match x.u8(f::SUB) {
            0 | 2 => control::run(x),
            4 => attacks::run(x),
            6 => moves::run_c(x),
            8 => moves::run_d(x),
            _ => {}
        }
    }
    zone_shadow(x);
    x.reinsert();
}

handlers! {
    0x67854 "enemy_11" run,
}
