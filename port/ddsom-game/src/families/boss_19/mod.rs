//! Boss de types 14 et 19 (0x8fae4, une seule routine pour les deux), scènes 32, 65 et 66. Voir
//! `docs/familles/boss.md`.
//!
//! Dragon (types 14 « Green Dragon » et 19 « Black Dragon » dans les noms de debug du jeu) : le type 19 de variante 0 est le
//! boss de la scène 32, celui de variante 1 le dragon que Nagpa (`boss_10`) fait venir dans les scènes 65 et 66 (`+0x46` =
//! Nagpa : le dragon disparaît quand Nagpa n'a plus de PV). Le type 14 n'a été vu dans aucune scène des scénarios. Charpente
//! des ennemis (`+4` état, `+5` groupe, `+6` et `+7` étapes), états posés en longs.
//!
//! | `+5` | Fichier | Rôle |
//! |---|---|---|
//! | 0 | `intro` | arrivée : cinématique d'apparition, propre à chaque type et variante |
//! | 2 | `fight` | combat : marche vers un joueur (`+6` = 0, 8), demi-tour (2), arrêt (4), approche (6) |
//! | 4 | `attacks` | cinq attaques (`+6` = 0 à 10) |
//! | 6, 8 | ici | debout |
//! | 10 | ici | attente tant qu'aucun joueur n'est en vie |
//! | état 4 | `hit` | réactions aux coups (table 0x909a0), mort |
//!
//! Animations : trois jeux (`+0xa0` = 0, 2 ou 4) par type, à choisir par `set_anim` ; `+0xa1` est l'animation en cours.
//! Le dragon flotte autour de l'altitude 0x20 (`hover`, 0x91424) quand `+0xa4` est levé.

use super::boss_common::*;
use super::cx::handlers;

mod attacks;
mod fight;
mod hit;
mod intro;

pub(self) mod g {
    pub const TYPE: u32 = 0x2;
    pub const VARIANT: u32 = 0x3;
    pub const REACTION: u32 = 0x49;
    pub const STUN: u32 = 0x56;
    pub const VULNERABLE: u32 = 0x67;
    pub const WALK_STEPS: u32 = 0x6b;
    pub const APPROACH: u32 = 0x6e;
    pub const WALK_DELAY: u32 = 0x71;
    pub const SHOT: u32 = 0x79;
    pub const SHAKE: u32 = 0x90;
    /// Jeu d'animations courant (0, 2 ou 4) et animation en cours.
    pub const ANIM_SET: u32 = 0xa0;
    pub const ANIM_N: u32 = 0xa1;
    /// Boîtes de coups (valeur lue par le moteur de collision) et marque posée par les scripts d'animation.
    pub const HURT_BOX: u32 = 0xa2;
    pub const MARK: u32 = 0xa3;
    /// Flottement demandé (`+0xa4`) et son état (`+0xa5`).
    pub const HOVER: u32 = 0xa4;
    pub const HOVER_STATE: u32 = 0xa5;
    pub const TIMER: u32 = 0xa6;
    pub const SAVED_STATE: u32 = 0xa8;
    /// Compte à rebours de la riposte (`+0xac`, 4 toutes les 32 images) et son rythme (`+0xad`).
    pub const RETALIATE: u32 = 0xac;
    pub const RETALIATE_TICK: u32 = 0xad;
    /// Images d'invulnérabilité restantes.
    pub const SHIELD: u32 = 0xae;
    /// Nombre de ripostes spéciales restantes.
    pub const SPECIALS: u32 = 0xaf;
    pub const BUBBLES: u32 = 0xb0;
    pub const BUBBLE_TICK: u32 = 0xb1;
    pub const PUSHED: u32 = 0xb2;
}
use g::*;
use crate::objects::pools;

pub(self) const TYPE_GREEN: u8 = 0xe;
pub(self) const TYPE_BLACK: u8 = 0x13;

/// États posés d'un coup (`+4..+7`).
pub(self) const STAND: u32 = 0x0202_0000;
const CHASE: u32 = 0x0202_0400;
const TURN: u32 = 0x0202_0200;
pub(self) const WAIT: u32 = 0x020a_0000;
pub(self) const ATTACK_4: u32 = 0x0204_0400;
pub(self) const ATTACK_10: u32 = 0x0204_0a00;
const DEAD_HOST: u32 = 0x0406_0000;
pub(self) const FREE: u32 = 0x0600_0000;

