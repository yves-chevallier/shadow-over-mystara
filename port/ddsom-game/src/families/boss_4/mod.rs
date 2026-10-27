//! Boss de type 4 (0x81b3a) : la Chimère (scènes 104 et 105, et dans `cov_2p_c`, `cov_tour4_c3`). Voir
//! `docs/familles/boss.md`.
//!
//! Charpente des boss à IA (`+4` état, `+5` groupe, `+6` et `+7` étapes, états posés d'un coup en longs,
//! bibliothèque d'IA 0x103d6..0x105b6). Le corps est accompagné de trois doubles graphiques (ENEMY 22, 23, 24 de
//! `enemy_double`), créés avec lui et liés par `+0xb4`, `+0xb6`, `+0xb8` ; le premier (`+0xb4`) rejoue sur demande
//! (`+0xb6` du double) les gestes du souffle et dit par son `+0x33` quand souffler.
//!
//! | `+5` | Fichier | Rôle |
//! |---|---|---|
//! | 0 | `fight` | arrivée en descendant du haut de l'écran |
//! | 2 | `fight` | combat : attente, demi-tour, marche (`+6` = 4, 6), charge (8), bond (10) |
//! | 4 | `attacks` | sept attaques (`+6` = 0 à 12) : cri, envol et piqué, traversée, bond, souffles |
//! | 6 à 12 | `fight` | attente tant qu'aucun joueur n'est en vie |
//! | 14 | `hit` | retour au combat |
//! | état 4 | `hit` | réactions aux coups (table de la ROM), recul, chute, mort |
//!
//! Les choix d'attaque (tirages dans les tables de la ROM selon la phase `+0xaa`) et les états qu'ils posent sont
//! dans `decide`. Animations : 0x17557e (corps), 0x175596 (vol et attaques), 0x1755ae (coups reçus).

use super::boss_common::*;
use super::cx::handlers;
use crate::objects::Obj;

mod attacks;
mod decide;
mod fight;
mod hit;

/// Champs propres à ce type.
pub(self) mod g {
    /// Pas restants de la marche (`+0x6b`), décompte avant le prochain pas (`+0x71`), attente debout (`+0x6e`).
    pub const WALK_STEPS: u32 = 0x6b;
    pub const IDLE_COUNT: u32 = 0x6e;
    pub const WALK_DELAY: u32 = 0x71;
    pub const STUN: u32 = 0x56;
    pub const VULNERABLE: u32 = 0x67;
    pub const SHOT: u32 = 0x79;
    /// Compteur du tremblement (réaction 0x4c).
    pub const SHAKE: u32 = 0x90;
    /// Jeu d'animations courant et animation en cours.
    pub const ANIM_KIND: u32 = 0xa0;
    pub const ANIM_LAST: u32 = 0xa1;
    /// État mémorisé (long) à reprendre après un demi-tour.
    pub const SAVED_STATE: u32 = 0xa2;
    /// Minuterie de l'étape et cadence de la poussière (ou de l'oscillation en vol).
    pub const TIMER: u32 = 0xa6;
    pub const DUST: u32 = 0xa8;
    pub const PHASE: u32 = 0xaa;
    pub const PHASES: u32 = 0xab;
    /// Octet non nul : le boss est invulnérable (`+0x38` remis à zéro à chaque image).
    pub const INVULNERABLE: u32 = 0xad;
    /// Décompte entre deux jets du souffle, et rang du jet dans la table des angles (`+0x42`).
    pub const BREATH: u32 = 0xaf;
    pub const BREATH_STEP: u32 = 0x42;
    /// Charge : 0 vers le milieu de l'écran, 1 vers l'abscisse retenue de la cible (`+0xbc`).
    pub const CHARGE_AT_TARGET: u32 = 0xb2;
    pub const MARK_X: u32 = 0xbc;
    /// Battement d'ailes : alterné à chaque marque (0x11a36).
    pub const WING: u32 = 0xb3;
    /// Les trois doubles (mots).
    pub const DOUBLE_A: u32 = 0xb4;
    pub const DOUBLE_B: u32 = 0xb6;
    pub const DOUBLE_C: u32 = 0xb8;
    /// Coups encaissés au sol (mot).
    pub const HITS: u32 = 0x84;
}
use g::*;
use crate::objects::pools;
use crate::objects::Courant;

