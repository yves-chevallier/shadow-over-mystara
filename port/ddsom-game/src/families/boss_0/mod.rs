//! Boss de type 0 (0x794ae) : boss de la scène 20 (h), qui se dédouble (une seconde moitié, variante 1, lui est
//! liée par `+0xba`). Voir `docs/familles/boss.md`.
//!
//! Même charpente que le fauve bondissant (`boss_1`) : `+4` état, `+5` groupe, `+6` et `+7` étapes, états posés d'un
//! coup en longs, bibliothèque d'IA 0x103d6.. Il court et saute vers sa cible, la frappe à portée, et quand il
//! n'a plus que la moitié de ses PV il se transforme : la seconde moitié (variante 1) entre en scène et les deux
//! combattent ensemble ; ils ne meurent qu'ensemble.
//!
//! | `+5` | Fichier | Rôle |
//! |---|---|---|
//! | 0 | `moves` | entrées en scène : arrivée par un bord (`+6` = 0), transformation avec création de la seconde moitié (2), arrivée de la seconde moitié (4) |
//! | 2 | `moves` | combat : attente (`+6` = 0), demi-tour (2), course (4, 6), charge (8) |
//! | 4 | `moves` | attaques : sept coups (`+6` = 0 à 12) |
//! | 6 à 10 | `moves` | attente tant qu'aucun joueur n'est en vie |
//! | état 4 | `hit` | réactions aux coups (table de la ROM), recul, chute, mort |
//!
//! Les choix d'attaque (tirages dans les tables de la ROM selon la phase `+0xaa`) et les états qu'ils posent sont ici
//! (`go`). Animations : 0x171a52 (corps), 0x171a7e (entrées en scène, attaques), 0x171a94 (coups reçus).

use super::boss_common::*;
use super::cx::handlers;

mod hit;
mod moves;

/// Champs propres à ce type.
pub(self) mod g {
    pub const VULNERABLE: u32 = 0x67;
    /// Nombre de pas de la course, délai entre deux pas, attente debout.
    pub const WALK_STEPS: u32 = 0x6b;
    pub const IDLE_COUNT: u32 = 0x6e;
    pub const WALK_DELAY: u32 = 0x71;
    /// Temps au sol (mot) et compteur du tremblement (réaction 0x4c).
    pub const DOWN_TIME: u32 = 0x84;
    pub const SHAKE: u32 = 0x90;
    /// Abscisse mémorisée pendant le tremblement (mot).
    pub const SAVED_X: u32 = 0xa;
    /// Sorte d'animation courante et dernière lancée.
    pub const ANIM_KIND: u32 = 0xa0;
    pub const ANIM_LAST: u32 = 0xa1;
    /// État mémorisé (long) à reprendre après un demi-tour.
    pub const SAVED_STATE: u32 = 0xa2;
    /// Minuteries de l'étape (mots).
    pub const TIMER: u32 = 0xa6;
    pub const TIMER2: u32 = 0xa8;
    pub const PHASE: u32 = 0xaa;
    /// Octet non nul : le boss est invulnérable (`+0x38` remis à zéro à chaque image).
    pub const INVULNERABLE: u32 = 0xad;
    /// Coups restants de l'enchaînement.
    pub const COMBO: u32 = 0xae;
    /// Rôle dans l'histoire : 1 = moitié principale transformée, 2 = seconde moitié.
    pub const ROLE: u32 = 0xaf;
    /// Charge vers la position mémorisée de la cible (`+0xbc`) plutôt que vers la caméra.
    pub const CHARGE_AIMED: u32 = 0xb2;
    /// La transformation a eu lieu.
    pub const TRANSFORMED: u32 = 0xb3;
    /// Cette moitié est morte la première (l'autre meurt aussitôt).
    pub const DONE: u32 = 0xb4;
    /// L'autre moitié (mot, adresse RAM ; 0 = aucune).
    pub const PARTNER: u32 = 0xba;
    /// Abscisse de la cible mémorisée (mot).
    pub const MARK_X: u32 = 0xbc;
}
use g::*;
use crate::objects::pools;
use crate::objects::Courant;

/// Jeux d'animations : corps, entrées en scène et attaques, coups reçus.
pub(self) const SET_BODY: u32 = 0x171a52;
pub(self) const SET_ENTRY: u32 = 0x171a7e;
pub(self) const SET_HURT: u32 = 0x171a94;
/// Jeux de marche selon le sens (0x103e0).
pub(self) const SET_RUN_FORWARD: u32 = 0x171b76;
pub(self) const SET_RUN_BACK: u32 = 0x171c1a;

