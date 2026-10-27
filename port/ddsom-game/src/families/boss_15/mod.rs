//! Boss de types 15 et 20 (0x91530, une seule routine pour les deux), scène 46. Voir `docs/familles/boss.md`.
//!
//! Dragon rouge (type 15 « Red Dragon » dans les noms de debug ; le type 20 est une variante plus petite jamais vue dans
//! les scénarios) : de la même famille que le dragon de `boss_19` (mêmes tirages de cible, mêmes sondes de portée) avec ses
//! propres tables, un décor de vol en trois temps et une « armure » (les premiers coups de chaque période de 64 images ne
//! l'interrompent pas). Charpente des ennemis (`+4` état, `+5` groupe, `+6` et `+7` étapes), états posés en longs.
//!
//! | `+5` | Fichier | Rôle |
//! |---|---|---|
//! | 0 | `intro` | arrivée : tombe du ciel par la droite en trois temps |
//! | 2 | `fight` | combat : marche (`+6` = 0), saut vers un joueur (2), approche (4) |
//! | 4 | `attacks` | onze attaques (`+6` = 0 à 18), la plupart en vol |
//! | 6, 8, 10 | ici | debout, puis attente tant qu'aucun joueur n'est en vie |
//! | 12 | ici | prise d'un joueur (drainé jusqu'à 1 PV) |
//! | 14 | ici | attente |
//! | état 4 | `hit` | réaction (une seule routine), recul, relevé, mort |
//!
//! Animations : trois jeux par type (`+0xa0` = 0, 2 ou 4, `set_anim`), `+0xa1` est l'animation en cours.

use super::boss_common::*;
use super::cx::handlers;

mod attacks;
mod fight;
mod hit;
mod intro;

pub(self) mod g {
    pub const TYPE: u32 = 0x2;
    pub const STUN: u32 = 0x56;
    pub const VULNERABLE: u32 = 0x67;
    pub const WALK_STEPS: u32 = 0x6b;
    pub const WALK_DELAY: u32 = 0x71;
    pub const SHOT: u32 = 0x79;
    /// Foreground : la liste d'affichage du premier plan (`+0x43`).
    pub const FOREGROUND: u32 = 0x43;
    pub const SHAKE: u32 = 0x90;
    pub const ANIM_SET: u32 = 0xa0;
    pub const ANIM_N: u32 = 0xa1;
    pub const HURT_BOX: u32 = 0xa2;
    pub const HOVER: u32 = 0xa4;
    pub const HOVER_STATE: u32 = 0xa5;
    pub const TIMER: u32 = 0xa6;
    /// Position mémorisée à la fin de l'image précédente (X, altitude, profondeur) : un coup ne déplace pas le dragon.
    pub const KEPT_X: u32 = 0xa8;
    pub const KEPT_Z: u32 = 0xaa;
    pub const KEPT_Y: u32 = 0xac;
    pub const SHIELD: u32 = 0xae;
    pub const SPECIALS: u32 = 0xaf;
    /// État actif de l'image précédente (long) et sa boîte de coups (`+0xb9`).
    pub const LAST_STATE: u32 = 0xb0;
    pub const LAST_BOX: u32 = 0xb9;
    /// Armure : période (`+0xb4`, 64 images), coups encore ignorés (`+0xb5`), temporisation de la poussée (`+0xb8`).
    pub const ARMOR_TICK: u32 = 0xb4;
    pub const ARMOR: u32 = 0xb5;
    pub const COOLDOWN: u32 = 0xb7;
    pub const PUSHED: u32 = 0xb8;
}
use g::*;
use crate::objects::pools;
use crate::objects::Courant;

pub(self) const TYPE_BIG: u8 = 0xf;
pub(self) const TYPE_SMALL: u8 = 0x14;

pub(self) const STAND: u32 = 0x0202_0000;
const TURN: u32 = 0x0202_0200;
const CHASE: u32 = 0x0202_0400;
pub(self) const WAIT: u32 = 0x020a_0000;
pub(self) const ATTACK_4: u32 = 0x0204_0400;

pub(self) mod call {
    pub const PICK_FAR: u32 = 0x43ae;
    pub const CATCH_IN_REACH: u32 = 0xfc44;
    pub const SEIZE: u32 = 0xfd64;
    pub const DRAG_SEIZED: u32 = 0xfda2;
    pub const SOUND: u32 = 0x289c;
}

pub fn run(x: &mut Cx) {
    if suspended(x) {
        return;
    }
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        _ => x.free(&pools::BOSS),
    }
}

// --- animations ----------------------------------------------------------------------------------