/// Jeux d'animations : corps, vol et attaques, coups reçus.
pub(self) const SET_BODY: u32 = 0x17557e;
pub(self) const SET_AIR: u32 = 0x175596;
pub(self) const SET_HURT: u32 = 0x1755ae;

/// Lance l'animation `n` d'un jeu par l'IA (0x103d6).
pub(self) fn body(x: &mut Cx, n: u32) {
    lib::start(x, SET_BODY, n);
}

pub(self) fn air(x: &mut Cx, n: u32) {
    lib::start(x, SET_AIR, n);
}

/// `moveq #k,D0 ; move.b D0,(0xa0) ; move.b D0,(0xa1)` : sorte d'animation `k`.
pub(self) fn kind(x: &mut Cx, k: u8) {
    x.c.d[0] = k as u32;
    x.set8(ANIM_KIND, k);
    x.set8(ANIM_LAST, k);
}

/// Mot de poids faible d'un registre de données.
pub(self) fn setw(x: &mut Cx, n: usize, v: u16) {
    set_dw(x, n, v);
}

/// `subq.w #1,(off,A0)` : vrai quand le décompte est écoulé (`ble`).
pub(self) fn expired(x: &mut Cx, off: u32) -> bool {
    sub16(x, off, 1).le()
}

/// Au moins un joueur en vie (0x10474).
pub(self) fn alive(x: &mut Cx) -> bool {
    lib::any_player_alive(x)
}

/// Le double `off` (`movea.w (off,A0),A1`).
pub(self) fn double(x: &mut Cx, off: u32) -> Obj {
    x.c.a[1] = x.u16(off) as i16 as i32 as u32;
    x.linked(off)
}

/// Demande au premier double de rejouer son geste (`+0xb6` du double).
pub(self) fn cue_double(x: &mut Cx) {
    let d = double(x, DOUBLE_A);
    d.set_u8(x.b, 0xb6, 1);
}

/// Toutes les 4 images, une bouffée de poussière (0x1172e, sorte 3).
pub(self) fn dust_tick(x: &mut Cx) {
    if !sub16(x, DUST, 1).gt() {
        x.set16(DUST, 4);
        lib::debris(x, 3);
    }
}

/// Image-clé marquée (bit 7 de `+0x33`) : un battement d'ailes, de côté alterné.
pub(self) fn flap(x: &mut Cx) {
    if (x.u8(f::KEY_FRAME) as i8) < 0 {
        let k = x.u8(f::KEY_FRAME) & 0x7f;
        x.set8(f::KEY_FRAME, k);
        let w = x.u8(WING) ^ 1;
        x.set8(WING, w);
        lib::ground_debris(x, w);
    }
}