pub(self) mod call {
    pub const MOVE_X: u32 = 0x1d78;
    pub const MOVE_XYZ: u32 = 0x1d90;
    pub const MOVE_XZ: u32 = 0x1da6;
    pub const MOVE_Z: u32 = 0x1dbc;
    pub const MOVE_TABLE_XY: u32 = 0x1cb4;
    pub const ANIM_START: u32 = 0x1a32;
    pub const ANIM_STEP: u32 = 0x1a56;
    pub const ASK_TARGET: u32 = 0xe76c;
    pub const PICK_TARGET: u32 = 0x424e;
    pub const REINSERT_FOREGROUND: u32 = 0x1ef6c;
    pub const EFFECT: u32 = 0xb10c8;
    pub const BURST: u32 = 0xabec8;
}

pub fn run(x: &mut Cx) {
    if suspended(x) {
        return;
    }
    x.call(rom::TICK_COUNTERS);
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        _ => x.free(&pools::BOSS),
    }
}

// --- animations ----------------------------------------------------------------------------------

/// Lance l'animation `n` du jeu `set` (0, 2 ou 4) du type, sauf si elle joue déjà (0x914c6, 0x914e0, 0x914fa).
pub(self) fn set_anim(x: &mut Cx, set: u8, n: u32) {
    let black = x.u8(TYPE) == TYPE_BLACK;
    let base = match (set, black) {
        (0, false) => 0x17dbb0,
        (0, true) => 0x1824d4,
        (2, false) => 0x17dbba,
        (2, true) => 0x1824de,
        (_, false) => 0x17dbd2,
        (_, true) => 0x1824f6,
    };
    x.c.a[4] = base;
    x.c.d[0] = n;
    // l'original charge le numéro du jeu dans D1b avant de comparer (certains appelants le relisent)
    x.c.d[1] = x.c.d[1] & !0xff | set as u32;
    if set == x.u8(ANIM_SET) && n as u8 == x.u8(ANIM_N) {
        return;
    }
    x.set8(ANIM_SET, set);
    x.set8(ANIM_N, n as u8);
    x.call(call::ANIM_START);
}

/// Animation de repos (jeu 0).
pub(self) fn rest(x: &mut Cx) {
    set_anim(x, 0, 0);
}

fn step(x: &mut Cx) -> bool {
    x.call(call::ANIM_STEP);
    carry(x)
}

// --- changements d'état ---------------------------------------------------------------------------

/// 0x913da : debout, au repos.
pub(self) fn stand(x: &mut Cx) {
    x.set32(f::STATE, STAND);
    rest(x);
}

/// 0x913e8 : marche vers un joueur.
pub(self) fn chase(x: &mut Cx) {
    x.set32(f::STATE, CHASE);
    rest(x);
}

/// 0x913f6 : demi-tour.
pub(self) fn turn(x: &mut Cx) {
    x.set32(f::STATE, TURN);
    let side = x.u8(f::FLIP) ^ 0x20;
    x.set8(f::FLIP, side);
    set_anim(x, 0, 1);
}

/// 0x9140a : l'attaque 4 (souffle), invulnérable un instant.
pub(self) fn attack_4(x: &mut Cx) {
    x.set32(f::STATE, ATTACK_4);
    x.set8(SHIELD, 8);
}

/// 0x8fcf0 : attend un joueur.
pub(self) fn wait(x: &mut Cx) {
    x.set32(f::STATE, WAIT);
}

/// 0x9141a : plus d'hôte : le boss disparaît.
pub(self) fn host_gone(x: &mut Cx) {
    x.set32(f::STATE, DEAD_HOST);
}

// --- état 0 : création --------------------------------------------------------------------------

fn init(x: &mut Cx) {
    if x.a5_u8(var::PAUSE) != 0 {
        return;
    }
    x.call(0x63ae0);
    if zero(x) {
        return;
    }
    x.add8(f::STATE, 2);
    x.set8(f::SUB, 0);
    x.set16(f::SUB2, 0);
    x.set8(0x9e, 5);
    x.set8(0x9f, 0);
    x.set8(0x7e, 0x26);
    x.set8(0x80, 0x24);
    let kind = x.u8(TYPE);
    if kind == TYPE_GREEN {
        x.set8(0x7e, 0x4b);
        x.set8(0x80, 0x20);
        x.set8(0x2a, 1);
        x.set8(0x2b, 0x1c);
    }
    if kind == TYPE_BLACK && x.u8(VARIANT) == 1 {
        x.call(0x453c);
    } else {
        if x.a5_u8(0x30) != 0 {
            x.call(0xe1330);
        }
        x.call(0x19db4);
        x.call(0x19e10);
        x.call(0x19eb0);
        x.set_a5_u32(0x38, 0x0459_5900);
        x.set_a5_u8(0x50, 1);
        x.set8(0x93, 1);
        x.set8(0x9a, 0xff);
        x.call(0x453c);
        x.call(0xd5f30);
        x.call(0xd6952);
    }
    x.set8(0x78, 0xff);
    x.set8(f::BOX_SET, 0x2e);
    x.set8(0x82, 1);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    for (off, v) in [(0x76, 5), (0x8e, 3), (0x6f, 2), (0x68, 5), (0x69, 5), (0x6c, 4), (0x6d, 8), (0x6a, 3)] {
        x.set8(off, v);
    }
    x.set_a5_u8(0x5a, 0);
    x.call(0x788c);
    x.set8(0x70, 0);
    x.call(0xa470);
    x.set8(0x81, 0);
    x.set8(SHIELD, 0);
    x.set8(SPECIALS, 3);
    x.set16(ANIM_SET, 0xffff);
    rest(x);
}

