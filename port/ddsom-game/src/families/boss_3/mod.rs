//! Boss de type 3 (0x7f41a) : la créature volante de la scène 11 (nom de debug « Harpy », h). Voir
//! `docs/familles/boss.md`.
//!
//! Charpente des boss à IA (`+4` état, `+5` groupe, `+6` et `+7` étapes, états posés en longs). Le boss vole :
//! la hauteur `+0xc` est sa altitude, il plane, pique sur les joueurs, les saisit et les emporte. Son corps
//! repousse les joueurs (`+0xa4` : forme du corps, routines `body`) et les autres objets (`boss_common`, 0x81aa4).
//!
//! | `+5` | Fichier | Rôle |
//! |---|---|---|
//! | 0 | `fight` | arrivée : descente du haut de l'écran (variante 0 seulement) |
//! | 2 | `fight` | combat : vol stationnaire, demi-tour, approche (`+6` = 4, 8), montée, descente, ronde (16) |
//! | 4 | `attacks` | seize attaques (`+6` = 0 à 30) : coups de griffes, piqués, ruées, cris, enlèvement |
//! | 6, 8 | `fight` | tour de l'écran, envol |
//! | 10 | `fight` | attente tant qu'aucun joueur n'est en vie |
//! | 12 | `fight` | un joueur saisi est emporté puis lâché |
//! | 14 | `fight` | riposte tirée au sort |
//! | état 4 | `hit` | réactions aux coups (table de la ROM), recul, chute, mort |
//!
//! Animations : 0x174776 (corps), 0x1747c4 (attaques), 0x1747f0 (mort).

use super::boss_common::*;
use crate::objects::Obj;

mod attacks;
pub mod body;
mod fight;
mod hit;

/// Champs propres à ce type.
pub(self) mod g {
    pub const VULNERABLE: u32 = 0x67;
    /// Bonds restants de la marche, attente debout, décompte avant le prochain choix de cible.
    pub const WALK_STEPS: u32 = 0x6b;
    pub const IDLE_COUNT: u32 = 0x6e;
    pub const WALK_DELAY: u32 = 0x71;
    /// Sorte de visée demandée à 0xe76c.
    pub const AIM: u32 = 0x79;
    /// Temps au sol (mot).
    pub const DOWN_TIME: u32 = 0x84;
    /// Octet remis à 0xff à chaque image hors pause, à 0 quand le boss plane (h).
    pub const HOVER: u32 = 0xa0;
    /// Mode vol (1) ou sol (0) : posé par les étapes, et par les paliers de PV de la table 0x81712 (`wound_changed`) ;
    /// en vol, les attaques sont tirées dans une autre table.
    pub const FLYING: u32 = 0xa2;
    /// Étape du piqué (attaque 20).
    pub const DIVE: u32 = 0xa3;
    /// Forme du corps qui repousse (0 aucune, 1 devant, 2 de côté), remise à zéro à chaque image.
    pub const BODY_SHAPE: u32 = 0xa4;
    /// Images d'invulnérabilité restantes (`+0x38` remis à zéro).
    pub const INVULNERABLE: u32 = 0xa5;
    /// Minuterie de l'étape (octet ou mot selon l'étape).
    pub const TIMER: u32 = 0xa6;
    /// État à reprendre après un demi-tour (long).
    pub const SAVED_STATE: u32 = 0xaa;
    /// Point de passage de la ronde (mots) et sens dans lequel l'angle tourne.
    pub const GOAL_X: u32 = 0xb0;
    pub const GOAL_Z: u32 = 0xb2;
    pub const GOAL_TURN: u32 = 0xb5;
}
use g::*;
use crate::objects::pools;
use crate::objects::Courant;

/// Jeux d'animations : corps, attaques, mort.
pub(self) const SET_BODY: u32 = 0x174776;
pub(self) const SET_ATTACK: u32 = 0x1747c4;
pub(self) const SET_DEATH: u32 = 0x1747f0;

/// Boss enregistré comme obstacle pour les autres objets (mot, adresse ; voir `boss_common`).
pub(self) const BODY_OWNER: u32 = 0xff8000 - 0x5fb2;

/// Lance l'animation `n` d'un jeu par l'IA (0x103d6).
pub(self) fn body_anim<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_BODY, n);
}

pub(self) fn attack_anim<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>, n: u32) {
    lib::start(x, SET_ATTACK, n);
}

/// Étape suivante (`addq.b #2,(+6)` ou `(+7)`).
pub(self) fn next6<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 2);
}
pub(self) fn next7<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    x.add8(0x7, 2);
}

/// `move.w #v,(6,A0)` : étapes `+6` et `+7` d'un coup.
pub(self) fn set_steps<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>, v: u16) {
    x.set16(f::SUB2, v);
}

/// 0x8188e : vitesses et accélérations (`+0x14..+0x1f`) à zéro.
pub(self) fn stop<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    x.set32(0x14, 0);
    x.set32(0x18, 0);
    x.set32(0x1c, 0);
}

