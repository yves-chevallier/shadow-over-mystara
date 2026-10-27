//! Boss de type 1 (0x7acea) : le fauve bondissant de la scène 27 (nom de debug « Displacer Beast », h). Voir
//! `docs/familles/boss.md`.
//!
//! Même charpente que la Chimère (`boss_4`) : `+4` état, `+5` groupe, `+6` et `+7` étapes, états posés d'un coup en
//! longs, bibliothèque d'IA 0x103d6..0x105b6. Le boss se déplace par bonds (une marche est une suite de sauts vers
//! un point tiré autour de sa cible) et saisit les joueurs.
//!
//! | `+5` | Fichier | Rôle |
//! |---|---|---|
//! | 0 | `moves` | arrivée : traversée de l'écran en sautant, puis chute du haut de l'écran |
//! | 2 | `moves` | combat : attente, demi-tour, marche par bonds (`+6` = 4, 6), grand bond (8), bond en arrière (10) |
//! | 4 | `moves` | attaques : quatre coups (`+6` = 0 à 6), saisie (8), bond avec un double (10) |
//! | 6 à 10 | `moves` | attente tant qu'aucun joueur n'est en vie |
//! | 12 | `moves` | un joueur saisi : morsures qui lui retirent des PV |
//! | 14 | `hit` | retour au combat |
//! | état 4 | `hit` | réactions aux coups (table de la ROM), recul, chute, mort |
//!
//! Les choix d'attaque (tirages dans les tables de la ROM selon la phase `+0xaa`) et les états qu'ils posent sont
//! ici (`go`). Animations : 0x172794 (corps), 0x1727d4 (attaques), 0x1727e4 (coups reçus).

use super::boss_common::*;
use super::cx::handlers;
use crate::objects::Obj;

mod hit;
mod moves;

/// Champs propres à ce type.
pub(self) mod g {
    /// Bonds restants de la marche, attente debout.
    pub const WALK_STEPS: u32 = 0x6b;
    pub const IDLE_COUNT: u32 = 0x6e;
    pub const VULNERABLE: u32 = 0x67;
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
    pub const PHASES: u32 = 0xab;
    /// Octet non nul : le boss est invulnérable (`+0x38` remis à zéro à chaque image).
    pub const INVULNERABLE: u32 = 0xad;
    /// Coups restants de l'enchaînement d'attaques.
    pub const COMBO: u32 = 0xae;
    /// Compteur des bouffées de l'arrivée.
    pub const PUFFS: u32 = 0xaf;
    /// Double (ENEMY 0x1a) créé pour le bond en ciseaux (mot, 0 = aucun).
    pub const DOUBLE: u32 = 0xb0;
    /// Tentatives de saisie (index de la table des chances).
    pub const GRAB_TRIES: u32 = 0xb2;
    /// Grand bond : 0 vers la cible, 1 vers le milieu de l'écran.
    pub const LEAP_TO_CENTER: u32 = 0xb3;
    /// Première animation du bond de marche (7 ou 15 selon le sens).
    pub const HOP_ANIM: u32 = 0xb4;
    /// État de l'image précédente (long).
    pub const PREV_STATE: u32 = 0xb8;
    /// Point visé par un bond (mots).
    pub const MARK_X: u32 = 0xbc;
    pub const MARK_Z: u32 = 0xbe;
}
use g::*;
use crate::objects::pools;
use crate::objects::Courant;

/// Jeux d'animations : corps, attaques, coups reçus.
pub(self) const SET_BODY: u32 = 0x172794;
pub(self) const SET_ATTACK: u32 = 0x1727d4;
pub(self) const SET_HURT: u32 = 0x1727e4;

/// Seuils de PV des phases (0x105b6).
pub(self) const THRESHOLDS: u32 = 0x10be2;

/// Lance l'animation `n` d'un jeu par l'IA (0x103d6).
pub(self) fn body(x: &mut Cx, n: u32) {
    lib::start(x, SET_BODY, n);
}

pub(self) fn attack(x: &mut Cx, n: u32) {
    lib::start(x, SET_ATTACK, n);
}

/// `moveq #k,D0 ; move.b D0,(0xa0) ; move.b D0,(0xa1)` : sorte d'animation `k`.
pub(self) fn kind(x: &mut Cx, k: u8) {
    x.c.d[0] = k as u32;
    x.set8(ANIM_KIND, k);
    x.set8(ANIM_LAST, k);
}

