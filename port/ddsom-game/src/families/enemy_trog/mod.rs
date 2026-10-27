//! Ennemi « Troglodyte » (ENEMY type 2, 0x59baa). Voir `docs/familles/enemy.md`.
//!
//! Charpente commune : `+4` état (0 création, 2 actif, 4 touché, 6 libération), `+5` groupe de
//! comportements, `+6` et `+7` étapes. En état actif :
//!
//! | `+5` | Fichier | Rôle (h) |
//! |---|---|---|
//! | 0 | `mod` | arrivée selon la variante `+3` |
//! | 2 | `stance` | debout : marche, demi-tour, attente, choix d'un coup |
//! | 4 | `attacks` | coups au corps à corps et bonds (`+6` = le coup) |
//! | 6 | `attacks` | bonds d'esquive et de garde |
//! | 8 | `attacks` | coups de repli |
//! | 10 | `stance` | repos : attend qu'un joueur soit en vie |
//! | état 4 | `hit` | réactions aux coups |
//!
//! Animations : 0x15faa0 (mouvements) et 0x15fabe (coups). La série 0 (`+0x2f`) clignote après un
//! coup encaissé : `flicker` (compteurs `+0xa5..+0xa8`, `+0xaa`, `+0xae`).

use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

mod attacks;
mod hit;
mod stance;

/// Champs propres à ce type (les champs communs sont dans `enemy_common::f`).
pub(self) mod g {
    /// Série : 0 troglodyte (clignote), 1 variante armée.
    pub const SERIES: u32 = 0x2f;
    pub const STRIKE_KIND: u32 = 0x44;
    pub const STRIKE_POWER: u32 = 0x45;
    pub const VULNERABLE: u32 = 0x67;
    pub const POSTURE: u32 = 0x68;
    /// Compteurs d'IA : délai avant un nouveau choix, attente, marche.
    pub const AI_CHOICE: u32 = 0x6b;
    pub const AI_WAIT: u32 = 0x6e;
    pub const AI_WALK: u32 = 0x71;
    pub const COOLDOWN: u32 = 0x77;
    pub const AI_KIND: u32 = 0x78;
    pub const AI_SHOT: u32 = 0x79;
    /// Aspect courant et précédent (octet de teinte lu par l'affichage).
    pub const LOOK: u32 = 0xa0;
    pub const LOOK_PREV: u32 = 0xa1;
    /// Répétitions ou minuterie d'une étape.
    pub const REPEAT: u32 = 0xa2;
    /// Coup à reprendre après une annonce ou un demi-tour.
    pub const RESUME: u32 = 0xa3;
    /// Clignotement : phase (`+0xa5`, 0xff = au repos), alternance, attente, compteurs.
    pub const BLINK_PHASE: u32 = 0xa5;
    pub const BLINK_ON: u32 = 0xa6;
    pub const BLINK_WAIT: u32 = 0xa7;
    pub const BLINK_COUNT: u32 = 0xa8;
    pub const BLINK_DELAY: u32 = 0xaa;
    pub const BLINK_REST: u32 = 0xae;
    /// Octet non nul : l'état touché ne dessine pas l'ombre.
    pub const NO_SHADOW: u32 = 0xb0;
}

use g::*;
use crate::objects::Courant;

/// Jeux d'animations : mouvements et coups.
const SET_BODY: u32 = 0x15faa0;
const SET_STRIKE: u32 = 0x15fabe;
/// États longs (`+4..+7`).
const STANCE: u32 = 0x0202_0000;
const IDLE: u32 = 0x020a_0000;
/// Tables de la ROM : coups à choisir par niveau de difficulté (`A5+0x7f92`), 32 octets par niveau.
const FALLBACK_MOVES: u32 = 0x5bba6;
const CLOSE_ATTACKS: u32 = 0x5b074;
const CLOSE_ATTACKS_ARMED: u32 = 0x5b274;
const REPEATS_TABLE: u32 = 0x5b4d6;
const JUMP_MOVES: u32 = 0x5b6f8;
const BLINK_DELAYS: u32 = 0x5b974;
const BLINK_TIMES: u32 = 0x5b96c;
/// Vitesses de marche par série (2 mots par série).
const WALK_SPEEDS: u32 = 0x5af64;

