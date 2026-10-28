//! Ennemi « Ghoul » (ENEMY type 5, 0x5f792). Voir `docs/familles/enemy.md`.
//!
//! Charpente commune (`active_loop` : gel, sous-état, zone et ombre, remise en liste). En état actif :
//!
//! | `+5` | Fichier | Rôle (h) |
//! |---|---|---|
//! | 0 | `mod` | arrivée selon la variante `+3` |
//! | 2 | `stance` | debout : marche, demi-tour, attente, choix d'un coup |
//! | 4 | `attacks` | coups et bonds (`+6` = le coup) |
//! | 6 | `moves` | déplacements de repli, pas de côté, sortie d'écran |
//! | 8 | `moves` | demi-tours et reprise |
//! | 10 | `stance` | patrouille tant qu'aucun joueur n'est en vie |
//! | état 4 | `hit` | réactions aux coups |
//!
//! Animations : 0x16191c (mouvements) et 0x161930 (coups). `+0xa0/+0xa1` sont des octets d'aspect.
//! Les sous-états 2, 4, 6 et 8 commencent par la réaction aux drapeaux de coup `+0x8f` (0x60c88).

use crate::families::cx::Banc;
use super::cx::{handlers, Cx};
use super::enemy_common::*;
use crate::objects::Obj;

mod attacks;
mod hit;
mod moves;
mod stance;

pub(self) mod g {
    pub const STRIKE_KIND: u32 = 0x44;
    pub const VULNERABLE: u32 = 0x67;
    pub const AI_CHOICE: u32 = 0x6b;
    pub const AI_WAIT: u32 = 0x6e;
    pub const AI_WALK: u32 = 0x71;
    pub const AI_SHOT: u32 = 0x79;
    pub const SPECIAL_HIT: u32 = 0x8f;
    /// Aspect courant et précédent.
    pub const LOOK: u32 = 0xa0;
    pub const LOOK_PREV: u32 = 0xa1;
    /// Minuterie d'une étape.
    pub const TIMER: u32 = 0xa2;
    /// Position mémorisée (fuite).
    pub const SAVED_X: u32 = 0xa8;
}

use g::*;
use crate::objects::Courant;

const SET_BODY: u32 = 0x16191c;
const SET_STRIKE: u32 = 0x161930;
const STANCE: u32 = 0x0202_0000;
const IDLE: u32 = 0x020a_0000;
const KNOCKED: u32 = 0x0408_0000;
/// Tables de la ROM : coups à choisir par niveau de difficulté (`A5+0x7f92`), 32 octets par niveau.
const CLOSE_ATTACKS: u32 = 0x60d38;
const FAR_ATTACKS: u32 = 0x60f38;
const FALLBACK_MOVES: u32 = 0x61164;

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

/// L'aspect voulu (`LOOK` et `LOOK_PREV` identiques).
fn look<R: Banc>(x: &mut Cx<'_, R>, v: u8) {
    x.set8(LOOK, v);
    x.set8(LOOK_PREV, v);
}

/// Direction de marche selon l'orientation : `flipped` si `+0x2c` est non nul, `otherwise` sinon.
fn set_angle<R: Banc>(x: &mut Cx<'_, R>, flipped: u8, otherwise: u8) {
    let angle = if x.u8(f::FLIP) != 0 { flipped } else { otherwise };
    x.set8(Obj::SPEED_INDEX, angle);
}

/// Table de vitesses 0xd puis marche : `ANGLE` puis 0x1b64 avec D0 = `speed`.
fn set_speed<R: Banc>(x: &mut Cx<'_, R>, speed: u32) {
    x.select_speed(&crate::objects::motion::SPEEDS_B, speed as u16);
}

/// Ligne de 32 octets de la difficulté (`A5+0x7f92`).
fn difficulty_row<R: Banc>(x: &mut Cx<'_, R>) -> u32 {
    ((var::RANK.get(x.b) >> 1) << 5) as u32
}

/// Octet tiré au hasard dans la ligne de la difficulté de `table` (A4 = ligne, D0 = indice).
fn pick_byte(x: &mut Cx, table: u32) -> u8 {
    let row = difficulty_row(x);
    set_dw(x, 0, row as u16);
    let n = rand_and(x, 0x1f);
    x.data8(table + row + n)
}

/// 0x61138 : fin d'un coup, le suivant est tiré dans la ligne de la difficulté.
fn end_attack(x: &mut Cx) {
    let mv = pick_byte(x, FALLBACK_MOVES);
    x.set8(f::SUB, 8);
    x.set8(f::SUB2, mv);
    x.set8(f::SUB3, 0);
}

/// 0x60d04 / 0x60d0c : choisit le coup de l'attaque (`+6` = octet de la table + `bias`).
fn pick_attack(x: &mut Cx, table: u32, bias: u8) {
    let mv = pick_byte(x, table);
    x.set8(f::SUB2, mv.wrapping_add(bias));
    x.set8(f::SUB3, 0);
}