/// 0x8189e : enregistrement de la caméra en service (A2 : 0xff84fe, ou 0xff854e si `A5+0x36`).
pub(self) fn camera<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) -> Obj {
    let a = if var::CAMERA_SELECT.get(x.b) != 0 { 0xff854e } else { 0xff84fe };
    Obj(a)
}

/// 0x7f5ae : plus de cible, attente (`+4` = 0x020a0000).
pub(self) fn idle_alone<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, 0x020a_0000);
}

/// 0x8173c : la cible est-elle derrière le boss ? Sans cible : attente, et vrai.
pub(self) fn target_behind(x: &mut Cx) -> bool {
    if (x.u16(f::TARGET) as i16) >= 0 {
        idle_alone(x);
        return true;
    }
    let t = load_target(x);
    let mut d = t.u16(x.b, 0x8).wrapping_sub(x.u16(0x8));
    if x.u8(f::FLIP) == 0 {
        d = d.wrapping_neg();
    }
    let behind = (d as i16) < 0;
    behind
}

/// 0x81764 : la cible est-elle loin (plus de 0x20 en profondeur ou 0xc0 en largeur) ? Sans cible : attente, et vrai.
pub(self) fn target_far(x: &mut Cx) -> bool {
    if x.u16(f::TARGET) == 0 {
        idle_alone(x);
        return true;
    }
    let t = load_target(x);
    let dz = t.u16(x.b, 0x10).wrapping_sub(x.u16(0x10)).wrapping_add(0x20);
    let far = if dz <= 0x40 {
        false
    } else {
        let dx = t.u16(x.b, 0x8).wrapping_sub(x.u16(0x8)).wrapping_add(0xc0);
        dx > 0x180
    };
    far
}

/// 0x81722 : seizièmes de PV restants (0 à 15).
fn hp_sixteenths(x: &mut Cx) -> u32 {
    let mut d1 = x.u16(f::HP) << 4;
    let max = x.u16(0x60);
    x.c.d[2] = x.c.d[2] & 0xffff_0000 | max as u32;
    let mut d0 = 0xfu32;
    loop {
        d1 = d1.wrapping_sub(max);
        if d1 & 0x8000 != 0 {
            break;
        }
        d0 -= 1;
        if d0 == 0 {
            break;
        }
    }
    d0
}

/// 0x816f8 : le boss vient-il de changer de palier de PV (`+0xa2`, table 0x81712) ?
pub(self) fn wound_changed(x: &mut Cx) -> bool {
    let n = hp_sixteenths(x);
    let w = x.rom8(0x81712 + n);
    let changed = w != x.u8(FLYING);
    if changed {
        x.set8(FLYING, w);
    }
    changed
}

/// Tirage d'une attaque dans une table de 32 octets par niveau de difficulté (lue en données, à partir de `table`) :
/// `+6` = l'attaque, `+4..+5` = actif / attaques, `+7` = 0.
fn draw_attack(x: &mut Cx, table: u32) {
    let level = (var::RANK.get(x.b) >> 1) << 5;
    let at = table.wrapping_add(level as i16 as u32);
    let r = rand_and(x, 0x1f);
    let k = x.data8(at.wrapping_add(r));
    x.set8(f::SUB2, k);
    x.set16(f::STATE, 0x0204);
    x.set8(0x7, 0);
}

/// 0x80e1c / 0x80e22 : attaque tirée pour une cible proche (`far` : à distance) ; blessé, la table des blessés.
pub(self) fn choose_attack(x: &mut Cx, far: bool) {
    let table = if x.u8(FLYING) != 0 {
        0x81262
    } else if far {
        0x81062
    } else {
        0x80e62
    };
    draw_attack(x, table)
}

/// 0x81462 : fin d'une attaque : nouvelle attaque si la cible est tout près, sinon reprise du vol (ou attente).
pub(self) fn after_attack(x: &mut Cx) {
    if !pick_target(x) {
        let s = if x.u8(FLYING) != 0 { 0x0202_0a00 } else { 0x0202_0400 };
        return x.set32(f::STATE, s);
    }
    load_target(x);
    let close = lib::target_in_box(x, -0x40, 0x40, 0x20);
    if x.u8(FLYING) != 0 {
        return x.set32(f::STATE, 0x0202_0a00);
    }
    if close {
        return draw_attack(x, 0x814f8);
    }
    x.set32(f::STATE, 0x0202_0400);
}

pub fn run(x: &mut Cx) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    if var::PAUSE.get(x.b) == 0 {
        x.set8(HOVER, 0xff);
    }
    x.set8(BODY_SHAPE, 0);
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        _ => {
            crate::mem::wr16(x.b, BODY_OWNER, 0);
            x.free(&pools::BOSS);
        }
    }
}

