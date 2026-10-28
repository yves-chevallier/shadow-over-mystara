//! Ennemi « Shadow Elf » (ENEMY type 12, 0x69384). Voir `docs/familles/enemy.md`.
//!
//! Elfe sombre qui frappe à l'épée, disparaît et retombe sur sa cible. Charpente commune : `+4` état
//! (0 création, 2 actif, 4 touché, 6 libération), `+5` groupe de comportements, `+6` et `+7` étapes.
//! En état actif :
//!
//! | `+5` | Fichier | Rôle (h) |
//! |---|---|---|
//! | 0 | `mod` | arrivée selon la variante `+3` |
//! | 2 | `stance` | debout : marche, demi-tour, attente, choix d'un coup |
//! | 4 | `attacks` | coups (`+6` = le coup) |
//! | 6 | `attacks` | coups d'après-bond |
//! | 8 | `attacks` | coups de repli |
//! | 10 | `stance` | repos : attend qu'un joueur soit en vie |
//! | état 4 | `hit` | réactions aux coups |
//!
//! Animations : 0x164174 (mouvements) et 0x164184 (coups). L'état actif exécute toujours, hors gel, le
//! test de proximité 0x81aa4, le test de zone, l'ombre et la remise en liste.

use crate::families::cx::Banc;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

mod attacks;
mod hit;
mod stance;

/// Champs propres à ce type (les champs communs sont dans `enemy_common::f`).
pub(self) mod g {
    /// Dégâts du coup en cours : type et force.
    pub const STRIKE_KIND: u32 = 0x44;
    pub const STRIKE_POWER: u32 = 0x45;
    /// Vulnérable (1) ou non.
    pub const VULNERABLE: u32 = 0x67;
    /// Compteurs d'IA : coups restants, délai avant un nouveau choix, attente, marche.
    pub const AI_CHOICE: u32 = 0x6b;
    pub const AI_WAIT: u32 = 0x6e;
    pub const AI_WALK: u32 = 0x71;
    pub const AI_KIND: u32 = 0x78;
    /// Compteur de répétitions d'un coup et minuterie.
    pub const REPEAT: u32 = 0xa2;
    /// Dernière direction d'IA (animation de marche) et son compte à rebours.
    pub const WALK_ANIM: u32 = 0xa1;
    pub const WALK_ANIM_TIMER: u32 = 0xa4;
    /// Coup prévu après l'approche et animation de mouvement de ce coup.
    pub const NEXT_ATTACK: u32 = 0xa5;
    pub const APPROACH_ANIM: u32 = 0xa6;
    /// Attente restante avant l'étape suivante.
    pub const COOLDOWN: u32 = 0x77;
}

use g::*;
use crate::objects::Courant;

/// Jeux d'animations : mouvements et coups.
const SET_BODY: u32 = 0x164174;
const SET_STRIKE: u32 = 0x164184;
/// États longs (`+4..+7`).
const STANCE: u32 = 0x0202_0000;
const IDLE: u32 = 0x020a_0000;
/// Tables de la ROM : coups à choisir par niveau de difficulté (`A5+0x7f92`), 32 octets par niveau.
const FALLBACK_MOVES: u32 = 0x6a480;
const CLOSE_ATTACKS: u32 = 0x6a73c;
const FAR_ATTACKS: u32 = 0x6a93c;

pub fn run(x: &mut Cx) {
    if suspended(x) {
        return;
    }
    if var::GAME_ON.get(x.b) != 0 {
        x.tick_counters();
    }
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        st::DEAD => free(x),
        _ => {}
    }
}

// --- aides ------------------------------------------------------------------------------------

fn body<R: Banc>(x: &mut Cx<'_, R>, n: u32) {
    x.anim_start(SET_BODY, n as u16);
}

fn strike(x: &mut Cx, n: u32) {
    x.anim_start(SET_STRIKE, n as u16);
}

fn step<R: Banc>(x: &mut Cx<'_, R>) -> bool {
    x.anim_step()
}

fn move_xy<R: Banc>(x: &mut Cx<'_, R>) {
    x.move_by_table_xy();
}

/// 0x69528 : saut — effet 0xecfe4 et animation 3.
fn launch<R: Banc>(x: &mut Cx<'_, R>) {
    lib::sound(x, 0xecfe4);
    body(x, 3);
}

/// 0x6954a : accroupi (animation 4).
fn crouch<R: Banc>(x: &mut Cx<'_, R>) {
    body(x, 4);
}

/// 0x6953c : attend la fin de l'animation, puis passe au `+6` suivant.
fn wait_anim(x: &mut Cx) {
    if step(x) {
        x.add8(f::SUB2, 2);
    }
}

/// 0x6957c : l'animation finie, retour debout.
fn end_to_stance<R: Banc>(x: &mut Cx<'_, R>) {
    if step(x) {
        set_state_long(x, STANCE);
    }
}

/// 0x6a680 : retour debout ; si la décision 0x79ce le veut, au repos.
fn back_to_stance<R: Banc>(x: &mut Cx<'_, R>) {
    set_state_long(x, STANCE);
    if lib::maybe_draw_6e(x) {
        x.set8(f::SUB, 0xa);
    }
}

