//! Boss de type 8 (0x87fc0), scène 64 : le deuxième guerrier noir (« DarkWarrior 2 »). Voir `docs/familles/boss.md`.
//!
//! Même charpente que les boss 17, 18 et 21 (`+4` état, `+5` groupe, `+6` et `+7` étapes, états posés d'un coup en
//! longs, tirages dans des tables de la ROM) mais ses réactions aux coups sont les routines communes des ennemis
//! (table de longs 0x89330) et il passe en « ralenti » quand il est affaibli.
//!
//! | `+5` | Fichier | Rôle |
//! |---|---|---|
//! | 0 | `fight` | cinématique d'arrivée (délais, saut, effets) |
//! | 2 | `fight` | combat : marche (`+6` = 0, 8), demi-tour (2), arrêt (4), approche (6) |
//! | 4 | `attacks` | dix attaques (`+6` = 0 à 18) et choix de la suivante |
//! | 6, 8 | `moves` | esquives (pas de côté, saut, reculade) |
//! | 10, 12, 14 | ici | attente sans joueur, debout, relevé |
//! | état 4 | `hit` | réactions aux coups, mort |
//!
//! Animations : 0x1790f6 (mouvements), 0x179116 (coups).

use super::boss_common::*;
use super::cx::handlers;
use crate::mem::A5;

mod aids;
mod attacks;
mod fight;
mod hit;
mod moves;

pub(self) mod g {
    pub const REACTION: u32 = 0x49;
    pub const STUN: u32 = 0x56;
    pub const VULNERABLE: u32 = 0x67;
    pub const SHOT: u32 = 0x79;
    pub const HITS: u32 = 0x84;
    pub const TIMER: u32 = 0xa0;
    pub const ANIM_KIND: u32 = 0xa2;
    pub const ANIM_LAST: u32 = 0xa3;
    /// Sous-état et état à reprendre (`+0xa5`, long `+0xa6`).
    pub const RESUME_SUB: u32 = 0xa5;
    pub const RESUME_STATE: u32 = 0xa6;
    pub const GRABBED: u32 = 0xaa;
    pub const PHASE_TIMER: u32 = 0xab;
    pub const VOLLEY: u32 = 0xac;
    pub const PHASES: u32 = 0xad;
    pub const HIT_ACK: u32 = 0xae;
    pub const PICKED: u32 = 0xaf;
    pub const SPAWN_TIME: u32 = 0xb4;
    pub const WEAK_HP: u32 = 0xb6;
    pub const SPAWNED: u32 = 0xb8;
    /// Ralenti : actif (`+0xb9`), pas (`+0xba`), compteur de la troisième attaque (`+0xbb`).
    pub const SLOW: u32 = 0xb9;
    pub const SLOW_TICK: u32 = 0xba;
    pub const THIRD: u32 = 0xbb;
    pub const IDLE_TIME: u32 = 0xbc;
    pub const IDLE_WAIT: u32 = 0xbe;
}
use g::*;
use crate::objects::pools;
use crate::objects::Courant;
use crate::machine::Machine;

pub(self) const SET_BODY: u32 = 0x1790f6;
pub(self) const SET_STRIKE: u32 = 0x179116;

pub(self) const WAIT: u32 = 0x020a_0000;

pub(self) fn body(x: &mut Cx, n: u32) {
    x.anim_start(SET_BODY, n as u16);
}

pub(self) fn strike(x: &mut Cx, n: u32) {
    x.anim_start(SET_STRIKE, n as u16);
}

pub(self) mod call {
    pub const SOUND_EVENT: u32 = 0x2824;
    pub const SOUND_B: u32 = 0x2860;
    pub const ASK_TARGET: u32 = 0xe76c;
    pub const BURST: u32 = 0xb10c8;
    pub const ATTACK_START: u32 = 0xac3a;
}

/// Mot de poids faible d'un registre de données (`move.w`, `andi.w`...) : le reste du registre est conservé.
pub(self) fn setw(x: &mut Cx, n: usize, v: u16) {
    set_dw(x, n, v);
}

/// Saut par table relative (`move.w (tbl,PC,D0w),D1w ; jmp (tbl,PC,D1w)`) indexée par l'octet `idx` de l'objet :
/// rend l'adresse d'arrivée. D0 reçoit l'octet (après un `moveq 0,D0`), D1w le décalage.
pub(self) fn dispatch(x: &mut Cx, table: u32, idx: u32) -> u32 {
    let i = x.u8(idx) as u32;
    x.c.d[0] = i;
    let off = x.rom16(table + i);
    setw(x, 1, off);
    table.wrapping_add(off as i16 as i32 as u32)
}

pub fn run(x: &mut Cx) {
    if suspended(x) {
        return;
    }
    loop {
        x.tick_counters();
        if x.u16(f::TARGET) == 0 {
            x.call(0x424e);
        }
        x.c.d[0] = x.u8(f::STATE) as u32;
        let off = x.rom16(0x8803e + x.c.d[0]);
        setw(x, 1, off);
        match x.u8(f::STATE) {
            st::INIT => init(x),
            st::ACTIVE => active(x),
            st::HIT => hit::run(x),
            _ => x.free(&pools::BOSS),
        }
        if x.u8(0) == 0 {
            return;
        }
        // ralenti : l'image est rejouée une fois sur trois (compteur +0xba : 2, 1, 0)
        if x.u8(SLOW) != 0 {
            let f = sub8(x, SLOW_TICK, 1);
            if f.eq() {
                x.set8(0x46, 1);
                continue;
            }
            if f.mi() {
                x.set8(SLOW_TICK, 2);
                x.set8(0x46, 0);
                return x.reinsert();
            }
        }
        break;
    }
    aids::phase(x);
    if x.u8(0x43) == 0 {
        let weak = x.u16(WEAK_HP);
        let hp = x.u16(f::HP);
        setw(x, 0, weak);
        setw(x, 1, hp.wrapping_sub(weak));
        if !Fl::sub16(hp, weak).gt() {
            x.set8(SLOW, 1);
        }
    }
    x.reinsert();
}