// --- état 2 : actif -----------------------------------------------------------------------------

/// Boucle de l'état actif (0x8fc42) ; l'état touché la rappelle pour reprendre le combat.
pub(self) fn active(x: &mut Cx) {
    if x.a5_u8(var::PAUSE) == 0 {
        if tst8(x, f::FREEZE).gt() {
            sub8(x, f::FREEZE, 1);
        } else {
            // le dragon de Nagpa disparaît avec son hôte
            if x.u8(TYPE) == TYPE_BLACK && x.u8(VARIANT) == 1 {
                if (x.u16(f::HOST) as i16) >= 0 {
                    return host_gone(x);
                }
                let host = x.linked(f::HOST);
                if host.u8(x.b, 0) == 0 || host.u16(x.b, f::HP) == 0 {
                    return host_gone(x);
                }
            }
            x.set8(HURT_BOX, 0);
            if sub8(x, RETALIATE_TICK, 1).le() {
                x.set8(RETALIATE_TICK, 0x20);
                x.set8(RETALIATE, 4);
            }
            match x.u8(f::SUB) {
                0 => intro::run(x),
                2 => fight::run(x),
                4 => attacks::run(x),
                6 | 8 => stand(x),
                10 => idle(x),
                _ => {}
            }
            let s = x.u32(f::STATE);
            x.set32(SAVED_STATE, s);
            hover(x);
            if tst8(x, SHIELD).gt() {
                sub8(x, SHIELD, 1);
                x.set8(0x38, 0);
            }
            if tst8(x, PUSHED).gt() && sub8(x, PUSHED, 1).le() {
                x.set8(0x58, 0);
            }
        }
    }
    if x.u8(f::SUB) == 0 {
        return;
    }
    x.call(IN_ZONE);
    x.call(SHADOW);
    x.reinsert();
}

/// `+5` = 10 (0x908bc) : attend qu'un joueur revienne.
fn idle(x: &mut Cx) {
    if x.a5_u8(var::PLAYERS_ALIVE) != 0 {
        x.set32(f::STATE, STAND);
        return;
    }
    x.set8(HOVER, 1);
    if x.u8(0x7) == 0 {
        x.add8(0x7, 2);
        return rest(x);
    }
    step(x);
}

/// 0x91424 : flottement autour de l'altitude 0x20 quand `+0xa4` est levé ; l'altitude ne descend pas sous 0.
fn hover(x: &mut Cx) {
    if x.u8(TYPE) == TYPE_GREEN && x.u8(f::SUB) == 0 {
        return;
    }
    if x.u8(HOVER) == 0 {
        x.set8(HOVER_STATE, 0);
    } else {
        x.set8(HOVER, 0);
        let mut physics = true;
        if x.u8(HOVER_STATE) == 0 {
            // rejoint l'altitude 0x20 d'un pixel par image
            let mut d: i16 = 1;
            if (x.u16(0xc).wrapping_sub(0x20) as i16) >= 0 {
                d = -d;
            }
            x.add16(0xc, d as u16);
            if x.u16(0xc) != 0x20 {
                physics = false;
            } else {
                x.set8(HOVER_STATE, 0xff);
                let (mut vz, mut az) = (0x20i16, -0x200i16);
                if d >= 0 {
                    vz = -vz;
                    az = -az;
                }
                x.set16(0x18, vz as u16);
                x.set16(0x1a, az as u16);
            }
        }
        if physics {
            // oscillation : l'accélération s'inverse de part et d'autre de 0x20
            x.call(call::MOVE_Z);
            let (mut a, mut b) = (0x20i16, -0x200i16);
            let accel = x.u16(0x1a) as i16;
            if x.u16(0xc) as i16 >= 0x20 {
                if accel >= 0 {
                    a = -a;
                    b = -b;
                    x.set16(0x1a, a as u16);
                    x.set16(0x18, b as u16);
                }
            } else if accel < 0 {
                x.set16(0x1a, a as u16);
                x.set16(0x18, b as u16);
            }
        }
    }
    if (x.u16(0xc) as i16) < 0 {
        x.set16(0xc, 0);
    }
}

handlers! {
    0x8fae4 "boss_19" 1200 run,
}
