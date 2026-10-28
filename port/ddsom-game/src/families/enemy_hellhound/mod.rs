//! Ennemi « Hellhound » (ENEMY type 6, 0x61364). Voir `docs/familles/enemy.md`.
//!
//! Chien de l'enfer à huit variantes d'arrivée (`+3`) et deux jeux de paramètres d'IA (`+0x2f`). Charpente de
//! `active_loop` ; animations 0x162126 (mouvements) et 0x162148 (coups).
//!
//! | `+5` | Fichier | Rôle (h) |
//! |---|---|---|
//! | 0 | `arrival` | arrivée selon `+6` (course, saut, chute du haut, apparition) |
//! | 2 | `stance` | debout : approche, demi-tour, attente, crachat, saut (`+6` = 0 à 10) |
//! | 4 | `attacks` | coups et sauts d'attaque (`+6` = 0 à 16) |
//! | 6, 8 | `attacks` | bonds, gestes spéciaux (`+6` = 0 à 12) |
//! | 10 | `attacks` | repos tant qu'aucun joueur n'est en vie |
//! | 12 | `attacks` | saisie d'un joueur |
//! | état 4 | `hit` | réactions aux coups, recul, chute, saisie par un joueur |

use crate::families::cx::Banc;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

mod arrival;
mod attacks;
mod hit;
mod stance;

pub(self) mod g {
    pub const GROUP: u32 = 0x2f;
    pub const STRIKE_KIND: u32 = 0x44;
    pub const STRIKE_POWER: u32 = 0x45;
    pub const VULNERABLE: u32 = 0x67;
    pub const AI_WAIT: u32 = 0x6e;
    pub const AI_CHARGE: u32 = 0x71;
    pub const AI_WANDER: u32 = 0x6b;
    pub const AI_LUNGE: u32 = 0x77;
    pub const SHOT_KIND: u32 = 0x79;
    pub const TIMER: u32 = 0xa0;
    pub const LOOK: u32 = 0xa1;
    pub const LOOK_PREV: u32 = 0xa2;
    /// Sous-état à reprendre après un demi-tour ou un geste.
    pub const RESUME_SUB2: u32 = 0xa4;
    pub const RESUME_STATE: u32 = 0xa6;
    pub const TIMER_W: u32 = 0xaa;
    pub const SCRIPT: u32 = 0xac;
    pub const GUARD: u32 = 0xad;
    pub const NO_ZONE: u32 = 0xaf;
    pub const BITES: u32 = 0xb0;
}
use g::*;
use crate::objects::pools;
use crate::objects::Courant;

pub(self) const SET_MOVE: u32 = 0x162126;
pub(self) const SET_STRIKE: u32 = 0x162148;

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
}

// --- aides --------------------------------------------------------------------------------------

pub(self) fn mv<R: Banc>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_MOVE, n as u16);
}

pub(self) fn strike(x: &mut Cx, n: u32) {
    x.anim_start(SET_STRIKE, n as u16);
}

/// L'aspect voulu : `+0xa1` et son double `+0xa2`.
pub(self) fn look<R: Banc>(x: &mut Cx<'_, R>, v: u8) {
    x.set8(LOOK, v);
    x.set8(LOOK_PREV, v);
}

pub(self) fn set_speed<R: Banc>(x: &mut Cx<'_, R>, n: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
}

/// Direction de marche : 8 (regarde à gauche) ou 0x18.
pub(self) fn set_angle<R: Banc>(x: &mut Cx<'_, R>) {
    let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
    x.set8(Obj::SPEED_INDEX, angle);
}

/// Étape commune des gestes : attend la clé d'animation `+0x33` puis passe à l'étape suivante (0x6244a).
pub(self) fn wait_key<R: Banc>(x: &mut Cx<'_, R>) {
    if x.u8(f::KEY_FRAME) != 0 {
        x.add8(f::SUB3, 2);
        x.set8(VULNERABLE, 0);
    }
    x.anim_step();
}

/// Retombée d'un saut (0x62460) : le geste se termine par une poussière et l'animation 0xc.
pub(self) fn land(x: &mut Cx) {
    if x.move_xz().gt() {
        x.anim_step();
        return;
    }
    land_dust(x);
}

/// 0x62478 : poussière, retour au sol, étape suivante.
pub(self) fn land_dust(x: &mut Cx) {
    dust(x);
    lib::sound(x, 0xec820);
    x.set32(Obj::Z, 0);
    x.add8(f::SUB3, 2);
    x.set8(VULNERABLE, 1);
    look(x, 0xc);
    mv(x, 0xc);
}

/// 0x63a50 : quatre nuages de poussière autour de l'ennemi.
pub(self) fn dust(x: &mut Cx) {
    for d2 in [-0x1c, -8, 8, 0x1c] {
        lib::shock(x, 0, 0, d2 as u16, 4);
    }
}

/// Un pas de fumée toutes les `mask + 1` images (`A5+0x6e` est le compteur d'images, que l'original masque en place).
pub(self) fn puff(x: &mut Cx, mask: u16, d0: u32, d1: u32, d2: i16, d3: i16) {
    let v = x.a5_u16(0x6e) & mask;
    x.set_a5_u16(0x6e, v);
    if v == 0 {
        lib::shock(x, d0, d1 as u16, d2 as u16, d3 as u16);
    }
}