// --- état 0 : création --------------------------------------------------------------------------

fn init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    x.set_a5_u8(0x50, 1);
    x.set_a5_u32(0x38, 0x0459_5900);
    for a in [0x19e10, 0x19db4, 0x19eb0, 0xe13f0, 0xd6952, 0x424e, 0xa470, 0x788c] {
        x.call(a);
    }
    x.set8(0x2e, 0x28);
    x.call(0x453c);
    for (off, v) in [(0x70, 0), (0x7e, 0x4f), (0x7f, 0), (0x8e, 3), (0x78, 0xff)] {
        x.set8(off, v);
    }
    x.set_a5_u8(0x5a, 0);
    for (off, v) in [(0x68, 8), (0x69, 8), (0x6a, 0), (0x6c, 1), (0x6d, 2), (0x72, 1), (0x73, 1), (0x74, 4), (0x75, 0x40), (0x76, 1), (0x81, 0x22), (0x80, 0x2a)] {
        x.set8(off, v);
    }
    x.set16(SPAWN_TIME, 0xf0);
    x.set16(IDLE_WAIT, 0x50);
    x.set16(IDLE_TIME, 0xf0);
    x.set8(0, 2);
    x.set_bit(0x1, 1, true);
    let px = x.a5_u16(var::SCROLL_X).wrapping_add(0xc0);
    let py = x.a5_u16(var::SCROLL_Y).wrapping_add(0x69);
    setw(x, 0, px);
    setw(x, 1, py);
    x.set16(0x8, px);
    x.set16(0x10, py);
    x.add16(0xc, 0x50);
    x.set16(TIMER, 0x3c);
    x.call(0xb84ae);
    // points de vie « affaiblis » (seuil du ralenti) selon le nombre de joueurs
    let hp = x.u16(0x60);
    setw(x, 1, hp);
    setw(x, 2, hp);
    aids::lsr_w(x, 1, 1);
    aids::lsr_w(x, 2, 2);
    x.c.d[0] = x.a5_u8(var::PLAYERS_ALIVE) as u32;
    let mut d1 = x.c.d[1] as u16;
    if Fl::sub8(x.c.d[0] as u8, 1).gt() {
        if Fl::sub8(x.c.d[0] as u8, 2).ne() {
            x.set8(PHASES, 3);
            d1 = d1.wrapping_add(d1);
        } else {
            x.set8(PHASES, 2);
            d1 = d1.wrapping_add(x.c.d[2] as u16);
        }
    } else {
        x.set8(PHASES, 1);
    }
    setw(x, 1, d1);
    x.set16(WEAK_HP, d1);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(0x93, 1);
    x.set8(0x9a, 0xff);
    x.c.d[0] = 0;
    body(x, 0);
}

// --- état 2 : actif -----------------------------------------------------------------------------

fn active(x: &mut Cx) {
    if x.u16(f::TARGET) == 0 {
        x.call(0x424e);
    }
    if tst8(x, SPAWNED).eq() {
        if sub16(x, SPAWN_TIME, 1).mi() {
            if let Some(e) = x.alloc(&pools::SET) {
                e.set_u8(x.b, 0, 1);
                e.set_u8(x.b, 3, 0x72);
                e.set_u8(x.b, 2, 2);
                x.set8(SPAWNED, 1);
            }
        }
    }
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else if x.a5_u8(var::PAUSE) == 0 {
        let t = dispatch(x, 0x881aa, f::SUB);
        match t {
            0x882ae => fight::intro(x),
            0x8839a => fight::fight(x),
            0x887ac => attacks::run(x),
            0x88f36 => moves::group6(x),
            0x88f78 => moves::group8(x),
            0x891e2 => fight::wait(x),
            0x882a4 => x.set32(f::STATE, 0x0202_0000),
            0x8921e => fight::stand_up(x),
            t => unreachable!("boss 8 : groupe {t:x}"),
        }
        if (x.u8(PHASE_TIMER) as i8) > 0 {
            sub8(x, PHASE_TIMER, 1);
            x.set8(0x38, 0);
        }
        x.set8(0xa4, 0);
        if sub16(x, IDLE_TIME, 1).le() {
            idle_reset(x);
        } else if (x.u16(IDLE_WAIT) as i16) < 0 && x.u16(0xc) == 0 {
            idle_attack(x);
            idle_reset(x);
        }
    }
    x.call(IN_ZONE);
    x.call(SHADOW);
}

fn idle_reset(x: &mut Cx) {
    x.set16(IDLE_WAIT, 0x3c);
    x.set16(IDLE_TIME, 0x12c);
}

/// Trop longtemps sans combattre : le boss se redresse et attaque (état 2, groupe 4, étape tirée dans 0x88294).
fn idle_attack(x: &mut Cx) {
    x.set_bit(0x1, 1, false);
    x.set16(HITS, 0);
    x.set8(0x86, 0);
    x.set8(REACTION, 0);
    x.set8(0, 1);
    x.set8(PHASE_TIMER, 0xa);
    x.set32(f::STATE, 0x0204_0000);
    let alea = x.random();
    let i = alea as u32 & 0xf;
    setw(x, 0, i as u16);
    let step = x.rom8(0x88294 + i);
    x.set8(f::SUB2, step);
}

handlers! {
    0x87fc0 "boss_8" run,
}