mod call {
    pub const FACE_TARGET: u32 = 0xa470;
    pub const TURN_TO_TARGET: u32 = 0x788c;
    pub const RAND_TARGET_CHECK: u32 = 0x79ce;
    pub const ANNOUNCE: u32 = 0xac3a;
    pub const WALK_STEP: u32 = 0xe76c;
    pub const REMOVE: u32 = 0x27ee;
    pub const BEHIND_CHECK: u32 = 0x57c66;
    pub const PICK_ATTACK_KIND: u32 = 0x5cdc6;
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

/// Direction de marche selon l'orientation : `flipped` si `+0x2c` est non nul, `otherwise` sinon.
fn set_angle(x: &mut Cx, flipped: u8, otherwise: u8) {
    let angle = if x.u8(f::FLIP) != 0 { flipped } else { otherwise };
    x.set8(Obj::SPEED_INDEX, angle);
}

fn move_xy(x: &mut Cx) {
    x.move_by_table_xy();
}

/// Ligne de 32 octets de la difficulté dans `table` (`A5+0x7f92`).
fn difficulty_row(x: &mut Cx) -> u32 {
    (((x.a5_u16(0x7f92) >> 1) << 5) as u32) & 0xffff
}

/// Octet tiré au hasard dans la ligne de la difficulté de `table` (A4 = ligne, D0 = indice).
fn pick_byte(x: &mut Cx, table: u32) -> u8 {
    let row = difficulty_row(x);
    x.c.a[4] = table + row;
    let n = rand_and(x, 0x1f);
    x.data8(table + row + n)
}

/// 0x5af4e : choisit la table de vitesses selon la série (`idx` : 0, 2 ou 4).
fn walk_speed(x: &mut Cx, idx: u32) {
    let series = x.u8(SERIES) as u32;
    x.c.d[1] = (series << 2) & 0xff;
    x.c.a[1] = WALK_SPEEDS + x.c.d[1];
    let n = (x.data16(x.c.a[1] + idx) as u32) as u16;
    x.select_speed(&crate::objects::motion::SPEEDS_B, n);
}

/// Un joueur dans la boîte ? Les deux décisions par masque (0xa4c2, 0xa4b4) doivent accepter.
fn box_attack(x: &mut Cx, d0: i16, d1: u16, d2: u16) -> bool {
    let ok = target_in_box(x, d0, d1, d2) && {
        load_target(x);
        x.call(0xa4c2);
        !x.c.cond(7)
    } && {
        x.call(0xa4b4);
        !x.c.cond(7)
    };
    x.c.d[0] = ok as u32;
    ok
}

/// 0x5afa0 : cible devant, à portée courte.
fn front_box(x: &mut Cx) -> bool {
    x.c.d[0] = 0;
    box_attack(x, 0, 0x70, 8)
}

/// 0x5af90 : cible derrière ou loin devant.
fn side_box(x: &mut Cx) -> bool {
    box_attack(x, -0xb5, 0x35, 0x15)
}

/// 0x5afda : la cible est de l'autre côté de l'orientation.
fn needs_turn_around(x: &mut Cx) -> bool {
    if x.u16(f::TARGET) == 0 {
        x.c.d[0] = 0;
        return false;
    }
    let target = load_target(x);
    let dx = x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X));
    set_dw(x, 0, dx);
    let want = if (dx as i16) < 0 { 0x20 } else { 0 };
    x.c.d[1] = want;
    let turn = want != x.u8(f::FLIP) as u32;
    x.c.d[0] = turn as u32;
    turn
}

/// 0x5af78 : retour debout ; si la décision 0x79ce le veut, au repos.
fn back_to_stance(x: &mut Cx) {
    set_state_long(x, STANCE);
    x.call(call::RAND_TARGET_CHECK);
    if !x.c.cond(7) {
        x.set8(f::SUB, 0xa);
    }
}

/// Fin des coups : le suivant est tiré dans la ligne de la difficulté (0x5bb74).
fn end_attack(x: &mut Cx) {
    x.set8(AI_KIND, 0xff);
    let row = difficulty_row(x);
    set_dw(x, 0, row as u16);
    x.c.a[4] = FALLBACK_MOVES + row;
    let n = rand_and(x, 0x1f);
    x.set8(f::SUB, 8);
    let mv = x.data8(FALLBACK_MOVES + row + n);
    x.set8(f::SUB2, mv);
    x.set8(f::SUB3, 0);
}

/// Le clignotement est lancé s'il ne l'est pas déjà (série 0, phase positive).
fn blink_if_running(x: &mut Cx) {
    if x.u8(SERIES) == 0 && (x.u8(BLINK_PHASE) as i8) > 0 {
        x.set8(BLINK_ON, 1);
        x.set16(BLINK_COUNT, 0xc);
        x.set8(BLINK_WAIT, 0);
    }
}

// --- état 0 : création --------------------------------------------------------------------------