// --- état 0 : création --------------------------------------------------------------------------

/// 0x7f44e : réglages du boss (hors cinématique) ; il s'enregistre comme obstacle.
fn init(x: &mut Cx) {
    if var::PAUSE.get(x.b) != 0 {
        return;
    }
    x.add8(f::STATE, 2);
    x.set_a5_u8(0x50, 1);
    x.set_a5_u32(0x38, 0x0459_5900);
    var::SCENE_LEVEL.set(x.b, 0);
    x.set8(0x93, 1);
    x.set8(0x9a, 0xff);
    pick_target(x);
    lib::draw_6b(x);
    x.set8(f::BOX_SET, 0x23);
    lib::initial_hp_boss(x);
    lib::create_set_14(x);
    lib::create_set_15(x);
    for (off, v) in [(0x70, 0), (0x8e, 3), (0x78, 0xff), (0x68, 1), (0x69, 1)] {
        x.set8(off, v);
    }
    x.set32(f::IMMUNITY, 0xffff_ffff);
    for (off, v) in [(0x6a, 3), (0x6c, 4), (0x6d, 8)] {
        x.set8(off, v);
    }
    let mut n = 6;
    if x.u16(f::TARGET) != 0 && var::PLAYERS_ALIVE.get(x.b) != 1 {
        n = 7;
    }
    x.c.d[0] = n as u32;
    x.set8(0x72, n);
    x.set8(0x73, n);
    for (off, v) in [(0x74, 1), (0x75, 0x44), (0x76, 6), (0x7e, 0x21), (0x7f, 0), (0x81, 0), (0x80, 0x19), (0xbf, 0)] {
        x.set8(off, v);
    }
    x.set8(INVULNERABLE, 0);
    crate::mem::wr16(x.b, BODY_OWNER, x.o.0 as u16);
    crate::families::effects_creators::mirrored_pair(x.b, x.o);
    body_anim(x, 0);
    lib::empty_popups(x);
    lib::clear_popup(x);
    lib::clear_all_popups(x);
}

// --- état 2 : actif -----------------------------------------------------------------------------

/// 0x7f550 : un groupe par image (hors gel et pause), invulnérabilité décomptée, puis zone, ombre et liste.
fn active(x: &mut Cx) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else if var::PAUSE.get(x.b) == 0 {
        match x.u8(f::SUB) {
            0 => fight::intro(x),
            2 => fight::run(x),
            4 => attacks::run(x),
            6 => fight::round(x),
            8 => fight::rise(x),
            10 => fight::wait_for_players(x),
            12 => fight::carrying(x),
            _ => fight::counter(x),
        }
        if tst8(x, INVULNERABLE).gt() {
            sub8(x, INVULNERABLE, 1);
            x.set8(0x38, 0);
        }
    }
    if x.u8(f::SUB) == 0 {
        return;
    }
    end_frame(x, 0x40);
}

/// Fin d'image ordinaire : retenue dans l'écran (marge `margin`), ombre, remise en liste.
pub(self) fn end_frame(x: &mut Cx, margin: u16) {
    lib::keep_on_screen(x, margin);
    lib::collide_terrain(x);
    x.reinsert();
}

/// 0x81ae0 : riposte d'un coup commun (appelée par les réactions, 0x3e22e) : tirage selon la difficulté, puis
/// l'attaque 24 (ruée au sol).
pub(crate) fn counter_reaction(x: &mut Cx) {
    let r = random(x) & 0xf;
    // `andi.l #0xf,D0` : D0 rendu (contrat de sortie)
    x.c.d[0] = r;
    let level = (var::RANK.get(x.b) >> 1).wrapping_mul(4);
    // index `(d8,PC,D1.w)` : mot étendu en signe
    let mask = x.rom16(0x81b1a_u32.wrapping_add(level as i16 as i32 as u32));
    x.c.d[1] = x.c.d[1] & 0xffff_0000 | mask as u32;
    if mask >> r & 1 == 0 {
        return;
    }
    x.set8(INVULNERABLE, 8);
    x.set32(f::STATE, 0x0204_1800);
    x.set8(VULNERABLE, 1);
    x.set8(0, 1);
}

/// Natives : gestionnaire, riposte et formes du corps (appelées par 0x81aa4 de `boss_common`).
pub const NATIVES: &[crate::Native] = &[
    super::cx::handler(0x7f41a, "boss_3", |c, b| {
        let mut x = Cx::new(c, b);
        run(&mut x);
        x.ret();
    }),
    crate::Native {
        entry: 0x81ae0,
        name: "boss_3_riposte",
        run: crate::Corps::Processeur(|c, b| {
            let mut x = Cx::new(c, b);
            counter_reaction(&mut x);
            x.ret();
        }),
        touches_sound: false,
        outputs: crate::regs::d(0) | crate::regs::d(1),
    },
    body::SHOVE_FRONT,
    body::SHOVE_SIDE,
];