/// 0x639fe : l'ennemi doit se retourner pour faire face à sa cible (faux sans cible ni aspect mémorisé).
pub(self) fn must_turn<R: Banc>(x: &mut Cx<'_, R>) -> bool {
    if x.u8(LOOK_PREV) == 0 || x.u16(f::TARGET) == 0 {
        return false;
    }
    let target = x.linked(f::TARGET);
    let dx = target.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X)) as i16;
    let want = if dx > 0 { 0x20 } else { 0 };
    let turn = want != x.u8(f::FLIP);
    turn
}

/// 0x63a98 : la cible est dans la même rangée (profondeur à 0x30 près) mais loin en X (plus de 0xf0).
pub(self) fn target_far_in_line<R: Banc>(x: &mut Cx<'_, R>) -> bool {
    if x.u16(f::TARGET) == 0 {
        return false;
    }
    let target = x.linked(f::TARGET);
    let dz = target.u16(x.b, Obj::Y).wrapping_sub(x.u16(Obj::Y)).wrapping_add(0x30);
    if dz > 0x60 {
        return false;
    }
    let dx = target.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X)).wrapping_add(0xf0);
    let far = dx > 0x1e0;
    far
}

/// 0x62f1e / 0x62f26 : choisit le coup suivant (`+6`) dans la ligne de la difficulté du profil `+0x2f` ;
/// `bias` = 8 pour la série lointaine.
pub(self) fn pick_attack(x: &mut Cx, bias: u8) {
    let profile = x.u8(GROUP) as u32;
    let list = x.data32(0x62f80 + bias as u32 + 4 * profile);
    let row = ((var::RANK.get(x.b) >> 1) << 5) as u32;
    let r = rand_and(x, 0x1f);
    let mv = x.data8(list + row + r);
    x.set8(f::SUB2, mv.wrapping_add(bias));
    x.set16(f::STATE, 0x0204);
    x.set8(f::SUB3, 0);
    if !x.in_arena(0) && x.u8(f::SUB2) == 0x10 {
        x.set8(f::SUB2, 8);
    }
}

/// 0x63790 : fin d'un geste : si la cible est dans une boîte de portée, un coup de la série `0x637fe` suit ; sinon debout.
pub(self) fn after_strike(x: &mut Cx) {
    if x.u16(f::TARGET) == 0 {
        return set_state_long(x, STANCE);
    }
    let close = lib::target_in_box(x, -0x30, 0x30, 8) || lib::target_in_box(x, -0xd0, 0x70, 8);
    if !close {
        return set_state_long(x, STANCE);
    }
    let row = ((var::RANK.get(x.b) >> 1) << 5) as u32;
    let r = rand_and(x, 0x1f);
    let mv = x.data8(0x637fe + row + r);
    x.set8(f::SUB2, mv);
    x.set16(f::STATE, 0x0208);
    x.set8(f::SUB3, 0);
}

pub(self) const STANCE: u32 = 0x0202_0000;
pub(self) const HELD: u32 = 0x0408_0000;
pub(self) const IDLE: u32 = 0x020a_0000;

// --- état 0 : création --------------------------------------------------------------------------

fn init(x: &mut Cx) {
    if paused(x) {
        return;
    }
    x.add8(f::STATE, 2);
    x.set8(0x6f, 1);
    x.set8(f::BOX_SET, 0x12);
    lib::initial_hp(x);
    for (off, v) in [(0x70, 0), (0x7e, 0xf), (0x7f, 0), (0x8e, 0), (0x78, 0xff), (0x81, 0xf), (0x80, 0x10)] {
        x.set8(off, v);
    }
    x.set32(f::IMMUNITY, 0);
    lib::set_immunities(x);
    let params: [(u32, u8); 10] = if x.u8(GROUP) == 0 {
        [(0x68, 2), (0x69, 2), (0x6a, 0), (0x6c, 3), (0x6d, 2), (0x72, 4), (0x73, 3), (0x74, 3), (0x75, 0x24), (0x76, 4)]
    } else {
        [(0x68, 2), (0x69, 2), (0x6a, 0), (0x6c, 4), (0x6d, 2), (0x72, 5), (0x73, 4), (0x74, 2), (0x75, 0x28), (0x76, 6)]
    };
    for (off, v) in params {
        x.set8(off, v);
    }
    if !pick_target(x) {
        return set_state_long(x, IDLE);
    }
    lib::draw_stance(x);
    lib::draw_6b(x);
    // état de départ, aspect et animation selon la variante
    let (state, look_v, anim) = match x.u8(Obj::VARIANT) {
        0 => (STANCE, 0xc, 2),
        1 => (0x0202_0600, 3, 3),
        2 => (0x0200_0400, 0, 0),
        3 => {
            x.set8(f::FLIP, 0x20);
            (0x0200_0600, 0, 0)
        }
        4 => (0x0200_0800, 0xf, 0xf),
        5 => (0x0200_0000, 0x17, 0x10),
        6 => (0x0200_0200, 0, 0),
        7 => (0x0200_0a00, 0x1a, 9),
        _ => return,
    };
    set_state_long(x, state);
    look(x, look_v);
    x.set8(VULNERABLE, 1);
    mv(x, anim);
}

// --- état 2 : actif -----------------------------------------------------------------------------

fn active(x: &mut Cx) {
    active_loop(x, |x| {
        x.c.d[0] = x.u8(f::SUB) as u32;
        match x.u8(f::SUB) {
            0 => arrival::run(x),
            2 => stance::run(x),
            4 => attacks::run(x),
            6 => attacks::run_b(x),
            8 => attacks::run_c(x),
            10 => attacks::idle(x),
            12 => attacks::grab(x),
            _ => {}
        }
        false
    });
}

handlers! {
    0x61364 "enemy_6" run,
}
