//! Boss de type 7 (0x859f8) : le premier guerrier noir (« DarkWarrior 1 »), qui paraît aussi, immobile, dans la
//! démonstration de l'attract. Voir `docs/familles/boss.md`.
//!
//! Même charpente que le boss 8 (le second guerrier noir, `boss_8`) : `+4` état, `+5` groupe, `+6` et `+7`
//! étapes, états posés d'un coup en longs, réactions aux coups par les routines communes des ennemis (table de
//! longs 0x86c3a), ralenti une image sur trois quand il est affaibli.
//!
//! | `+5` | Fichier | Rôle |
//! |---|---|---|
//! | 0 | `fight` | arrivée (chute, atterrissage) ; pose de l'attract (`+6` = 2) |
//! | 2 | `fight` | combat : marche (`+6` = 0, 8), demi-tour (2), arrêt (4), approche (6) |
//! | 4 | `attacks` | neuf attaques (`+6` = 0 à 16) |
//! | 6, 8 | `moves` | esquives (disparition et réapparition, bond en arrière, marche courte) |
//! | 10, 12, 14 | ici | attente sans joueur, retour au combat, relevé |
//! | état 4 | `hit` | réactions aux coups, chute, mort |
//!
//! Animations : 0x17809c (mouvements), 0x1780ba (coups).

use super::boss_common::*;
use super::cx::handlers;

mod aids;
mod attacks;
mod fight;
mod hit;
mod moves;