/// Étape suivante (`addq.b #2,(+6)` ou `(+7)`).
pub(self) fn next6(x: &mut Cx) {
    x.add8(f::SUB2, 2);
}
pub(self) fn next7(x: &mut Cx) {
    x.add8(0x7, 2);
}

/// Au moins un joueur en vie (0x10474).
pub(self) fn alive(x: &mut Cx) -> bool {
    lib::players_alive(x)
}

/// Le boss est-il hors de la bande `[caméra + d0, caméra + d1]` (0x104a2) ?
pub(self) fn outside(x: &mut Cx, d0: i16, d1: u16) -> bool {
    lib::outside_band(x, d0, d1 as i16)
}

/// Le boss doit-il se retourner vers sa cible (0x1050c) ?
pub(self) fn turn_needed(x: &mut Cx) -> bool {
    !lib::facing_target(x)
}

/// Boîte large devant le boss (0x7ef4).
pub(self) fn in_wide_box(x: &mut Cx, d0: i16, d1: u16, d2: u16) -> bool {
    lib::guard_in_front(x, d0, d1, d2)
}

/// Bouffée de poussière de sorte `k` (0x1172e).
pub(self) fn dust(x: &mut Cx, k: u32) {
    lib::debris(x, k);
}

/// `subq.w #1,(off,A0)` : vrai quand le décompte est écoulé (`ble`).
pub(self) fn expired(x: &mut Cx, off: u32) -> bool {
    sub16(x, off, 1).le()
}

/// Cible du boss (`movea.w (0x50,A0),A1`).
pub(self) fn target_obj(x: &mut Cx) -> crate::objects::Obj {
    load_target(x)
}

/// Mot de la table de sauts relative au PC `base` pour l'index `i` : adresse de la routine choisie.
pub(self) fn jump_target(x: &mut Cx, base: u32, i: u32) -> u32 {
    base.wrapping_add(x.rom16(base + i) as i16 as u32)
}

pub fn run(x: &mut Cx) {
    if suspended(x) {
        return;
    }
    x.tick_counters();
    let s = x.u32(f::STATE);
    x.set32(PREV_STATE, s);
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

/// 0x7ad2a : réglages du boss ; il attend, en pose, le début de la cinématique d'arrivée.
fn init(x: &mut Cx) {
    x.add8(f::STATE, 2);
    x.set_a5_u8(0x50, 1);
    x.set_a5_u32(0x38, 0x0459_5900);
    pick_target(x);
    x.set8(PHASES, 2);
    x.set8(f::BOX_SET, 0x21);
    lib::initial_hp_boss(x);
    var::SCENE_LEVEL.set(x.b, 0);
    lib::spread_index(x);
    lib::empty_popups(x);
    lib::clear_popup(x);
    lib::clear_all_popups(x);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    for (off, v) in [(0x76, 0), (0x70, 0), (0x6a, 2), (0x75, 0)] {
        x.set8(off, v);
    }
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
        (0x7e, 0x25),
        (0x7f, 0),
        (0x80, 0x23),
        (0x81, 0x1d),
    ] {
        x.set8(off, v);
    }
    body(x, 4);
    lib::sound(x, 0xe1370);
}

// --- état 2 : actif -----------------------------------------------------------------------------

/// 0x7ae16 : un groupe par image (hors gel et pause), puis zone de jeu et ombre une fois arrivé.
fn active(x: &mut Cx) {
    if tst8(x, f::FREEZE).gt() {
        sub8(x, f::FREEZE, 1);
    } else if var::PAUSE.get(x.b) == 0 {
        match x.u8(f::SUB) {
            0 => moves::intro(x),
            2 => moves::fight(x),
            4 => moves::attacks(x),
            6..=10 => moves::wait_for_players(x),
            12 => moves::holding(x),
            _ => hit::get_up(x),
        }
    }
    if x.u8(f::SUB) != 0 {
        lib::keep_in_zone(x);
        lib::collide_terrain(x);
    }
}

// --- décisions : états posés d'un coup (0x7c24a..0x7c372) -----------------------------------------

/// Pose le long d'état `+4..+7` et rend le boss vulnérable ou non (`+0x67`).
fn state(x: &mut Cx, vulnerable: bool, s: u32) {
    x.set8(VULNERABLE, vulnerable as u8);
    x.set32(f::STATE, s);
}

/// 0x7c24a : debout.
pub(self) fn stand(x: &mut Cx) {
    state(x, true, 0x0202_0000);
}