/// 0x6a452 : fin d'un coup — le suivant est tiré dans la ligne de la difficulté (32 octets).
fn end_attack(x: &mut Cx) {
    x.set8(AI_KIND, 0xff);
    let row = (var::RANK.get(x.b) >> 1) << 5;
    set_dw(x, 0, row);
    let n = rand_and(x, 0x1f);
    x.set8(f::SUB, 8);
    let mv = x.data8(FALLBACK_MOVES + row as u32 + n);
    x.set8(f::SUB2, mv);
    x.set8(f::SUB3, 0);
}

// --- état 0 : création --------------------------------------------------------------------------

fn init(x: &mut Cx) {
    if paused(x) {
        return;
    }
    x.add8(f::STATE, 2);
    x.set8(f::BOX_SET, 0x18);
    lib::initial_hp(x);
    lib::set_immunities(x);
    for (off, v) in [(0x68, 4), (0x69, 1), (0x70, 0), (0x6a, 1), (0x6c, 3), (0x6d, 3), (0x72, 3), (0x73, 2), (0x74, 3), (0x75, 0x38), (0x76, 3), (REPEAT, 0x20), (WALK_ANIM, 0), (WALK_ANIM_TIMER, 2)] {
        x.set8(off, v);
    }
    if var::GAME_ON.get(x.b) != 0 {
        x.set8(VULNERABLE, 1);
    }
    for (off, v) in [(0x8e, 0), (0x7f, 0), (0x7e, 0xe)] {
        x.set8(off, v);
    }
    x.set32(0x8a, 0);
    x.set8(APPROACH_ANIM, 0);
    x.set8(AI_KIND, 0xff);
    x.set8(0x80, 0xc);
    x.set8(0x81, 0xb);
    // la création enchaîne directement sur l'état actif
    active(x);
}

// --- état 2 : actif -----------------------------------------------------------------------------

fn active(x: &mut Cx) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else if !paused(x) {
        x.c.d[0] = x.u8(f::SUB) as u32;
        match x.u8(f::SUB) {
            0 => arrival(x),
            2 => stance::run(x),
            4 => attacks::run(x),
            6 => attacks::run_b(x),
            8 => attacks::run_fallback(x),
            10 => stance::rest(x),
            _ => {}
        }
    }
    super::boss_common::push_out_of_boss(x);
    zone_and_terrain(x);
    x.reinsert();
}

/// Sous-état 0 : arrivée selon la variante `+3` (octet pair, indice de table : 0 marche, 2 chute, 4 bond,
/// 6 chute lente).
fn arrival(x: &mut Cx) {
    match x.u8(Obj::VARIANT) {
        0 => {
            if x.u8(f::SUB2) != 0 {
                move_xy(x);
                step(x);
                let t = x.u8(REPEAT).wrapping_sub(1);
                x.set8(REPEAT, t);
                if (t as i8) < 0 {
                    set_state_long(x, STANCE);
                }
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(REPEAT, 0x14);
            let angle = if x.u8(f::FLIP) != 0 { 8 } else { 0x18 };
            x.set8(Obj::SPEED_INDEX, angle);
            x.select_speed(&crate::objects::motion::SPEEDS_B, 0x12);
            body(x, 1);
        }
        2 => match x.u8(f::SUB2) {
            0 => {
                x.set16(Obj::VZ, 0);
                x.set16(Obj::AZ, -0x40i16 as u16);
                x.add8(f::SUB2, 2);
                launch(x);
            }
            2 => wait_anim(x),
            4 => land(x),
            6 => end_to_stance(x),
            _ => {}
        },
        4 => match x.u8(f::SUB2) {
            0 => {
                x.add8(f::SUB2, 2);
                x.set16(Obj::VZ, 0);
                x.set16(Obj::AZ, -0x38i16 as u16);
                x.set16(Obj::VX, -0x280i16 as u16);
                x.set16(Obj::AX, 0);
                if x.u8(f::FLIP) != 0 {
                    negate(x, Obj::VX);
                }
                launch(x);
            }
            2 => wait_anim(x),
            4 => {
                x.add8(f::SUB2, 2);
                x.anim_start(SET_STRIKE, 1);
            }
            6 => {
                x.move_xz();
                if (x.u16(Obj::Z) as i16) <= 0 {
                    x.add8(f::SUB2, 2);
                    x.set32(Obj::Z, 0);
                    body(x, 5);
                } else if (x.u16(Obj::Z) as i16) <= 0x30 {
                    step(x);
                }
            }
            8 => end_to_stance(x),
            _ => {}
        },
        6 => match x.u8(f::SUB2) {
            0 => {
                x.set16(Obj::VZ, 0);
                x.set16(Obj::AZ, -0x40i16 as u16);
                x.add8(f::SUB2, 2);
                launch(x);
            }
            2 => wait_anim(x),
            4 => {
                x.add8(f::SUB2, 2);
                x.set32(Obj::Z, 0);
                body(x, 5);
            }
            6 => end_to_stance(x),
            _ => {}
        },
        _ => {}
    }
}

/// 0x69558 : descente (déplacement en hauteur) puis réception.
fn land<R: Banc>(x: &mut Cx<'_, R>) {
    x.move_z();
    if (x.u16(Obj::Z) as i16) > 0 {
        return;
    }
    x.add8(f::SUB2, 2);
    x.set32(Obj::Z, 0);
    body(x, 5);
}

handlers! {
    0x69384 "enemy_12" run,
}