/// Seuils de PV des phases (0x105b6).
pub(self) const THRESHOLDS: u32 = 0x10ae2;

/// Lance l'animation `n` du jeu des corps par l'IA (0x103d6).
pub(self) fn body(x: &mut Cx, n: u32) {
    lib::start(x, SET_BODY, n);
}

/// Lance l'animation `n` du jeu des entrées en scène.
pub(self) fn entry(x: &mut Cx, n: u32) {
    lib::start(x, SET_ENTRY, n);
}

/// Étape suivante (`addq.b #2,(+6)` ou `(+7)`).
pub(self) fn next6(x: &mut Cx) {
    x.add8(f::SUB2, 2);
}
pub(self) fn next7(x: &mut Cx) {
    x.add8(0x7, 2);
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
        _ => x.free(&pools::BOSS),
    }
    if x.u8(INVULNERABLE) != 0 {
        x.set8(0x38, 0);
    }
    if x.u8(0) != 0 {
        x.reinsert();
    }
}

// --- état 0 : création --------------------------------------------------------------------------

/// 0x794e8 : réglages du boss ; la moitié principale (variante 0) attend invulnérable en pose, la seconde moitié
/// (variante 1) attend son tour. Rien n'est réglé si la scène est déjà lancée (`A5+0x52`).
fn init(x: &mut Cx) {
    x.set_bit(0x1, 1, true);
    if x.a5_u16(0x52) == 0 {
        x.add8(f::STATE, 2);
        x.set_a5_u8(0x50, 1);
        pick_target(x);
        x.set8(0xab, 2);
        x.set8(f::BOX_SET, 0x20);
        lib::initial_hp_boss(x);
        var::SCENE_LEVEL.set(x.b, 0);
        lib::spread_index(x);
        lib::clear_popup(x);
        lib::empty_popups(x);
        lib::clear_all_popups(x);
        x.set32(f::IMMUNITY, 0xffff_ffff);
        for (off, v) in [
            (0x76, 0),
            (0x70, 0),
            (0x6a, 2),
            (0x75, 0),
            (0x93, 0),
            (0x72, 1),
            (0x73, 1),
            (0x6c, 6),
            (0x6d, 1),
            (0x74, 7),
            (0x78, 0xff),
            (0x8e, 3),
            (0x68, 0xb),
            (0x69, 1),
            (VULNERABLE, 0),
            (ANIM_KIND, 0),
            (ANIM_LAST, 0xff),
            (0x7f, 0),
        ] {
            x.set8(off, v);
        }
        if x.u8(3) == 0 {
            for (off, v) in [(0x80, 0x1e), (0x81, 0x19), (0x93, 1), (0x9a, 0xff), (0x7e, 0x23)] {
                x.set8(off, v);
            }
            wait_for_transformation(x);
        } else {
            for (off, v) in [(0x7e, 0x24), (0x80, 0x1f), (0x81, 0x1a), (0x2a, 1), (0x2b, 0x1c)] {
                x.set8(off, v);
            }
            partner_waits(x);
        }
    }
    body(x, 4);
}

/// 0x7aad0 : la moitié principale attend, invulnérable, que la scène la lance (entrée en scène `+6` = 0).
fn wait_for_transformation(x: &mut Cx) {
    x.set_bit(0x1, 1, false);
    x.set8(INVULNERABLE, 1);
    x.set8(ANIM_KIND, 0x30);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set32(f::STATE, 0x0200_0000);
}

/// 0x7ab4c : la seconde moitié attend sa création complète (entrée en scène `+6` = 4).
fn partner_waits(x: &mut Cx) {
    x.set_bit(0x1, 1, false);
    x.set8(ANIM_KIND, 0x32);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(0, 2);
    x.set8(ROLE, 2);
    x.set32(f::STATE, 0x0200_0400);
}

// --- état 2 : actif -----------------------------------------------------------------------------

/// 0x79600 : un groupe par image (hors gel et pause), puis zone de jeu et ombre.
fn active(x: &mut Cx) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else if var::PAUSE.get(x.b) == 0 {
        match x.u8(f::SUB) {
            0 => moves::intro(x),
            2 => moves::fight(x),
            4 => moves::attacks(x),
            6 | 8 | 10 => moves::wait_for_players(x),
            _ => {}
        }
    }
    lib::keep_in_zone(x);
    lib::collide_terrain(x);
}