/// Lance l'animation `n` du jeu `set` (0, 2 ou 4) du type, sauf si elle joue déjà (0x93182, 0x9319c, 0x931b6).
pub(self) fn set_anim(x: &mut Cx, set: u8, n: u32) {
    let small = x.u8(TYPE) == TYPE_SMALL;
    let base = match (set, small) {
        (0, false) => 0x17e03a,
        (0, true) => 0x18295e,
        (2, false) => 0x17e048,
        (2, true) => 0x18296c,
        (_, false) => 0x17e096,
        (_, true) => 0x1829ba,
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
    let set = x.c.a[4];
    let n = x.c.d[0] as u16;
    x.anim_start(set, n);
}

pub(self) fn rest(x: &mut Cx) {
    set_anim(x, 0, 0);
}

pub(self) fn step(x: &mut Cx) -> bool {
    x.anim_step()
}

// --- changements d'état ---------------------------------------------------------------------------

/// 0x92ff4 : debout, au repos.
pub(self) fn stand(x: &mut Cx) {
    set_abs8(x, 0xff8043, 0xff);
    x.set8(HURT_BOX, 0);
    x.set32(f::STATE, STAND);
    rest(x);
}

/// 0x9301e : repos ou marche, au hasard (le petit dragon : seulement le demi-tour).
pub(self) fn wander(x: &mut Cx) {
    let mut r = random(x) as u16 & 3;
    if x.u8(TYPE) != TYPE_BIG {
        r &= 1;
    }
    x.set8(HURT_BOX, 0);
    if r as u8 == 0 {
        x.set32(f::STATE, TURN);
    } else {
        x.set16(TIMER, 0x100);
        x.set32(f::STATE, CHASE);
    }
    rest(x);
}

/// 0x91748 : attend un joueur.
pub(self) fn wait(x: &mut Cx) {
    x.set32(f::STATE, WAIT);
}

// --- état 0 : création --------------------------------------------------------------------------

fn init(x: &mut Cx) {
    if x.a5_u8(var::PAUSE) != 0 {
        return;
    }
    x.call(0x76ec4);
    if zero(x) {
        return;
    }
    x.add8(f::STATE, 2);
    x.set8(f::SUB, 0);
    x.set16(f::SUB2, 0);
    x.set_a5_u32(0x38, 0x0459_5900);
    x.set_a5_u8(0x50, 1);
    x.set8(0x93, 1);
    x.set8(0x9a, 0xff);
    let big = x.u8(TYPE) == TYPE_BIG;
    x.set8(0x7e, if big { 0x50 } else { 0x29 });
    x.call(0x453c);
    x.call(0xd5f30);
    x.call(0xd6952);
    x.set8(0x78, 0xff);
    x.set8(f::BOX_SET, 0x2f);
    x.set8(0x82, 1);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(0, 2);
    for (off, v) in [(0x76, 5), (0x8e, 3), (0x6f, 2), (0x68, 5), (0x69, 5), (0x6c, 4), (0x6d, 8), (0x6a, 3)] {
        x.set8(off, v);
    }
    x.set_a5_u8(0x5a, 0);
    x.call(0x788c);
    x.set8(0x70, 0);
    x.call(0xa470);
    x.set8(0x81, 0);
    x.set8(0x80, if big { 0x28 } else { 0x2c });
    x.set8(FOREGROUND, 0);
    x.set8(0xb6, 0);
    x.set8(SHIELD, 0);
    x.set8(HURT_BOX, 0);
    x.set8(SPECIALS, if big { 2 } else { 3 });
    let (px, pz, py) = (x.u16(0x8), x.u16(0xc), x.u16(0x10));
    x.set16(KEPT_X, px);
    x.set16(KEPT_Z, pz);
    x.set16(KEPT_Y, py);
    x.set16(ANIM_SET, 0xffff);
    rest(x);
    x.call(0x19db4);
    x.call(0x19e10);
    x.call(0x19eb0);
}

// --- état 2 : actif -----------------------------------------------------------------------------

/// Boucle de l'état actif (0x91684) ; l'état touché la rappelle pour reprendre le combat.
pub(self) fn active(x: &mut Cx) {
    if x.u8(FOREGROUND) != 0 {
        x.set8(FOREGROUND, 0);
        x.unlink();
    }
    if x.a5_u8(var::PAUSE) == 0 {
        if tst8(x, f::FREEZE).gt() {
            sub8(x, f::FREEZE, 1);
        } else {
            if tst8(x, PUSHED).gt() && sub8(x, PUSHED, 1).le() {
                x.set32(0x58, 0);
            }
            x.set8(0x42, 0);
            if sub8(x, ARMOR_TICK, 1).le() {
                x.set8(ARMOR_TICK, 0x40);
                x.set8(ARMOR, 4);
            }
            restore_position(x);
            match x.u8(f::SUB) {
                0 => intro::run(x),
                2 => fight::run(x),
                4 => attacks::run(x),
                6 | 8 => {
                    stand(x);
                    idle(x);
                }
                10 => idle(x),
                12 => grab(x),
                _ => {}
            }
            let s = x.u32(f::STATE);
            x.set32(LAST_STATE, s);
            let b = x.u8(HURT_BOX);
            x.set8(LAST_BOX, b);
            keep_position(x);
            hover(x);
            if tst8(x, SHIELD).gt() {
                sub8(x, SHIELD, 1);
                x.set8(0x38, 0);
            }
        }
    }
    finish(x);
}

/// 0x9171e : fin de l'image : le premier plan reçoit le dragon quand `+0x43` est levé.
pub(self) fn finish(x: &mut Cx) {
    x.set8(0xb6, 0);
    if x.u8(FOREGROUND) != 0 {
        x.unlink();
        x.reinsert_foreground();
    } else {
        x.reinsert();
    }
}

/// Les positions mémorisées reprennent la place du dragon au début de l'image (un coup ne le déplace pas).
pub(self) fn restore_position(x: &mut Cx) {
    let (px, pz, py) = (x.u16(KEPT_X), x.u16(KEPT_Z), x.u16(KEPT_Y));
    x.set16(0x8, px);
    x.set16(0xc, pz);
    x.set16(0x10, py);
}

pub(self) fn keep_position(x: &mut Cx) {
    let (px, pz, py) = (x.u16(0x8), x.u16(0xc), x.u16(0x10));
    x.set16(KEPT_X, px);
    x.set16(KEPT_Z, pz);
    x.set16(KEPT_Y, py);
}

/// 0x93066 : balancement vertical, quand `+0xa4` est levé : l'altitude suit une table de sinus (0x1c67b6).
pub(self) fn hover(x: &mut Cx) {
    if x.u8(f::SUB) == 0 {
        return;
    }
    if x.u8(HOVER) == 0 {
        x.set8(HOVER_STATE, 0);
        return;
    }
    x.set8(HOVER, 0);
    let phase = x.u8(HOVER_STATE).wrapping_add(1);
    x.set8(HOVER_STATE, phase);
    let i = (phase as u32) & 0x7c;
    let w = x.data16(0x1c67b6 + 2 + i);
    x.add32(0xc, (((w as u32) << 16) as i32 >> 8) as u32);
}

/// `+5` = 6, 8, 10 (0x927fa) : attend qu'un joueur revienne.
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
    fight::steer(x);
}