/// 0x5fdf8 : la cible est de l'autre côté de l'orientation (D0 = 1 ou 0).
fn needs_turn_around(x: &mut Cx) -> bool {
    if x.u16(f::TARGET) == 0 {
        return false;
    }
    let target = load_target(x);
    let dx = x.u16(Obj::X).wrapping_sub(target.u16(x.b, Obj::X));
    let want = if (dx as i16) < 0 { 0x20 } else { 0 };
    let turn = want != x.u8(f::FLIP) as u32;
    turn
}

/// Décision par masque (0xa4c2 de même orientation, 0xa4b4 sinon) pour la cible courante.
fn decision(x: &mut Cx) -> bool {
    let target = load_target(x);
    let theirs = target.u8(x.b, f::FLIP);
    x.c.d[0] = x.c.d[0] & !0xff | theirs as u32;
    let same = theirs == x.u8(f::FLIP);
    lib::decision(x, same)
}

/// 0x5fe22 / 0x5fe70 : un joueur dans la boîte et la décision l'accepte ; l'attaque est alors choisie.
fn box_attack(x: &mut Cx, d0: i16, d1: u16, d2: u16, table: u32, bias: u8) -> bool {
    let ok = lib::target_in_box(x, d0, d1, d2) && decision(x);
    if ok {
        set_state_long(x, 0x0204_0000);
        pick_attack(x, table, bias);
    }
    ok
}

