//! Ennemi « Owlbear » (ENEMY type 4, 0x5d87a). Voir `docs/familles/enemy.md`.
//!
//! Même charpente que le Riyakaa (`enemy_riyakaa`) : `+4` état principal (0 création, 2 actif,
//! 4 touché, 6 libération), `+5` groupe de comportements, `+6` et `+7` étapes. En état actif :
//!
//! | `+5` | Fichier | Rôle (h) |
//! |---|---|---|
//! | 0 | `stance` | arrivée selon la variante `+3` |
//! | 2 | `stance` | debout : poursuite, demi-tour, attente, approche |
//! | 4 | `attacks` | coups (`+6` = le coup) |
//! | 6 | `attacks` | coups de l'après-arrivée : bond, charge |
//! | 8 | `moves` | charges et sauts vers un joueur |
//! | 10 | `stance` | repos, attend qu'un joueur soit en vie |
//! | état 4 | `hit` | réactions aux coups |
//!
//! Animations : 0x160e26 (mouvements) et 0x160e44 (coups). `+0xa1` est l'animation voulue,
//! `+0xa2` celle en cours (on ne relance pas la même).

use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

mod attacks;
mod hit;
mod moves;
mod stance;

/// Champs propres à ce type (les champs communs sont dans `enemy_common::f`).
pub(self) mod g {
    /// Dégâts du coup en cours : `+0x44` type, `+0x45` force.
    pub const STRIKE_KIND: u32 = 0x44;
    pub const STRIKE_POWER: u32 = 0x45;
    /// Vulnérable (1) ou non (0) pendant le geste.
    pub const VULNERABLE: u32 = 0x67;
    /// Paramètres d'IA posés à la création.
    pub const AI_STEP: u32 = 0x6b;
    pub const AI_REPEAT: u32 = 0x6e;
    pub const AI_TURN: u32 = 0x71;
    pub const AI_KICK: u32 = 0x77;
    pub const AI_SHOT: u32 = 0x79;
    /// Délai avant la première attaque (octet fort du mot `+0xaa`).
    pub const DELAY: u32 = 0xaa;
    /// Minuterie de l'étape en cours.
    pub const TIMER: u32 = 0xa0;
    /// Animation voulue et animation en cours.
    pub const WANT_ANIM: u32 = 0xa1;
    pub const CUR_ANIM: u32 = 0xa2;
    /// État long (`+0xa6`) et étape (`+0xa4`) à reprendre après un demi-tour.
    pub const RESUME_STATE: u32 = 0xa6;
    pub const RESUME_STEP: u32 = 0xa4;
    /// Octet non nul : l'état touché ne teste pas la zone de jeu.
    pub const NO_ZONE: u32 = 0xa5;
    /// Répétitions d'un coup : `+0xac` (série), `+0xad` (même animation).
    pub const SERIES: u32 = 0xac;
    pub const REPEAT_LEFT: u32 = 0xad;
}

use g::*;
use crate::objects::Courant;

/// Jeux d'animations : mouvements et coups.
const SET_BODY: u32 = 0x160e26;
const SET_STRIKE: u32 = 0x160e44;
/// États longs (`+4..+7`).
const ACTIVE_STANCE: u32 = 0x0202_0000;
const ACTIVE_IDLE: u32 = 0x020a_0000;
const ACTIVE_TURN: u32 = 0x0202_0200;
const ACTIVE_ENGAGE: u32 = 0x0202_0002;
const KNOCKED: u32 = 0x0408_0000;

pub fn run(x: &mut Cx) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        st::DEAD => free(x),
        _ => {}
    }
}

// --- aides ------------------------------------------------------------------------------------

/// Pose l'animation voulue `want` ; lance le script `n` du jeu `set` si ce n'est pas déjà la
/// courante.
fn pose(x: &mut Cx, want: u8, set: u32, n: u32) {
    x.set8(WANT_ANIM, want);
    if want != x.u8(CUR_ANIM) {
        x.set8(CUR_ANIM, want);
        x.anim_start(set, n as u16);
    }
}

/// Ce que doit faire un coup quand il voudrait rejouer son animation (`+0xa1`).
enum Replay {
    New,
    Again,
    End,
}

/// Comme `pose`, pour un coup répété : si l'animation tourne déjà, une répétition de moins
/// (`+0xad`), et à zéro la bête se remet debout.
fn replay(x: &mut Cx, want: u8) -> Replay {
    x.set8(WANT_ANIM, want);
    if want != x.u8(CUR_ANIM) {
        x.set8(CUR_ANIM, want);
        return Replay::New;
    }
    let left = x.u8(REPEAT_LEFT).wrapping_sub(1);
    x.set8(REPEAT_LEFT, left);
    if left == 0 {
        set_state_long(x, ACTIVE_STANCE);
        return Replay::End;
    }
    Replay::Again
}

