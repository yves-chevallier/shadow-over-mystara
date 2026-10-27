//! Ennemi « Skeleton » (ENEMY type 1, 0x57cc4). Voir `docs/familles/enemy.md`.
//!
//! Squelette à épée (série `+0x2f` = 0) ou squelette armé (série 1). Charpente commune : `+4` état
//! (0 création, 2 actif, 4 touché, 6 libération), `+5` groupe de comportements, `+6` et `+7`
//! étapes. En état actif :
//!
//! | `+5` | Fichier | Rôle (h) |
//! |---|---|---|
//! | 0 | `mod` | arrivée selon la variante `+3` |
//! | 2 | `stance` | debout : idée, marche, attente, demi-tour, choix d'une attaque |
//! | 4 | `attacks` | coups au corps à corps (`+6` = le coup) |
//! | 6 | `leaps` | bonds, garde, sortie d'écran |
//! | 8 | `leaps` | esquive latérale et pas de côté |
//! | 10 | `leaps` | chute d'arrivée |
//! | état 4 | `hit` | réactions aux coups |
//!
//! Animations : 0x15eed8 (mouvements) et 0x15eef4 (coups).

use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

mod attacks;
mod hit;
mod leaps;
mod stance;

/// Champs propres à ce type (les champs communs sont dans `enemy_common::f`).
pub(self) mod g {
    /// Série : 0 squelette, 1 squelette armé.
    pub const SERIES: u32 = 0x2f;
    /// Compteur d'images de l'animation (mot) et sa valeur courante.
    pub const ANIM_COUNT: u32 = 0x30;
    pub const ANIM_STATE: u32 = 0x32;
    /// Dégâts du coup en cours : type et force.
    pub const STRIKE_KIND: u32 = 0x44;
    pub const STRIKE_POWER: u32 = 0x45;
    /// Table de vitesses de déplacement (long).
    pub const MOVE_TABLE: u32 = 0x4c;
    /// Vulnérable (1) ou non.
    pub const VULNERABLE: u32 = 0x67;
    /// Posture d'IA : 9 = en garde (riposte à un coup).
    pub const POSTURE: u32 = 0x68;
    /// Compteurs d'IA : délai avant un nouveau choix, avant une attente, demi-tours.
    pub const AI_CHOICE: u32 = 0x6b;
    pub const AI_WAIT: u32 = 0x6e;
    pub const AI_WALK: u32 = 0x71;
    /// Attente restante avant l'étape suivante d'un coup.
    pub const COOLDOWN: u32 = 0x77;
    pub const AI_KIND: u32 = 0x78;
    pub const AI_SHOT: u32 = 0x79;
    /// Jeux de boîtes de marche.
    pub const WALK_ARGS_Y: u32 = 0x7a;
    pub const WALK_ARGS_Z: u32 = 0x7c;
    /// Minuterie (mot) et position mémorisée.
    pub const TIMER: u32 = 0xa2;
    pub const SAVED_POS: u32 = 0xa4;
    /// Côté où se trouve la cible (0 devant, 2 derrière), posé par la détection.
    pub const SIDE: u32 = 0xa6;
    /// Drapeau « saisi » : l'ennemi part en chute.
    pub const CAUGHT: u32 = 0xa7;
    /// Riposte : drapeau (`+0xa8`, 0xff = au hasard) et coup à jouer (`+0xa9`).
    pub const COUNTER: u32 = 0xa8;
    pub const COUNTER_MOVE: u32 = 0xa9;
    /// Drapeaux de réaction spéciale (1 chute, 2 recul, 3 projection).
    pub const SPECIAL_HIT: u32 = 0x8f;
}

use g::*;
use crate::objects::Courant;

/// Jeux d'animations : mouvements et coups.
const SET_BODY: u32 = 0x15eed8;
const SET_STRIKE: u32 = 0x15eef4;
/// États longs (`+4..+7`).
const STANCE: u32 = 0x0202_0000;
const STANCE_ALT: u32 = 0x0202_0600;
const AFTER_TURN: u32 = 0x0202_0002;
const IDLE: u32 = 0x020a_0000;
const DASH: u32 = 0x0206_0800;
const KNOCKED: u32 = 0x0408_0000;
/// Tables de la ROM : attaques à choisir par niveau de difficulté (`A5+0x7f92`), 32 octets par niveau.
const CLOSE_ATTACKS: u32 = 0x5932a;
const FAR_ATTACKS: u32 = 0x5952a;
const NEXT_MOVES: u32 = 0x59982;

pub fn run(x: &mut Cx) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        st::DEAD => dead(x),
        _ => {}
    }
}

// --- aides ------------------------------------------------------------------------------------

fn body(x: &mut Cx, n: u32) {
    x.anim_start(SET_BODY, n as u16);
}