/// `moveq #0,D0 ; move.w (off,A0),D0 ; swap D0 ; asr.l #8,D0` : vitesse (mot) en long à 8 bits de fraction.
pub(self) fn speed_long(x: &mut Cx, off: u32) -> u32 {
    let v = ((x.u16(off) as i16 as i32) << 8) as u32;
    x.c.d[0] = v;
    v
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

/// 0x81b74 : réglages du boss, création des trois doubles ; en partie, il entre par le haut de l'écran.
fn init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    x.set_a5_u8(0x50, 1);
    x.set_a5_u32(0x38, 0x0459_5900);
    pick_target(x);
    x.set8(PHASES, 2);
    x.set8(f::BOX_SET, 0x24);
    lib::initial_hp_boss(x);
    var::SCENE_LEVEL.set(x.b, 0);
    lib::spread_index(x);
    lib::clear_popup(x);
    lib::empty_popups(x);
    lib::clear_all_popups(x);
    for (off, v) in [(0x76, 0), (0x70, 0), (0x6a, 2), (0x75, 0)] {
        x.set8(off, v);
    }
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(0, 2);
    x.set_bit(0x1, 1, true);
    for (off, v) in [
        (0x93, 1),
        (0x9a, 0xff),
        (0x72, 1),
        (0x73, 1),
        (0x6c, 6),
        (0x6d, 1),
        (0x74, 7),
        (0x78, 0xff),
        (0x8e, 3),
        (0x68, 8),
        (0x69, 1),
        (VULNERABLE, 0),
        (ANIM_KIND, 0),
        (ANIM_LAST, 0xff),
        (0x7e, 0x4d),
        (0x7f, 0),
        (0x80, 0x25),
        (0x81, 0x1e),
    ] {
        x.set8(off, v);
    }
    // deux doubles avec leurs propres PV (ceux du boss moins 0xfa ou 0x12c, au moins 0x96 ou 0x64), un troisième sans
    spawn_double(x, 0x16, Some((0xfa, 0x96, 0x43)), DOUBLE_A);
    spawn_double(x, 0x17, Some((0x12c, 0x64, 0x44)), DOUBLE_B);
    spawn_double(x, 0x18, None, DOUBLE_C);
    if var::GAME_ON.get(x.b) != 0 {
        x.c.d[0] = 0;
        body(x, 0);
        lib::sound(x, 0xe1330);
    } else {
        x.set8(0, 1);
        x.set_bit(0x1, 1, false);
        decide::wait(x);
        lib::set_immunities(x);
        x.c.d[0] = 0;
        body(x, 0);
    }
}

/// Crée un double ENEMY de type `kind` (le pool est attendu tant qu'il est plein) et le lie au boss (`link`).
/// `life` : (retrait, minimum, jeu de boîtes) pour un double qui a ses propres PV.
fn spawn_double(x: &mut Cx, kind: u8, life: Option<(u16, u16, u8)>, link: u32) {
    let Some(d) = alloc_retry(x, &pools::ENEMY) else { return };
    d.set_u8(x.b, 0, 1);
    d.set_u8(x.b, 2, kind);
    if let Some((cut, least, boxes)) = life {
        let hp = x.u16(0x60);
        let mut v = hp.wrapping_sub(cut);
        if (v as i16) < 0 {
            v = least;
        }
        setw(x, 0, v);
        for off in [0x60, 0x62, 0x64] {
            d.set_u16(x.b, off, v);
        }
        d.set_u8(x.b, f::BOX_SET, boxes);
    }
    d.set_u32(x.b, f::IMMUNITY, 0xffff_ffff);
    if life.is_some() {
        for (off, v) in [(ANIM_KIND, 0), (ANIM_LAST, 0xff), (0x7f, 0), (0x80, 0), (0x81, 0)] {
            d.set_u8(x.b, off, v);
        }
    }
    d.set_u16(x.b, 0xb2, x.o.0 as u16);
    x.set16(link, d.0 as u16);
}

// --- état 2 : actif -----------------------------------------------------------------------------

/// 0x81d6e : un groupe par image (hors gel et pause), puis zone de jeu et ombre.
fn active(x: &mut Cx) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else if var::PAUSE.get(x.b) == 0 {
        match x.u8(f::SUB) {
            0 => fight::intro(x),
            2 => fight::run(x),
            4 => attacks::run(x),
            6..=12 => fight::wait_for_players(x),
            _ => hit::resume(x),
        }
    }
    lib::keep_in_zone(x);
    lib::collide_terrain(x);
}

handlers! {
    0x81b3a "boss_4" run,
    // retour au combat après une réaction commune (table 0x3e000 de `reactions.rs`)
    0x82ea0 "boss_4_releve" hit::resume,
}