// --- décisions : états posés d'un coup (0x7ab74..0x7ac70) -----------------------------------------

/// Pose le long d'état `+4..+7` et rend le boss vulnérable ou non (`+0x67`).
fn state(x: &mut Cx, vulnerable: bool, s: u32) {
    x.set8(VULNERABLE, vulnerable as u8);
    x.set32(f::STATE, s);
}

/// 0x7ab74 : debout.
pub(self) fn stand(x: &mut Cx) {
    state(x, true, 0x0202_0000);
}

/// 0x7ab84 : demi-tour, l'état courant sera repris.
pub(self) fn turn(x: &mut Cx) {
    x.set8(VULNERABLE, 0);
    let s = x.u32(f::STATE);
    x.set32(SAVED_STATE, s);
    x.set32(f::STATE, 0x0202_0200);
}

/// 0x7ab98 : début de la course (animation de course selon le rôle : 0x34 ou, seconde moitié, 0x36).
pub(self) fn run_start(x: &mut Cx) {
    let kind = if x.u8(3) == 0 { 0x34 } else { 0x36 };
    x.set8(0x79, kind);
    state(x, true, 0x0202_0400);
}

/// 0x7abba : fin de la course (0x35 ou 0x37).
pub(self) fn run_end(x: &mut Cx) {
    let kind = if x.u8(3) == 0 { 0x35 } else { 0x37 };
    x.set8(0x79, kind);
    state(x, true, 0x0202_0600);
}

/// 0x7abdc et 0x7abee : charge, vers la caméra (`aimed` faux) ou vers la cible mémorisée.
pub(self) fn charge(x: &mut Cx, aimed: bool) {
    x.set8(CHARGE_AIMED, aimed as u8);
    state(x, false, 0x0202_0800);
}

/// 0x7ac02, 0x7ac10, 0x7ac1e, 0x7ac32, 0x7ac40, 0x7ac4e, 0x7ac62 : attaque `n` (`+6` = 0 à 12).
pub(self) fn strike(x: &mut Cx, n: u32, shielded: bool) {
    if shielded {
        x.set8(INVULNERABLE, 1);
    }
    state(x, false, 0x0204_0000 | n << 8);
}

/// 0x7ac70 : plus aucun joueur, attente.
pub(self) fn idle_alone(x: &mut Cx) {
    x.set32(f::STATE, 0x020a_0000);
}

/// 0x7aaf4 : la transformation (PV à la moitié) : la moitié principale lance la cinématique.
pub(self) fn transform(x: &mut Cx) {
    x.set8(ANIM_KIND, 0x31);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(0x93, 0);
    x.set_a5_u16(0x51e, 0x30);
    let (cx, cy) = (var::SCROLL_X.get(x.b), var::SCROLL_Y.get(x.b));
    x.set_a5_u16(0x524, cx);
    x.set_a5_u16(0x528, cy);
    x.set8(0, 2);
    x.set8(TRANSFORMED, 1);
    x.set8(ROLE, 1);
    x.set8(INVULNERABLE, 0);
    x.set16(0xc, 0);
    if lib::outside_band(x, 0x30, 0x150) {
        return charge(x, false);
    }
    x.set32(f::STATE, 0x0200_0200);
}

/// Exécute la décision dont l'adresse d'origine est `at` (sauts par tables de la ROM).
pub(self) fn go(x: &mut Cx, at: u32) {
    match at {
        0x7ac02 => strike(x, 0, false),
        0x7ac10 => strike(x, 2, false),
        0x7ac1e => strike(x, 4, false),
        0x7ac32 => strike(x, 6, false),
        0x7ac2c => strike(x, 6, true),
        0x7ac40 => strike(x, 8, false),
        0x7ac4e => strike(x, 10, true),
        0x7ac62 => strike(x, 12, false),
        0x7abee => charge(x, true),
        0x7abdc => charge(x, false),
        _ => panic!("boss 0 : décision inconnue {at:#x}"),
    }
}

/// 0x7a8d2 : cible à courte portée : tirage dans la table 0x7a8e8.
pub(self) fn from_close(x: &mut Cx) {
    let r = rand_and(x, 0xf);
    let i = x.rom8(0x7a8e8 + r) as u32;
    let at = jump_target(x, 0x7a8f8, i);
    go(x, at)
}