fn strike(x: &mut Cx, n: u32) {
    x.anim_start(SET_STRIKE, n as u16);
}

fn step(x: &mut Cx) -> bool {
    x.anim_step()
}

fn move_xy(x: &mut Cx) {
    x.move_by_table_xy();
}

/// Fin des coups : étape suivante tirée dans la table par difficulté, ou retour debout.
fn end_attack(x: &mut Cx) {
    x.set8(AI_KIND, 0xff);
    if x.u16(f::TARGET) != 0 {
        load_target(x);
        if lib::target_in_box(x, -8, 0x68, 8) || lib::target_in_box(x, -0xa0, 0x30, 8) {
            x.set8(SIDE, x.c.d[0] as u8);
            pick_move(x, NEXT_MOVES, 0x208);
            return;
        }
    }
    set_state_long(x, STANCE);
}

/// Choisit le coup `+6` dans la ligne de la difficulté (32 octets) de `table`, et pose `+4/+5` à `state`.
fn pick_move(x: &mut Cx, table: u32, state: u16) {
    let row = (var::RANK.get(x.b) >> 1) << 5;
    set_dw(x, 0, row);
    x.c.a[4] = table + row as u32;
    let n = rand_and(x, 0x1f);
    let mv = x.data8(table + row as u32 + n);
    x.set8(f::SUB2, mv);
    x.set16(f::STATE, state);
    x.set8(f::SUB3, 0);
}

/// Réaction aux drapeaux de coup `+0x8f` (0x59146) : vrai si elle a pris la main. L'original reçoit en A6
/// l'adresse où reprendre (`resume`) quand rien n'est à faire, et laisse A6, D6 et D1 comme ci-dessous.
fn special_hit(x: &mut Cx, resume: u32) -> bool {
    x.c.a[6] = resume;
    let flag = x.u8(SPECIAL_HIT);
    x.c.d[6] = flag as u32;
    if (1..=3).contains(&flag) {
        x.c.d[6] = 2 * flag as u32;
        let entry = x.rom16(0x5915a + 2 * flag as u32);
        set_dw(x, 1, entry);
    }
    match flag {
        1 => {
            if x.u16(Obj::Z) != 0 {
                return false;
            }
            x.set8(SPECIAL_HIT, 0);
            x.set8(0x49, 0x58);
            if x.u32(f::STATE) == 0x0404_0200 {
                x.set16(ANIM_STATE, 0xffff);
                set_state_long(x, 0x0406_0200);
            } else {
                set_state_long(x, 0x0400_0000);
            }
            true
        }
        2 => {
            set_state_long(x, 0x0400_0000);
            x.set8(SPECIAL_HIT, 0);
            let code = if x.u16(Obj::Z) == 0 { 0xc } else { 0x38 };
            x.set8(0x49, code);
            true
        }
        3 => {
            if x.u16(Obj::Z) != 0 || x.u8(0x86) != 0 {
                return false;
            }
            x.set8(SPECIAL_HIT, 0);
            set_state_long(x, KNOCKED);
            true
        }
        _ => false,
    }
}

// --- état 0 : création --------------------------------------------------------------------------

fn init(x: &mut Cx) {
    if paused(x) {
        return;
    }
    if x.u8(Obj::VARIANT) == 4 {
        let t = x.u16(0x42).wrapping_sub(1);
        x.set16(0x42, t);
        if t != 0 {
            return;
        }
    }
    x.add8(f::STATE, 2);
    x.set8(f::BOX_SET, 0xd);
    lib::initial_hp(x);
    for (off, v) in [(0x70, 0), (0x7e, 0x44), (0x7f, 0), (0x6a, 2), (0x6c, 3), (0x74, 3), (0x76, 3), (0x69, 2), (0x78, 0xff)] {
        x.set8(off, v);
    }
    x.set16(COUNTER, 0);
    x.set8(0x8e, 0);
    x.set8(0x81, 0);
    x.set8(0x80, 0);
    x.set32(0x8a, 0);
    x.set8(CAUGHT, 0);
    x.set8(0xac, 0);
    x.set8(0x83, 1);
    x.set32(f::IMMUNITY, 0x1001);
    lib::set_immunities(x);
    let series = x.u8(SERIES) as u32;
    x.c.d[1] = 4 * series;
    if series != 0 {
        x.set8(0x2a, 1);
        x.set8(0x2b, 0x1a);
    }
    // quatre octets par série : 0x6d, 0x72, 0x73, 0x68
    let row = 0x57de6 + 4 * series;
    for (i, off) in [0x6d, 0x72, 0x73, POSTURE].into_iter().enumerate() {
        let v = x.data8(row + i as u32);
        x.set8(off, v);
    }
    x.c.a[1] = row + 4;
    x.set16(f::SUB2, 0);
    match x.u8(Obj::VARIANT) {
        0 => {
            x.set8(f::SUB, 2);
            body(x, 0);
        }
        1 | 4 => {
            if series != 0 {
                let v = x.u8(0x2b) | 0x80;
                x.set8(0x91, v);
            }
            x.set8(f::SUB, 0);
            x.set8(0x2a, 0);
            body(x, 5 + series);
        }
        2 => {
            x.set8(f::SUB, 0);
            x.set8(f::SUB2, 4);
            let cam = if var::CAMERA_SELECT.get(x.b) != 0 { var::CAMERA_ALT_Y.get(x.b) } else { var::SCROLL_Y.get(x.b) };
            let h = cam.wrapping_add(0x100).wrapping_sub(x.u16(Obj::Y));
            x.set16(Obj::Z, h);
            x.set16(Obj::VX, 0);
            x.set16(Obj::AX, 0);
            x.set16(Obj::VZ, 0);
            x.set16(Obj::AZ, -0x40i16 as u16);
            body(x, 0xb);
        }
        3 => {
            x.set8(f::SUB, 0);
            body(x, 10);
        }
        5 => {
            x.set8(0, 2);
            x.set8(f::SUB, 0);
            x.set8(f::SUB2, 6);
            body(x, 0);
        }
        _ => return,
    }
    x.reinsert();
    zone_and_terrain(x);
}