fn body(x: &mut Cx, n: u32) {
    x.anim_start(SET_BODY, n as u16);
}

fn strike(x: &mut Cx, n: u32) {
    x.anim_start(SET_STRIKE, n as u16);
}

fn step(x: &mut Cx) {
    x.anim_step();
}

fn move_xy(x: &mut Cx) {
    x.move_by_table_xy();
}

fn speed(x: &mut Cx, n: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
}

fn set_state_long(x: &mut Cx, v: u32) {
    x.set32(f::STATE, v);
}

/// Direction de marche selon l'orientation : `flipped` si la bête regarde à gauche (octet `+0x2c` non
/// nul), `otherwise` sinon.
fn set_angle(x: &mut Cx, flipped: u8, otherwise: u8) {
    let angle = if x.u8(f::FLIP) != 0 { flipped } else { otherwise };
    x.set8(Obj::SPEED_INDEX, angle);
}

/// Écart d'abscisse de la cible à la bête (positif : la cible est à droite), `None` sans cible.
fn target_dx(x: &mut Cx) -> Option<i16> {
    let t = target(x)?;
    Some(t.u16(x.b, 0x8).wrapping_sub(x.u16(0x8)) as i16)
}

/// Même chose en profondeur (positif : la cible est plus bas à l'écran).
fn target_dy(x: &mut Cx) -> Option<i16> {
    let t = target(x)?;
    Some(t.u16(x.b, 0x10).wrapping_sub(x.u16(0x10)) as i16)
}

/// 0x5f710 : la cible est derrière la bête, elle se retourne.
fn turn_if_behind(x: &mut Cx) {
    if needs_turn(x) {
        set_state_long(x, ACTIVE_TURN);
    }
}

// --- état 0 : création --------------------------------------------------------------------------

const AI_PARAMS: [(u32, u8); 14] =
    [(0x68, 2), (0x69, 2), (0x70, 0), (0x6a, 2), (0x6c, 2), (0x6d, 2), (0x72, 4), (0x73, 3), (0x74, 2), (0x75, 4), (0x76, 3), (0x78, 0xff), (0x81, 9), (0x80, 0xa)];

fn init(x: &mut Cx) {
    if paused(x) {
        return;
    }
    let scene = var::SCENE.get(x.b);
    if scene == 6 || scene == 0xc {
        x.set8(0x2a, 1);
        x.set8(0x2b, 0x18);
    }
    x.add8(f::STATE, 2);
    x.set8(f::BOX_SET, 0x10);
    lib::initial_hp(x);
    for (off, v) in AI_PARAMS {
        x.set8(off, v);
    }
    x.set32(f::IMMUNITY, 0);
    lib::set_immunities(x);
    x.set8(0x7e, 0xd);
    x.set8(0x7f, 0);
    x.set8(0x8e, 0);
    x.set16(DELAY, 0x258);
    pick_target(x);
    lib::draw_stance(x);
    lib::draw_6b(x);
    match x.u8(Obj::VARIANT) {
        0 => {
            set_state_long(x, ACTIVE_STANCE);
            x.set8(WANT_ANIM, 2);
            x.set8(VULNERABLE, 1);
            body(x, 2);
        }
        1 => {
            set_state_long(x, 0x0200_0000);
            x.set8(TIMER, 0x29);
            x.set8(VULNERABLE, 1);
            x.set8(WANT_ANIM, 2);
            // l'original ne mémorise pas ici l'animation lancée
            if x.u8(WANT_ANIM) != x.u8(CUR_ANIM) {
                body(x, 2);
            }
        }
        2 => {
            x.set16(0x14, (-0x500i16) as u16);
            set_state_long(x, 0x0204_0404);
            x.set8(WANT_ANIM, 0xc);
            x.set8(VULNERABLE, 0);
            strike(x, 1);
            attacks::leap_begin(x);
        }
        _ => {}
    }
}

// --- état 2 : actif -----------------------------------------------------------------------------

fn active(x: &mut Cx) {
    active_loop(x, |x| {
        match x.u8(f::SUB) {
            0 => stance::arrival(x),
            2 => stance::run(x),
            4 => attacks::run(x),
            6 => attacks::run_b(x),
            8 => moves::run(x),
            10 => stance::rest(x),
            _ => {}
        }
        false
    });
}

handlers! {
    0x5d87a "enemy_4" run,
}
