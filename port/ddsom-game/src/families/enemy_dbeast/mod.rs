//! Ennemi « Displacer Beast » et ses parties (ENEMY type 19, 0x71e44). Voir `docs/familles/enemy.md`.
//!
//! Un seul gestionnaire pour quatorze variantes (`+3`) : la bête elle-même et les pièces qui
//! l'accompagnent (membres, projectiles, ombre...), liées à elle par `+0x42` (parent) et visant un
//! joueur par `+0x50` (cible). L'état principal `+4` est celui de tous les ennemis (0 création,
//! 2 actif, 4 touché, 6 libération) ; en état actif `+5` choisit un groupe de comportements et `+6`,
//! `+7` leurs étapes. Un fichier par groupe :
//!
//! | `+5` | Fichier | Rôle (h) |
//! |---|---|---|
//! | 0 | `parts` | pièces (variantes 0 à 13 selon `+6`) : membres, projectiles, bonds |
//! | 2 | `stance` | la bête debout : poursuite, regard, choix d'une attaque |
//! | 4 | `attacks` | coups de la bête (`+6` = le coup) |
//! | 6 | `attacks_b` | seconde série de coups |
//! | 8, 10, 12 | `moves` | charges, bonds, entrée en scène |
//! | état 4 | `hit` | réactions aux coups |
//!
//! Les pièces ne servent pas d'objets séparés dans l'original : le même code les anime toutes.

use super::cx::{handlers, sub, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

mod attacks;
mod attacks_b;
mod hit;
mod moves;
mod parts;
mod stance;

/// Champs propres à ce type (les champs communs sont dans `enemy_common::f`).
pub(self) mod g {
    pub const VARIANT: u32 = 0x3;
    /// Direction de déplacement (index de table de vitesses, 0 à 31).
    pub const ANGLE: u32 = 0x2d;
    /// Profil d'IA (0 ou 1, difficulté de la bête).
    pub const PROFILE: u32 = 0x2f;
    /// Image-clé de l'animation en cours (posé par les scripts d'animation).
    pub const KEY_FRAME: u32 = 0x33;
    /// Priorité d'affichage.
    pub const LAYER: u32 = 0x3b;
    /// Parent (mot, adresse RAM) et cible (mot, joueur).
    pub const PARENT: u32 = 0x42;
    pub const TARGET: u32 = 0x50;
    /// Dégâts du coup en cours : `+0x44` type, `+0x45` force.
    pub const STRIKE_KIND: u32 = 0x44;
    pub const STRIKE_POWER: u32 = 0x45;
    /// Vulnérable (1) ou invulnérable (0) pendant le geste.
    pub const VULNERABLE: u32 = 0x67;
    /// Paramètres d'IA posés à la création.
    pub const AI_STEP: u32 = 0x6b;
    pub const AI_REPEAT: u32 = 0x6e;
    pub const AI_TURN: u32 = 0x71;
    pub const AI_KICK: u32 = 0x77;
    pub const AI_SHOT: u32 = 0x79;
    /// Compte à rebours de sortie de l'entrée en scène.
    pub const ENTRY_TIMER: u32 = 0x90;
    /// Minuterie de l'étape en cours (octet ou mot).
    pub const TIMER: u32 = 0xa0;
    /// Animation voulue / animation en cours (pour ne pas relancer la même).
    pub const WANT_ANIM: u32 = 0xa5;
    pub const CUR_ANIM: u32 = 0xa2;
    /// Le geste est fini (posé par la bête pour ses parties).
    pub const DONE: u32 = 0xa3;
    /// Étape à reprendre après un demi-tour : `+0xa4` = `+6`, `+0xa6` = `+4..+7`.
    pub const RESUME_SUB2: u32 = 0xa4;
    pub const RESUME_STATE: u32 = 0xa6;
    /// Nombre de répétitions d'un coup (`+0xab`, `+0xae`) et compteur de gestes (`+0xb5`).
    pub const REPEAT: u32 = 0xab;
    pub const REPEAT_LEFT: u32 = 0xae;
    pub const SHOTS: u32 = 0xb5;
    /// Point visé (X, Y) pour une pièce qui rejoint le parent.
    pub const AIM_X: u32 = 0xb0;
    pub const AIM_Y: u32 = 0xb2;
    /// Rang de la pièce dans la bête (−1 : pas une pièce) et décalage de la série d'animations.
    pub const PART: u32 = 0xbc;
    pub const ANIM_SHIFT: u32 = 0xbf;
    /// Animation lancée en dernier (pour ne pas la relancer).
    pub const LAST_ANIM: u32 = 0xaf;
}

use g::*;
use crate::objects::Courant;

/// Jeux d'animations : mouvements et coups.
const SET_A: u32 = 0x166fb6;
const SET_B: u32 = 0x167054;
/// Décalages de numéros d'animation par série (`+0xbf`), tables de la ROM.
const SHIFT_A: u32 = 0x74f7a;
const SHIFT_B: u32 = 0x74f8a;
const SHIFT_PICK: u32 = 0x74f56;
/// États longs (`+4..+7`).
const ACTIVE_STANCE: u32 = 0x0202_0000;
const ACTIVE_IDLE: u32 = 0x020a_0000;
/// État touché, sous-état 8 : la bête est projetée ou s'enfuit.
const KNOCKED: u32 = 0x0408_0000;

/// Routines du moteur et de l'IA.
mod call {
    pub const ANGLE: u32 = 0x137e;
    pub const FACE_TARGET: u32 = 0xa470;
    pub const TURN_TO_TARGET: u32 = 0x788c;
}

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

/// 0x74f66 : numéro d'animation de la série des mouvements (les numéros à partir de 0x4b ne sont pas
/// décalés).
fn anim_a(x: &mut Cx, n: u8) -> u32 {
    let shift = x.u8(ANIM_SHIFT);
    if (n as i8) >= 0x4b {
        return n as u32;
    }
    x.c.d[1] = shift as u32;
    n.wrapping_add(x.rom8(SHIFT_A + shift as u32)) as u32
}

/// 0x74f7e : numéro d'animation de la série des coups, toujours décalé.
fn anim_b(x: &mut Cx, n: u8) -> u32 {
    let shift = x.u8(ANIM_SHIFT);
    x.c.d[1] = shift as u32;
    n.wrapping_add(x.rom8(SHIFT_B + shift as u32)) as u32
}

/// Joue l'animation `n` de la série des mouvements.
fn play(x: &mut Cx, n: u8) {
    let n = anim_a(x, n);
    x.anim_start(SET_A, n as u16);
}

/// Joue l'animation `n` de la série des coups.
fn play_b(x: &mut Cx, n: u8) {
    let n = anim_b(x, n);
    x.anim_start(SET_B, n as u16);
}

/// Joue l'animation `n` (série des mouvements) si ce n'est pas déjà celle en cours (`+0xaf`).
fn play_once(x: &mut Cx, n: u8) {
    let n = anim_a(x, n);
    if n as u8 != x.u8(LAST_ANIM) {
        x.set8(LAST_ANIM, n as u8);
        x.anim_start(SET_A, n as u16);
    }
}

/// Un pas d'animation (le résultat « fini » est ignoré).
fn step(x: &mut Cx) {
    x.anim_step();
}

/// Déplacement selon la table de vitesses (X, Y).
fn move_xy(x: &mut Cx) {
    x.move_by_table_xy();
}

/// Choisit la table de vitesses `n`.
fn speed(x: &mut Cx, n: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, n as u16);
}