/// 0x7c25a : demi-tour, l'état courant sera repris.
pub(self) fn turn(x: &mut Cx) {
    x.set8(VULNERABLE, 0);
    let s = x.u32(f::STATE);
    x.set32(SAVED_STATE, s);
    x.set32(f::STATE, 0x0202_0200);
}

/// 0x7c26e et 0x7c27c : marche par bonds (deux tirages du nombre de bonds).
pub(self) fn walk(x: &mut Cx) {
    state(x, false, 0x0202_0400);
}
pub(self) fn walk_b(x: &mut Cx) {
    state(x, false, 0x0202_0600);
}

/// 0x7c2a2 : grand bond, vers la cible ou (`center`) vers le milieu de l'écran.
pub(self) fn leap(x: &mut Cx, center: bool) {
    x.set8(LEAP_TO_CENTER, center as u8);
    state(x, false, 0x0202_0800);
}

/// 0x7c28a : grand bond vers la cible après une riposte (invulnérable).
pub(self) fn leap_guarded(x: &mut Cx) {
    x.set8(INVULNERABLE, 1);
    leap(x, false);
}

/// 0x7c2b0 : bond en arrière, sauf hors de l'écran (grand bond).
fn back_step(x: &mut Cx) {
    if outside(x, 0x18, 0x168) {
        return leap_guarded(x);
    }
    x.set8(INVULNERABLE, 1);
    state(x, false, 0x0202_0a00);
}

/// 0x7c2de : premier coup (une cible saisissable reçoit le coup 4).
fn strike_a(x: &mut Cx) {
    let t = target_obj(x);
    let s = if t.u8(x.b, 0x66) != 0 { 0x0204_0400 } else { 0x0204_0000 };
    x.set8(0x57, 0);
    state(x, false, s);
}

/// 0x7c304 : deuxième coup (ou le coup 6 contre une cible saisissable).
fn strike_b(x: &mut Cx) {
    let t = target_obj(x);
    let s = if t.u8(x.b, 0x66) != 0 { 0x0204_0600 } else { 0x0204_0200 };
    x.set8(0x57, 0);
    state(x, false, s);
}

/// 0x7c300 : enchaînement (nombre de coups tiré selon la difficulté) qui commence par le deuxième coup.
fn combo(x: &mut Cx) {
    combo_count(x);
    strike_b(x);
}

/// 0x7c34a : saisie.
fn grab(x: &mut Cx) {
    state(x, false, 0x0204_0800);
}

/// 0x7c358 : bond en ciseaux avec un double.
pub(self) fn split_leap(x: &mut Cx) {
    x.set8(0, 2);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set8(GRAB_TRIES, 0);
    x.set32(f::STATE, 0x0204_0a00);
}

/// 0x7c372 : plus aucun joueur, attente.
pub(self) fn idle_alone(x: &mut Cx) {
    x.set32(f::STATE, 0x020a_0000);
}

/// Exécute la décision dont l'adresse d'origine est `at` (sauts par tables de la ROM).
pub(self) fn go(x: &mut Cx, at: u32) {
    match at {
        0x7c24a => stand(x),
        0x7c25a => turn(x),
        0x7c26e => walk(x),
        0x7c27c => walk_b(x),
        0x7c28a => leap_guarded(x),
        0x7c296 => leap(x, true),
        0x7c29e => leap(x, false),
        0x7c2b0 => back_step(x),
        0x7c2da => {
            x.set8(COMBO, 0);
            strike_a(x)
        }
        0x7c2de => strike_a(x),
        0x7c2fa => {
            x.set8(INVULNERABLE, 1);
            combo(x)
        }
        0x7c300 => combo(x),
        0x7c304 => strike_b(x),
        0x7c344 => {
            x.set8(INVULNERABLE, 1);
            grab(x)
        }
        0x7c34a => grab(x),
        0x7c358 => split_leap(x),
        0x7c372 => idle_alone(x),
        _ => panic!("boss 1 : décision inconnue {at:#x}"),
    }
}

/// 0x7c0ba : cible dans la boîte large, tirage dans la table 0x7c0d0.
pub(self) fn from_wide_box(x: &mut Cx) {
    let r = rand_and(x, 0xf);
    let i = x.rom8(0x7c0d0 + r) as u32;
    x.c.d[0] = i;
    let at = jump_target(x, 0x7c0e0, i);
    go(x, at)
}