fn init(x: &mut Cx) {
    if paused(x) {
        return;
    }
    x.add8(f::STATE, 2);
    x.set8(f::BOX_SET, 0xe);
    x.call(rom::INITIAL_HP);
    x.call(rom::IMMUNITIES);
    x.c.d[0] = 0;
    for (off, v) in [(0x80, 0xb), (0x68, 4), (0x69, 4), (0x70, 0), (0x6a, 3), (0x6c, 3), (0x6d, 5), (0x72, 4), (0x73, 3), (0x74, 3), (0x75, 0x2c), (BLINK_PHASE, 0xff)] {
        x.set8(off, v);
    }
    x.set16(BLINK_REST, 0x258);
    x.set8(BLINK_ON, 0);
    if x.u8(SERIES) != 0 {
        x.add8(0x75, 4);
        x.set8(BLINK_PHASE, 0xff);
    }
    for (off, v) in [(0x76, 4), (REPEAT, 0x20), (LOOK_PREV, 2), (VULNERABLE, 1), (0x8e, 0), (0x7f, 0), (0x7e, 0xf)] {
        x.set8(off, v);
    }
    x.set32(0x8a, 0);
    x.set8(AI_KIND, 0xff);
    x.set8(0x81, 0xa);
    x.set8(AI_SHOT, 0x14);
    if pick_target(x) {
        // la création enchaîne directement sur l'état actif
        active(x);
    } else {
        set_state_long(x, IDLE);
    }
}

// --- état 2 : actif -----------------------------------------------------------------------------

fn active(x: &mut Cx) {
    let freeze = x.u8(f::FREEZE) as i8;
    let dispatch = if freeze > 0 {
        x.set8(f::FREEZE, (freeze - 1) as u8);
        true
    } else {
        // le jeu en pause empêche le comportement (mais pas l'ombre)
        !paused(x)
    };
    if !dispatch {
        if x.u8(f::SUB) != 0 {
            zone_shadow(x);
        }
        return x.reinsert();
    }
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
    if x.u8(f::SUB) != 0 {
        zone_shadow(x);
    }
    if x.u8(SERIES) == 0 {
        flicker(x);
    }
    x.reinsert();
}

/// Clignotement de la série 0 (0x59d02) : `+0xa5` est la phase (0xff : au repos, décompte `+0xae` ; 0 : lance le
/// clignotement ; positive : alterne la visibilité `+1` bit 1).
fn flicker(x: &mut Cx) {
    let phase = x.u8(BLINK_PHASE) as i8;
    if phase == 0 {
        x.add8(BLINK_PHASE, 1);
        x.set8(BLINK_WAIT, 1);
        x.set8(BLINK_ON, 1);
        x.set16(BLINK_REST, 0x258);
        x.set16(BLINK_COUNT, 0xc);
        return pick_blink_delay(x);
    }
    if phase < 0 {
        let n = x.u16(BLINK_REST).wrapping_sub(1);
        x.set16(BLINK_REST, n);
        return;
    }
    if x.u8(BLINK_ON) != 0 {
        let v = x.u8(1) ^ 2;
        x.set8(1, v);
    } else if x.u8(BLINK_WAIT) != 0 {
        x.set_bit(1, 1, true);
        let n = x.u16(BLINK_DELAY).wrapping_sub(1);
        x.set16(BLINK_DELAY, n);
        if (n as i16) < 0 {
            x.set8(BLINK_WAIT, 0);
            x.set8(BLINK_ON, 1);
            x.set16(BLINK_COUNT, 0xc);
            x.set_bit(1, 1, false);
        }
    } else {
        x.set8(BLINK_PHASE, 0xff);
        x.set_bit(1, 1, false);
    }
}

/// 0x5b940 : durée d'invisibilité tirée au hasard.
fn pick_blink_delay(x: &mut Cx) {
    x.set8(BLINK_WAIT, 1);
    let mut d2 = x.a5_u16(0x7f92);
    d2 = d2 & 0xff00 | (d2 as u8 >> 1) as u16;
    let row = (d2 << 5) as u32;
    x.c.d[2] = x.c.d[2] & 0xffff_0000 | row;
    x.c.a[4] = BLINK_DELAYS + row;
    let n = rand_and(x, 0x1f);
    let idx = x.data8(BLINK_DELAYS + row + n) as u32;
    set_dw(x, 0, idx as u16);
    let w = x.rom16(BLINK_TIMES + idx);
    x.set16(BLINK_DELAY, w);
}