/// Pose les trois vitesses d'un bond : X, Z, accélération en Z.
fn set_velocity(x: &mut Cx, vx: i16, vz: u16, az: i16) {
    x.set16(0x14, vx as u16);
    x.set16(0x16, 0);
    x.set16(0x18, vz);
    x.set16(0x1a, az as u16);
}

/// Le long d'état (`+4..+7`) d'un autre objet.
fn state_of(x: &mut Cx, o: Obj) -> u32 {
    o.u32(x.b, f::STATE)
}

fn set_state_long(x: &mut Cx, v: u32) {
    x.set32(f::STATE, v);
}

/// Le parent (`+0x42`) ou `None` s'il n'y en a pas.
fn parent(x: &mut Cx) -> Option<Obj> {
    if x.u16(PARENT) == 0 {
        return None;
    }
    Some(x.linked(PARENT))
}

/// 0x725c8 : pièce sans parent, elle redevient une bête debout et joue l'animation 8.
fn orphan(x: &mut Cx) {
    set_state_long(x, ACTIVE_STANCE);
    play(x, 8);
}

/// Coordonnées d'écran (caméra) : X en `A5+0x556`, Y en `A5+0x55a`.
fn camera(x: &mut Cx) -> (u16, u16) {
    (x.a5_u16(0x556), x.a5_u16(0x55a))
}

/// 0x71ff4 : se place sur le parent, décalé selon le rang de la pièce (X inversé quand la pièce
/// regarde à gauche).
fn follow_offset(x: &mut Cx) {
    const OFFSETS: u32 = 0x7202a;
    let p = x.linked(PARENT);
    let (px, py) = (p.u16(x.b, 0x8), p.u16(x.b, 0x10));
    x.set16(0x8, px);
    x.set16(0x10, py);
    let row = (x.u8(PART).wrapping_mul(4)) as u32;
    let mut dx = x.rom16(OFFSETS + row);
    let dy = x.rom16(OFFSETS + row + 2);
    if x.u8(f::FLIP) != 0 {
        dx = dx.wrapping_neg();
    }
    x.add16(0x8, dx);
    x.add16(0x10, dy);
}