/// 0x7c0e2 : cible tout près ; tirage dans la table de la phase `+0xaa`.
pub(self) fn from_close(x: &mut Cx) {
    let r = rand_and(x, 0xf);
    let phase = x.u8(PHASE) as i8 as i32 as u32;
    let table = [0x7c11a, 0x7c12a, 0x7c13a][phase as usize % 3];
    let i = x.rom8(table + r) as u32;
    x.c.d[0] = i;
    let at = jump_target(x, 0x7c14a, i);
    go(x, at)
}

/// 0x7c14e et 0x7c1cc : l'IA a choisi le geste `g` (1 ou 2) ; tirage dans la table de la phase.
fn from_gesture(x: &mut Cx, g: u32, tables: [u32; 6], jumps: u32) {
    let r = rand_and(x, 0xf);
    let phase = x.u8(PHASE) as i8 as i32;
    let i = (g as i32 - 1 + 2 * phase) as usize;
    let k = x.rom8(tables[i.min(5)] + r) as u32;
    x.c.d[0] = k;
    let at = jump_target(x, jumps, k);
    go(x, at)
}

/// 0x7c14e : boîte proche (-0x70).
pub(self) fn from_near_gesture(x: &mut Cx, g: u32) {
    from_gesture(x, g, [0x7c1a8, 0x7c1a8, 0x7c1b8, 0x7c1b8, 0x7c1b8, 0x7c1b8], 0x7c1c8)
}

/// 0x7c1cc : boîte lointaine (-0xe0).
pub(self) fn from_far_gesture(x: &mut Cx, g: u32) {
    from_gesture(x, g, [0x7c226, 0x7c226, 0x7c226, 0x7c226, 0x7c236, 0x7c236], 0x7c246)
}

/// 0x7c080 / 0x7c09e : cible dans la boîte et geste choisi par l'IA (0x1047a), ou rien.
pub(self) fn ai_box(x: &mut Cx, d0: i16, d1: u16, d2: u16) -> Option<u32> {
    if !lib::target_in_box(x, d0, d1, d2) {
        return None;
    }
    let turn = lib::turn_decision(x);
    (turn != lib::Turn::None).then_some(turn as u32)
}

/// 0x7c47c : nombre de coups de l'enchaînement, selon la difficulté et le nombre de joueurs.
fn combo_count(x: &mut Cx) {
    let level = (var::RANK.get(x.b) & 0x1e) * 2;
    let mut players = var::PLAYERS_ALIVE.get(x.b) as u16;
    if players != 0 {
        players -= 1;
    }
    let i = level.wrapping_add(players);
    x.c.d[0] = x.c.d[0] & 0xffff_0000 | i as u32;
    x.c.d[1] = players as u32;
    let n = x.rom8(0x7c49a + i as u32);
    x.set8(COMBO, n);
}

/// 0x7c50c : tentative de bond en ciseaux (sans double en cours) : une chance qui croît à chaque essai.
pub(self) fn try_split_leap(x: &mut Cx) -> bool {
    if x.u16(DOUBLE) != 0 {
        x.c.d[0] = 0;
        return false;
    }
    x.add8(GRAB_TRIES, 1);
    let r = rand_and(x, 0xf);
    let tries = x.u8(GRAB_TRIES) as u32;
    let mask = x.rom16(0x7c534 + 2 * tries);
    x.c.d[1] = mask as u32;
    mask >> r & 1 != 0
}

/// 0x7c01c / 0x7c02e : nombre de bonds de la marche, tiré dans la table `table`.
pub(self) fn draw_steps(x: &mut Cx, table: u32) {
    let r = rand_and(x, 0x1f);
    let n = x.rom8(table + r);
    x.set8(WALK_STEPS, n);
}

/// 0x7c396 / 0x7c3b0 : point visé près de la cible (décalages tirés dans `table`, `mask` + 1 entrées), du côté
/// du boss ; l'angle vers ce point (0x137e) devient la direction.
pub(self) fn aim_near_target(x: &mut Cx, table: u32, mask: u32) {
    let i = rand_and(x, mask) * 4;
    x.c.d[0] = i;
    let dx = x.rom16(table + i);
    let dz = x.rom16(table + i + 2);
    set_dw(x, 1, dx);
    set_dw(x, 2, dz);
    let t = target_obj(x);
    let tx = t.u16(x.b, 0x8);
    set_dw(x, 0, tx);
    let mut px = dx;
    if tx as i16 >= x.u16(0x8) as i16 {
        px = px.wrapping_neg();
    }
    let mx = px.wrapping_add(tx);
    let mz = dz.wrapping_add(t.u16(x.b, 0x10));
    set_dw(x, 1, mx); // relu : D1, D2 relus par la suite (DDSOM_POISON : covb_long_27, covb_long3p_27)
    set_dw(x, 2, mz); // relu : relu
    x.set16(MARK_X, mx);
    x.set16(MARK_Z, mz);
    aim_at_mark(x);
}