/// Réaction aux drapeaux de coup `+0x8f` (0x60c88) : vrai si elle a pris la main.
fn special_hit(x: &mut Cx) -> bool {
    let flag = x.u8(SPECIAL_HIT);
    match flag {
        1 => {
            if x.u16(Obj::Z) != 0 {
                return false;
            }
            x.set8(SPECIAL_HIT, 0);
            set_state_long(x, 0x0400_0000);
            x.set8(0x49, 0x58);
            hit::run(x);
            true
        }
        2 => {
            set_state_long(x, 0x0400_0000);
            x.set8(SPECIAL_HIT, 0);
            let code = if x.u16(Obj::Z) == 0 { 0xc } else { 0x38 };
            x.set8(0x49, code);
            hit::run(x);
            true
        }
        3 => {
            if x.u16(Obj::Z) != 0 || x.u8(0x86) != 0 {
                return false;
            }
            x.set8(SPECIAL_HIT, 0);
            set_state_long(x, KNOCKED);
            moves::run_b(x);
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
    x.add8(f::STATE, 2);
    x.set8(0, 2);
    x.set8(f::BOX_SET, 0x11);
    lib::initial_hp(x);
    lib::set_immunities(x);
    for (off, v) in [
        (0x68, 2),
        (0x69, 2),
        (0x70, 0),
        (0x6a, 3),
        (0x6c, 3),
        (0x6d, 3),
        (0x72, 3),
        (0x73, 2),
        (0x74, 3),
        (0x75, 4),
        (0x76, 3),
        (TIMER, 0x20),
        (LOOK, 6),
        (VULNERABLE, 1),
        (0x8e, 0),
        (0x81, 8),
        (0x80, 9),
        (0xa6, 0),
        (0xa7, 0),
        (0x83, 1),
        (0x7f, 0x44),
        (0x78, 0xff),
    ] {
        x.set8(off, v);
    }
    // la création enchaîne directement sur l'état actif
    active(x);
}

// --- état 2 : actif -----------------------------------------------------------------------------

fn active(x: &mut Cx) {
    active_loop(x, |x| {
        x.c.d[0] = x.u8(f::SUB) as u32;
        match x.u8(f::SUB) {
            0 => arrival(x),
            2 => stance::run(x),
            4 => attacks::run(x),
            6 => moves::run_b(x),
            8 => moves::run_c(x),
            10 => stance::patrol(x),
            _ => {}
        }
        false
    });
}

/// Sous-état 0 : arrivée selon la variante `+3` (0 marche, 1 saut, 2 chute, 3 réveil, 4 apparition, 5 clignotement).
fn arrival(x: &mut Cx) {
    match x.u8(Obj::VARIANT) {
        0 => {
            if x.u8(f::SUB2) != 0 {
                let t = x.u8(TIMER).wrapping_sub(1);
                x.set8(TIMER, t);
                if t == 0 {
                    set_state_long(x, STANCE);
                    x.set8(0, 1);
                }
                move_xy(x);
                step(x);
                return;
            }
            x.add8(f::SUB2, 2);
            x.set8(VULNERABLE, 1);
            look(x, 0xff);
            x.set8(TIMER, 0x18);
            set_angle(x, 8, 0x18);
            set_speed(x, 0xd);
            body(x, 2);
        }
        1 => match x.u8(f::SUB2) {
            0 => {
                x.add8(f::SUB2, 2);
                x.set8(VULNERABLE, 1);
                look(x, 0xff);
                x.set8(TIMER, 4);
                body(x, 3);
            }
            2 => {
                let t = x.u8(TIMER).wrapping_sub(1);
                x.set8(TIMER, t);
                if t != 0 {
                    return;
                }
                x.add8(f::SUB2, 2);
                x.set16(Obj::VX, -0x500i16 as u16);
                if x.u8(f::FLIP) != 0 {
                    negate(x, Obj::VX);
                }
                x.set16(Obj::VZ, 0x380);
                x.set16(Obj::AZ, -0x38i16 as u16);
                x.set16(Obj::AX, 0);
                body(x, 4);
            }
            4 => {
                if x.move_xz().gt() {
                    return;
                }
                x.add8(f::SUB2, 2);
                x.set32(Obj::Z, 0);
                body(x, 5);
            }
            6 => end_arrival(x),
            _ => {}
        },
        2 => match x.u8(f::SUB2) {
            0 => {
                x.add8(f::SUB2, 2);
                x.set8(VULNERABLE, 1);
                look(x, 0xff);
                x.set16(Obj::VZ, -0x300i16 as u16);
                x.set16(Obj::AZ, -0x80i16 as u16);
                let h = var::SCROLL_Y.get(x.b).wrapping_add(0x100).wrapping_sub(x.u16(Obj::Y));
                x.set16(Obj::Z, h);
                body(x, 4);
            }
            2 => {
                x.move_z();
                if (x.u16(Obj::Z) as i16) >= 0 {
                    return;
                }
                x.add8(f::SUB2, 2);
                x.set32(Obj::Z, 0);
                body(x, 5);
            }
            4 => end_arrival(x),
            _ => {}
        },
        3 => {
            if special_hit(x) {
                return;
            }
            match x.u8(f::SUB2) {
                0 => {
                    x.add8(f::SUB2, 2);
                    x.set8(VULNERABLE, 1);
                    look(x, 0xff);
                    body(x, 5);
                }
                2 => {
                    // un joueur assez près réveille le ghoul
                    let mut near = 0;
                    for k in 0..4u32 {
                        let p = 0xff85de + 0x200 * k;
                        let dx = x.u16(Obj::X).wrapping_sub(crate::mem::rd16(x.b, p + Obj::X)).wrapping_add(0x60);
                        let dy = x.u16(Obj::Y).wrapping_sub(crate::mem::rd16(x.b, p + Obj::Y)).wrapping_add(0x30);
                        if dx <= 0xc0 && dy <= 0x60 {
                            near += 1;
                        }
                    }
                    if near != 0 {
                        x.add8(f::SUB2, 2);
                    }
                }
                4 => end_arrival(x),
                _ => {}
            }
        }
        4 => {
            if x.u8(f::SUB2) == 0 {
                x.add8(f::SUB2, 2);
                x.set8(VULNERABLE, 1);
                look(x, 0xff);
                body(x, 8);
            } else if step(x) {
                set_state_long(x, STANCE);
                x.set8(0, 1);
            }
            zone_and_terrain(x);
        }
        5 => match x.u8(f::SUB2) {
            0 => {
                x.add8(f::SUB2, 2);
                x.set8(0x81, 0);
                x.set8(0x80, 0);
                x.set8(0xa6, 1);
                x.set8(VULNERABLE, 1);
                look(x, 0xff);
                x.set16(0x42, 0x703);
                let d7 = x.c.d[7] as u8;
                x.set8(TIMER, d7);
                body(x, 0);
            }
            2 => blink(x, 0x307),
            4 | 6 => blink(x, 0x110),
            8 => {
                set_state_long(x, STANCE);
                x.set8(0, 1);
            }
            _ => {}
        },
        _ => {}
    }
}

/// 0x5fa56 : l'animation finie, le ghoul se met debout.
fn end_arrival<R: Banc>(x: &mut Cx<'_, R>) {
    if step(x) {
        set_state_long(x, STANCE);
        x.set8(0, 1);
    }
}

/// Clignotement d'arrivée (0x5fbbe), `next` : valeur du mot `+0x42` quand une série est finie.
fn blink<R: Banc>(x: &mut Cx<'_, R>, next: u16) {
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
    x.add8(f::SUB2, 2);
    x.set16(0x42, next);
}

// --- état 6 : libération ------------------------------------------------------------------------

fn dead<R: Banc>(x: &mut Cx<'_, R>) {
    if x.u8(0xa6) != 0 {
        let parent = x.linked(0xa4);
        let n = parent.u8(x.b, 0xaf).wrapping_sub(1);
        parent.set_u8(x.b, 0xaf, n);
    }
    free(x);
}

handlers! {
    0x5f792 "enemy_5" run,
}