pub(self) mod g {
    pub const KEY: u32 = 0x33;
    pub const SUB3: u32 = 0x7;
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
    /// Rafales encore à lancer (pluie de projectiles).
    pub const BURSTS: u32 = 0xaa;
    pub const PHASE_TIMER: u32 = 0xab;
    pub const VOLLEY: u32 = 0xac;
    pub const PHASES: u32 = 0xad;
    pub const HIT_ACK: u32 = 0xae;
    pub const PICKED: u32 = 0xaf;
    /// Position de la cible au début du coup plongeant (`+0xb0`, `+0xb2`).
    pub const TARGET_X: u32 = 0xb0;
    pub const TARGET_Y: u32 = 0xb2;
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

pub(self) const SET_BODY: u32 = 0x17809c;
pub(self) const SET_STRIKE: u32 = 0x1780ba;

/// État « attente d'un joueur » (groupe 10).
pub(self) const WAIT: u32 = 0x020a_0000;
/// État du combat (groupe 2, marche).
pub(self) const FIGHT: u32 = 0x0202_0000;

pub(self) fn body(x: &mut Cx, n: u32) {
    x.anim_start(SET_BODY, n);
}

pub(self) fn strike(x: &mut Cx, n: u32) {
    x.anim_start(SET_STRIKE, n);
}

pub(self) mod call {
    pub const MOVE_X: u32 = 0x1d78;
    pub const MOVE_XZ: u32 = 0x1da6;
    pub const MOVE_TABLE_XY: u32 = 0x1cb4;
    pub const SOUND_EVENT: u32 = 0x2824;
    pub const SOUND_B: u32 = 0x2860;
    pub const ASK_TARGET: u32 = 0xe76c;
    pub const BURST: u32 = 0xb10c8;
    pub const ATTACK_START: u32 = 0xac3a;
    pub const STEP: u32 = 0x1a56;
    pub const FADE: u32 = 0xecf7a;
    pub const PICK_TARGET: u32 = 0x424e;
    pub const RNG: u32 = 0x19a88;
    /// Seuil des PV et ralenti (`boss_common`, 0x87048).
    pub const HP_THRESHOLD: u32 = 0x87048;
}

/// Mot de poids faible d'un registre de données.
pub(self) fn setw(x: &mut Cx, n: usize, v: u16) {
    set_dw(x, n, v);
}

/// Octet de poids faible d'un registre de données.
pub(self) fn setb(x: &mut Cx, n: usize, v: u8) {
    x.c.d[n] = x.c.d[n] & !0xff | v as u32;
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

/// Clé d'animation `kind` (`+0xa2`, recopiée en `+0xa3`).
pub(self) fn key(x: &mut Cx, kind: u8) {
    x.set8(ANIM_KIND, kind);
    x.set8(ANIM_LAST, kind);
}

/// Décompte de `+0xa0` : vrai quand il est écoulé (passé sous zéro).
pub(self) fn timer_done(x: &mut Cx) -> bool {
    sub16(x, TIMER, 1).mi()
}

pub fn run(x: &mut Cx) {
    if suspended(x) {
        return;
    }
    loop {
        x.call(rom::TICK_COUNTERS);
        if x.u16(f::TARGET) == 0 {
            x.call(call::PICK_TARGET);
        }
        match dispatch(x, 0x85a76, f::STATE) {
            0x85a80 => init(x),
            0x85bd0 => active(x),
            0x86baa => hit::run(x),
            _ => x.free(&pools::BOSS),
        }
        if x.u8(0) == 0 {
            return;
        }
        // ralenti : l'image est rejouée une fois sur trois (compteur +0xba : 3, 2, 1, 0)
        if x.u8(SLOW) != 0 {
            let f = sub8(x, SLOW_TICK, 1);
            if f.eq() {
                x.set8(0x46, 1);
                continue;
            }
            if f.mi() {
                x.set8(SLOW_TICK, 3);
                x.set8(0x46, 0);
                return x.reinsert();
            }
        }
        break;
    }
    x.call(call::HP_THRESHOLD);
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

/// 0x85a80 : en partie, le boss tombe du haut de l'écran (groupe 0) ; dans l'attract, il prend la pose (groupe 0,
/// étape 2).
fn init(x: &mut Cx) {
    if x.a5_u8(0x30) == 0 {
        x.set32(f::STATE, 0x0200_0200);
    } else {
        x.add8(f::STATE, 2);
        x.add16(0xc, 0x50);
        x.call(0xe13f0);
    }
    x.set_a5_u8(0x50, 1);
    for a in [0x19e10, 0x19db4, 0x19eb0, 0xd6952, call::PICK_TARGET, 0xa470, 0x788c] {
        x.call(a);
    }
    x.set8(0x2e, 0x27);
    x.call(0x453c);
    x.call(0xd5f30);
    for (off, v) in [(0x70, 0), (0x7e, 0x23), (0x7f, 0), (0x8e, 3), (0x78, 0xff)] {
        x.set8(off, v);
    }
    x.set_a5_u8(0x5a, 0);
    for (off, v) in [(0x68, 8), (0x69, 8), (0x6a, 0), (0x6c, 1), (0x6d, 2), (0x72, 1), (0x73, 1), (0x74, 4), (0x75, 0x40), (0x76, 1), (0x81, 0x13), (0x80, 0x18)] {
        x.set8(off, v);
    }
    x.set16(SPAWN_TIME, 0xf0);
    x.set16(IDLE_WAIT, 0x3c);
    x.set16(IDLE_TIME, 0xf0);
    x.set8(0, 2);
    x.set_bit(0x1, 1, true);
    // points de vie « affaiblis » (seuil du ralenti) selon le nombre de joueurs
    let hp = x.u16(0x60);
    setw(x, 1, hp >> 1);
    setw(x, 2, hp >> 2);
    let players = x.a5_u8(var::PLAYERS_ALIVE);
    x.c.d[0] = players as u32;
    let mut d1 = x.c.d[1] as u16;
    if Fl::sub8(players, 1).gt() {
        if players == 2 {
            x.set8(PHASES, 2);
            d1 = d1.wrapping_add(x.c.d[2] as u16);
        } else {
            x.set8(PHASES, 3);
            d1 = d1.wrapping_add(d1);
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

/// 0x85bd0 : hors partie (attract), le boss garde la pose ; sinon un groupe par image, une créature d'appoint
/// (SET 0x3f) au bout de 0xf0 images, et une attaque forcée s'il reste trop longtemps sans combattre.
fn active(x: &mut Cx) {
    if x.a5_u8(0x30) == 0 {
        x.set32(f::STATE, 0x0200_0200);
        x.set8(0, 1);
        x.set_bit(0x1, 1, false);
        return x.call(rom::IMMUNITIES);
    }
    if x.u16(f::TARGET) == 0 {
        x.call(call::PICK_TARGET);
    }
    if x.u8(SPAWNED) == 0 && sub16(x, SPAWN_TIME, 1).mi() {
        if let Some(e) = x.alloc(&pools::SET) {
            e.set_u8(x.b, 0, 1);
            e.set_u8(x.b, 3, 0x3f);
            e.set_u8(x.b, 2, 2);
            x.set8(SPAWNED, 1);
        }
    }
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else if x.a5_u8(var::PAUSE) == 0 {
        match dispatch(x, 0x85cc8, f::SUB) {
            0x85d08 => fight::intro(x),
            0x85e18 => fight::fight(x),
            0x8622a => attacks::run(x),
            0x868a4 => moves::group6(x),
            0x868e6 => moves::group8(x),
            0x86af6 => fight::wait(x),
            0x85ce8 => x.set32(f::STATE, FIGHT),
            0x86b32 => fight::stand_up(x),
            t => x.call(t),
        }
        if (x.u8(PHASE_TIMER) as i8) > 0 {
            sub8(x, PHASE_TIMER, 1);
            x.set8(0x38, 0);
        }
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
    x.set16(IDLE_TIME, 0xf0);
}

/// Trop longtemps sans combattre : le boss se redresse et attaque (état 2, groupe 4, attaque tirée dans 0x85cd8).
fn idle_attack(x: &mut Cx) {
    x.set_bit(0x1, 1, false);
    x.set16(HITS, 0);
    x.set8(0x86, 0);
    x.set8(REACTION, 0);
    x.set8(0, 1);
    x.set8(PHASE_TIMER, 0xa);
    x.set32(f::STATE, 0x0204_0000);
    x.call(call::RNG);
    let i = x.c.d[0] & 0xf;
    setw(x, 0, i as u16);
    let attack = x.rom8(0x85cd8 + i);
    x.set8(f::SUB2, attack);
}

handlers! {
    0x859f8 "boss_7" 1200 run,
}