// --- état 2 : actif -----------------------------------------------------------------------------

fn active(x: &mut Cx) {
    let freeze = x.u8(f::FREEZE) as i8;
    if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
    } else if !paused(x) {
        match x.u8(f::SUB) {
            0 => arrival(x),
            2 => stance::run(x),
            4 => attacks::run(x),
            6 => leaps::run(x),
            8 => leaps::run_dash(x),
            10 => leaps::run_land(x),
            _ => {}
        }
    }
    x.reinsert();
}

/// Sous-état 0 : arrivée (`+6` : 0 et 2 attente de l'animation, 4 chute, 6 clignotement).
fn arrival(x: &mut Cx) {
    match x.u8(f::SUB2) {
        0 | 2 => {
            step(x);
            if (x.u16(ANIM_STATE) as i16) >= 0 {
                return;
            }
            x.set8(f::SUB, 2);
            x.set16(f::SUB2, 0);
            if x.u8(SERIES) != 0 {
                x.set8(0x91, 0);
                x.set8(0x2a, 1);
                body(x, 0);
            }
        }
        4 => {
            if x.u8(f::SUB3) == 0 {
                x.move_xz();
                if (x.u16(Obj::Z) as i16) > 0 {
                    return;
                }
                x.add8(f::SUB3, 2);
                x.set32(Obj::Z, 0);
                x.set16(ANIM_COUNT, 1);
            } else {
                step(x);
                if (x.u16(ANIM_STATE) as i16) >= 0 {
                    return;
                }
                x.set8(f::SUB, 2);
                x.set16(f::SUB2, 0);
            }
        }
        6 => blink(x),
        _ => {}
    }
}

/// Arrivée clignotante (`+6 = 6`) : trois séries de clignotements puis l'état debout.
fn blink(x: &mut Cx) {
    let d1 = match x.u8(f::SUB3) {
        0 => {
            x.add8(f::SUB3, 2);
            x.set8(0, 2);
            x.set8(0xac, 1);
            x.set8(VULNERABLE, 1);
            x.set16(0x42, 0x703);
            let d7 = x.c.d[7] as u8;
            x.set8(TIMER, d7);
            return;
        }
        2 => 0x307,
        4 | 6 => 0x110,
        8 => {
            set_state_long(x, STANCE);
            x.set8(0, 1);
            return;
        }
        _ => return,
    };
    x.set_bit(1, 1, true);
    let t = x.u8(TIMER).wrapping_sub(1);
    x.set8(TIMER, t);
    if t & x.u8(0x42) != 0 {
        return;
    }
    x.set_bit(1, 1, false);
    let n = x.u8(0x43).wrapping_sub(1);
    x.set8(0x43, n);
    if n != 0 {
        return;
    }
    x.add8(f::SUB3, 2);
    x.set16(0x42, d1);
}

// --- état 6 : libération ------------------------------------------------------------------------

fn dead(x: &mut Cx) {
    if x.u8(0xac) != 0 {
        x.c.a[1] = x.u16(0xaa) as i16 as i32 as u32;
        let parent = x.linked(0xaa);
        let n = parent.u8(x.b, 0xaf).wrapping_sub(1);
        parent.set_u8(x.b, 0xaf, n);
    }
    free(x);
}

handlers! {
    0x57cc4 "enemy_1" run,
}