/// 0x7a8fa : cible devant (boîte de la course) ; les trois routines de la phase `+0xaa` lisent la même table 0x7a932.
pub(self) fn from_front(x: &mut Cx) {
    let r = rand_and(x, 0xf);
    let i = x.rom8(0x7a932 + r) as u32;
    let at = jump_target(x, 0x7a942, i);
    go(x, at)
}

/// 0x7a944 : l'IA a choisi le geste `g` (1 ou 2) pour la boîte proche ; tirage dans les tables 0x7a9ae (principale),
/// 0x7a9be (seconde moitié, geste 1) ou 0x7a9ce.
pub(self) fn from_near_gesture(x: &mut Cx, g: u32) {
    let r = rand_and(x, 0xf);
    let table = if x.u8(3) == 0 {
        0x7a9ae
    } else if g == 1 {
        0x7a9be
    } else {
        0x7a9ce
    };
    let i = x.rom8(table + r) as u32;
    let at = jump_target(x, 0x7a9de, i);
    go(x, at)
}

/// 0x7a9e4 : l'IA a choisi le geste `g` (1 ou 2) pour la boîte lointaine : sélection de la table d'après la phase.
pub(self) fn from_far_gesture(x: &mut Cx, g: u32) {
    let r = rand_and(x, 0xf);
    let phase = x.u8(PHASE) as i8 as i32 as u32;
    let index = g.wrapping_sub(1).wrapping_mul(2).wrapping_add(phase.wrapping_mul(4));
    let first = if x.u8(3) == 0 { 0x7aa16 } else { 0x7aa22 };
    let entry = jump_target(x, first, index);
    // chaque entrée lit un octet de sa table de 16 octets, qui sert d'index dans la table 0x7aac8
    let table = match entry {
        0x7aa2e | 0x7aa36 | 0x7aa3e | 0x7aa46 => 0x7aa88,
        0x7aa4e | 0x7aa56 => 0x7aa98,
        0x7aa5e | 0x7aa64 | 0x7aa6a | 0x7aa70 => 0x7aaa8,
        0x7aa76 | 0x7aa7c => 0x7aab8,
        _ => panic!("boss 0 : entrée {entry:#x} inconnue"),
    };
    let i = x.rom8(table + r) as u32;
    let at = jump_target(x, 0x7aac8, i);
    go(x, at)
}

/// 0x7a7a4 et 0x7a7b0 : délai entre deux pas de la course, selon la difficulté.
pub(self) fn walk_delay(x: &mut Cx, kind: u8) {
    let level = var::RANK.get(x.b) as u32;
    let table = if kind == 3 { 0x7a7dc } else { 0x7a7bc };
    let d = x.rom8(table + level);
    x.set8(WALK_DELAY, d);
}

/// 0x7a7fc et 0x7a80e : nombre de pas de la course, tiré dans la table du sens.
pub(self) fn draw_steps(x: &mut Cx, kind: u8) {
    let r = rand_and(x, 0x1f);
    let table = if kind == 3 { 0x7a840 } else { 0x7a820 };
    let n = x.rom8(table + r);
    x.set8(WALK_STEPS, n);
}

/// 0x7a860 : cible devant (boîte étroite) et geste choisi par l'IA.
pub(self) fn close_gesture(x: &mut Cx) -> Option<u32> {
    ai_box(x, -0x50, 0x30, 5)
}

/// 0x7a87e : si le boss est dans l'écran, cible dans une boîte profonde (plus étroite pour la seconde moitié) et
/// geste choisi par l'IA.
pub(self) fn far_gesture(x: &mut Cx) -> Option<u32> {
    let cam = camera_x(x);
    let px = x.u16(0x8);
    if Fl::sub16(cam.wrapping_sub(0x10), px).ge() || Fl::sub16(cam.wrapping_add(0x190), px).le() {
        return None;
    }
    let (d0, d1, d2) = if x.u8(3) == 0 { (-0xe0, 0x60, 0xa) } else { (-0xe8, 0x48, 0xc) };
    ai_box(x, d0, d1, d2)
}

handlers! {
    0x794ae "boss_0" run,
    // retour au combat après une réaction commune (table 0x3e000 de `reactions.rs`)
    0x7a5ac "boss_0_releve" hit::get_up,
}