/// Sous-état 0 : arrivée selon la variante `+3` (0 marche, 1 chute, 2 bond, 3 et 4 saut, 5 réveil).
fn arrival(x: &mut Cx) {
    match x.u8(Obj::VARIANT) {
        0 => {
            if x.u8(f::SUB2) != 0 {
                let t = x.u8(REPEAT).wrapping_sub(1);
                x.set8(REPEAT, t);
                if t == 0 {
                    x.set8(LOOK_PREV, 0);
                    set_state_long(x, STANCE);
                    x.set8(0, 1);
                }
                move_xy(x);
                step(x);
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(VULNERABLE, 1);
            x.set16(LOOK, 0);
            x.set8(REPEAT, 0x18);
            set_angle(x, 8, 0x18);
            x.set16(LOOK, 0);
            x.c.d[0] = 0;
            walk_speed(x, 0);
            body(x, 2);
        }
        1 => match x.u8(f::SUB2) {
            0 => {
                x.add8(f::SUB2, 2);
                x.set8(VULNERABLE, 1);
                x.set8(LOOK_PREV, 0xe);
                x.set16(Obj::VZ, -0x300i16 as u16);
                x.set16(Obj::AZ, -0x80i16 as u16);
                let cam = x.a5_u16(0x50a).wrapping_add(0x100).wrapping_sub(x.u16(Obj::Y));
                x.set16(Obj::Z, cam);
                body(x, 0xb);
            }
            2 => {
                x.move_z();
                if (x.u16(Obj::Z) as i16) < 0 {
                    x.add8(f::SUB2, 2);
                    x.set32(Obj::Z, 0);
                    body(x, 0xc);
                }
            }
            4 => end_arrival(x),
            _ => {}
        },
        2 => match x.u8(f::SUB2) {
            0 => {
                x.add8(f::SUB2, 2);
                x.set16(Obj::VX, 0);
                let t = load_target(x);
                if t.u8(x.b, 0) == 0 {
                    set_state_long(x, IDLE);
                    return rest_begin(x);
                }
                x.set16(Obj::VZ, 0x680);
                x.set16(Obj::AZ, -0x38i16 as u16);
                x.set16(Obj::AX, 0);
                let d = t.u16(x.b, Obj::X).wrapping_sub(x.u16(Obj::X)) << 2;
                x.set16(Obj::VX, d);
                body(x, 4);
            }
            2 => {
                x.move_xz();
                if step(x) && (x.u16(Obj::Z) as i16) < 0 {
                    x.set16(Obj::Z, 0);
                    x.add8(f::SUB2, 2);
                    body(x, 0xc);
                }
            }
            4 => end_arrival(x),
            _ => {}
        },
        3 | 4 => {
            if x.u8(f::SUB2) == 0 {
                let scene = x.a5_u8(0x34);
                if scene == 6 || scene == 0xc {
                    x.call(0xeb92e);
                } else {
                    x.call(0xeb9a0);
                }
                x.add8(f::SUB2, 2);
                x.set16(Obj::VZ, 0x600);
                x.set16(Obj::AZ, -0x38i16 as u16);
                x.set16(Obj::VY, 0x80);
                x.set16(Obj::AY, 0);
                if x.u8(Obj::VARIANT) != 3 {
                    negate(x, Obj::VY);
                }
                body(x, 0xa);
            } else {
                step(x);
                if x.move_yz().pl() {
                    return;
                }
                x.set8(LOOK_PREV, 0xe0);
                x.set16(Obj::Z, 0);
                set_state_long(x, STANCE);
            }
        }
        5 => {
            if x.u8(f::SUB2) != 0 {
                end_arrival(x)
            } else {
                x.add8(f::SUB2, 2);
                body(x, 9);
            }
        }
        _ => {}
    }
}

/// 0x59f38 : l'animation d'arrivée finie, le troglodyte se met debout.
fn end_arrival(x: &mut Cx) {
    if step(x) {
        x.set8(LOOK_PREV, 0xe0);
        set_state_long(x, STANCE);
        x.set8(0, 1);
    }
}

/// 0x5a010 : aspect de repos et animation 0xe.
fn rest_begin(x: &mut Cx) {
    x.add8(f::SUB2, 2);
    x.set8(LOOK_PREV, 1);
    body(x, 0xe);
}

// --- état 6 : libération ------------------------------------------------------------------------

fn dead(x: &mut Cx) {
    set_dw(x, 1, 0x12);
    let w = x.u16(0x48) & 0xff00;
    set_dw(x, 0, w.wrapping_add(0x12));
    free(x);
}

handlers! {
    0x59baa "enemy_2" run,
}