/// 0x7208c : se place sur le parent (position, orientation, altitude) selon le rang de la pièce.
/// Sans parent, la pièce redevient une bête debout (et la fonction rend la main à l'appelant, qui
/// continue).
fn follow_body(x: &mut Cx) {
    const OFFSETS: u32 = 0x720ee;
    let Some(p) = parent(x) else { return orphan(x) };
    let (px, py, flip) = (p.u16(x.b, 0x8), p.u16(x.b, 0x10), p.u8(x.b, f::FLIP));
    x.set16(0x8, px);
    x.set16(0x10, py);
    x.set8(f::FLIP, flip);
    x.set32(0xc, 0);
    let row = (x.u8(PART).wrapping_mul(8)) as u32;
    let mut dx = x.rom16(OFFSETS + row);
    let dy = x.rom16(OFFSETS + row + 2);
    let dz = x.rom16(OFFSETS + row + 4);
    let fraction = x.rom16(OFFSETS + row + 6) as i16 as i32 as u32;
    if flip != 0 {
        dx = dx.wrapping_neg();
    }
    x.add16(0x8, dx);
    x.add16(0x10, dy);
    x.add16(0xc, dz);
    x.add32(0x10, fraction);
}

/// 0x74eac : la bête doit se retourner pour faire face à sa cible.
fn player_ahead(x: &mut Cx) -> bool {
    needs_turn(x)
}

/// 0x74efe : la cible est loin, en profondeur et en largeur.
fn target_far(x: &mut Cx) -> bool {
    let Some(t) = target(x) else { return false };
    let dy = t.u16(x.b, 0x10).wrapping_sub(x.u16(0x10)).wrapping_add(0x30);
    if dy <= 0x60 {
        return false;
    }
    let dx = t.u16(x.b, 0x8).wrapping_sub(x.u16(0x8)).wrapping_add(0xa0);
    dx > 0x140
}

/// 0x74f8e : un joueur est tout près (boîte devant la bête).
fn player_near(x: &mut Cx) -> bool {
    for p in 0..4 {
        let player = Obj(0xff85de + 0x200 * p);
        if player.u8(x.b, 0) != 0 && target_in_box(x, 0, 0x60, 0x28) {
            return true;
        }
    }
    false
}

// --- état 0 : création --------------------------------------------------------------------------

/// Paramètres d'IA par profil (`+0x2f`) : (champ, valeur).
const PROFILE_A: [(u32, u8); 12] =
    [(0x68, 2), (0x69, 2), (0x6a, 1), (0x6c, 2), (0x6d, 2), (0x72, 2), (0x73, 2), (0x74, 2), (0x75, 0x18), (0x76, 2), (0x81, 4), (0x80, 5)];
const PROFILE_B: [(u32, u8); 12] =
    [(0x68, 2), (0x69, 2), (0x6a, 1), (0x6c, 3), (0x6d, 3), (0x72, 4), (0x73, 4), (0x74, 3), (0x75, 0x20), (0x76, 3), (0x81, 5), (0x80, 6)];

fn init(x: &mut Cx) {
    if paused(x) {
        return;
    }
    x.add8(f::STATE, 2);
    x.set8(f::BOX_SET, 0xc);
    x.call(rom::INITIAL_HP);
    x.call(rom::IMMUNITIES);
    x.set8(0x70, 0);
    x.set8(0x7e, 0xa);
    x.set8(0x7f, 0);
    x.set8(0x8e, 0);
    x.set8(0x78, 0xff);
    let profile = match x.u8(PROFILE) {
        0 | 1 => {
            // décalage aléatoire de la série d'animations de cette bête
            let r = random(x) as u8 & 0xf;
            let shift = x.rom8(SHIFT_PICK + r as u32);
            x.set8(ANIM_SHIFT, shift);
            if x.u8(PROFILE) == 0 { PROFILE_A } else { PROFILE_B }
        }
        _ => return,
    };
    for (off, v) in profile {
        x.set8(off, v);
    }
    pick_target(x);
    if (x.u8(VARIANT) as i8) > 3 {
        if x.u16(TARGET) == 0 {
            set_state_long(x, ACTIVE_IDLE);
            return;
        }
        x.call(call::FACE_TARGET);
        x.call(call::TURN_TO_TARGET);
    }
    parts::spawn(x);
}

// --- état 2 : actif -----------------------------------------------------------------------------

fn active(x: &mut Cx) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
        return held(x);
    }
    if paused(x) {
        return held(x);
    }
    match x.u8(f::SUB) {
        0 => parts::run(x),
        2 => stance::run(x),
        4 => attacks::run(x),
        6 => attacks_b::run(x),
        8 => moves::charge(x),
        10 => moves::enter(x),
        12 => moves::leave(x),
        _ => {}
    }
    // les pièces au repos (sous-état 0 et 0xc) ne testent pas la zone de jeu
    let s = x.u16(f::STATE);
    if s != 0x0200 && s != 0x020c {
        x.call(IN_ZONE);
        x.call(SHADOW);
    }
    x.reinsert();
}

/// Gel ou pause : une pièce qui attend son parent le suit quand même.
fn held(x: &mut Cx) {
    if x.u8(f::SUB) == 0 && x.u8(f::SUB2) == 2 {
        follow_body(x);
    }
}

handlers! {
    0x71e44 "enemy_19" run,
}