/// `+5` = 12 (0x9282c) : prise d'un joueur : il perd des PV puis est relâché.
fn grab(x: &mut Cx) {
    if (x.u16(f::TARGET) as i16) >= 0 {
        return stand(x);
    }
    let p = x.linked(f::TARGET);
    if p.u8(x.b, 0) == 0 || p.u8(x.b, 0x188) == 0 {
        return stand(x);
    }
    match x.u8(0x7) {
        0 => {
            x.add8(0x7, 2);
            x.set8(TIMER, 0x20);
            let hp = p.u16(x.b, f::HP).wrapping_sub(0x10);
            p.set_u16(x.b, f::HP, hp);
            if (hp as i16) <= 0 {
                p.set_u16(x.b, f::HP, 1);
            }
            set_anim(x, 2, 4);
            x.call(call::DRAG_SEIZED);
        }
        2 => {
            if (x.u8(TIMER) as i8) > 0 {
                if !sub8(x, TIMER, 1).gt() {
                    x.add8(0x7, 2);
                    x.set8(TIMER, 0x10);
                }
                if (x.u16(0xc) as i16) > 0 {
                    x.set16(0xc, 0);
                }
            }
            grab_step(x);
        }
        4 => grab_step(x),
        _ => {}
    }
}

fn grab_step(x: &mut Cx) {
    if !step_anim(x) {
        return x.call(call::DRAG_SEIZED);
    }
    x.set16(ANIM_SET, 0xffff);
    set_anim(x, 2, 4);
    if !sub8(x, TIMER, 1).gt() {
        return release(x);
    }
    if (x.u16(f::TARGET) as i16) >= 0 {
        return stand(x);
    }
    let p = x.linked(f::TARGET);
    if (p.u16(x.b, f::HP) as i16) <= 1 {
        return release(x);
    }
    let hp = p.u16(x.b, f::HP).wrapping_sub(1);
    p.set_u16(x.b, f::HP, hp);
    x.call(0xedd96);
    x.call(call::DRAG_SEIZED);
}

fn release(x: &mut Cx) {
    x.set8(0x45, 1);
    // 0xfd8c relâche le joueur désigné par A1 (resté chargé depuis le début de la prise)
    x.c.a[1] = x.linked(f::TARGET).0;
    x.call(0xfd8c);
    set_anim(x, 2, 5);
    stand(x);
}

/// `jsr 0x1a56` : un pas de script, rend vrai à la fin (retenue levée).
fn step_anim(x: &mut Cx) -> bool {
    x.anim_step()
}

handlers! {
    0x91530 "boss_15" run,
}

/// Routine du boss appelée par un effet (0xb482e) : rapprochement d'un objet vers un autre.
pub const HELPERS: &[crate::Native] = &[attacks::APPROACH];