/// 0x137e vers (D1, D2) puis `+0x2d` = l'angle.
pub(self) fn aim_at_mark(x: &mut Cx) {
    lib::aim_from(x, (x.c.d[1] as u16, x.c.d[2] as u16));
    let a = x.c.d[0] as u8;
    x.set8(Obj::SPEED_INDEX, a);
}

/// 0x7c430 : vitesse verticale du bond selon la distance au point visé : 10 de plus par tranche de la vitesse
/// horizontale (la plus grande des deux distances, avec la vitesse correspondante).
pub(self) fn hop_height(x: &mut Cx) {
    let abs = |v: u16| if (v as i16) < 0 { v.wrapping_neg() } else { v };
    // `move.w (0x1c,A0),D3` : le mot haut de D3 est celui que laisse la direction 0x137e appelée juste avant par
    // les deux appelants (`moveq #0,D3` puis le pas) : nul
    let d3 = abs(x.u16(0x1c)) as u32;
    let mut d4 = abs(x.u16(0x14)) as u32;
    let dz = abs(x.u16(MARK_Z).wrapping_sub(x.u16(0x10)));
    let mut d2 = abs(x.u16(MARK_X).wrapping_sub(x.u16(0x8))) as u32;
    if (d2 as u16 as i16) < dz as i16 {
        d2 = dz as u32;
        d4 = d3;
    }
    d2 = d2.rotate_left(16);
    d4 = (d4.rotate_left(16) as i32 >> 8) as u32;
    let mut d1: u16 = 0;
    loop {
        let f = Fl::sub32(d2, d4);
        d2 = d2.wrapping_sub(d4);
        if f.le() {
            break;
        }
        d1 = d1.wrapping_add(10);
    }
    x.add16(0x18, d1);
    x.c.d[1] = d1 as u32;
    x.c.d[2] = d2;
    x.c.d[3] = d3;
    x.c.d[4] = d4;
}

/// 0x7c4da : une bouffée (EFFCT 0x32, variante 4) toutes les `n` + 1 images.
pub(self) fn puff(x: &mut Cx, n: u8) {
    x.c.d[0] = n as u32;
    x.add8(PUFFS, 1);
    if (n as i8) > x.u8(PUFFS) as i8 {
        return;
    }
    x.set8(PUFFS, 0);
    let Some(e) = x.alloc(&pools::EFFCT) else { return };
    e.set_u8(x.b, 0, 1);
    e.set_u8(x.b, 2, 0x32);
    e.set_u8(x.b, 3, 4);
    e.set_u16(x.b, 0x42, x.o.0 as u16);
    let side = x.u8(f::FLIP);
    e.set_u8(x.b, 0x2c, side);
}

/// 0x7c542 : crée le double (ENEMY 0x1a) du bond en ciseaux, lié au boss par `+0xb0`.
pub(self) fn spawn_double(x: &mut Cx) {
    let Some(d) = x.alloc(&pools::ENEMY) else {
        x.set16(DOUBLE, 0);
        return;
    };
    let active = x.u8(0);
    d.set_u8(x.b, 0, active);
    d.set_u8(x.b, 2, 0x1a);
    d.set_u32(x.b, f::IMMUNITY, 0xffff_ffff);
    d.set_u8(x.b, f::BOX_SET, 0x21);
    for off in [0x8, 0xc, 0x10] {
        let v = x.u16(off);
        d.set_u16(x.b, off, v);
    }
    let side = x.u8(f::FLIP);
    d.set_u8(x.b, f::FLIP, side);
    d.set_u16(x.b, DOUBLE, x.o.0 as u16);
    x.set16(DOUBLE, d.0 as u16);
}

handlers! {
    0x7acea "boss_1" run,
    // retour au combat après une réaction commune (table 0x3e000 de `reactions.rs`)
    0x7bece "boss_1_releve" hit::get_up,
}
