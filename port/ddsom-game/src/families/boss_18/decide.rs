//! Boss 18 : choix de l'action suivante (changements d'état) et tirages dans les tables de la ROM.
//!
//! Les états sont posés d'un coup comme longs (`+4..+7`) : `0x0202_0400` = actif, groupe 2 (combat), étape 4.
//! Les tables sont lues dans la ROM (espace des opcodes : adressage relatif au PC). Les choix à deux niveaux
//! (octet de la table 1, puis mot de la table 2) donnent l'adresse d'une des routines d'état ci-dessous ;
//! `go` retrouve la routine par cette adresse.

use super::*;

pub(super) const WAIT: u32 = 0x020a_0000;
const STAND: u32 = 0x0202_0000;
const INTERRUPTED: u32 = 0x0202_0200;
const CHASE: u32 = 0x0202_0400;
const CHASE_B: u32 = 0x0202_0600;
const DASH: u32 = 0x0202_0800;

/// 0x9917a : attend (aucun joueur en vie).
pub(super) fn wait<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    x.set32(f::STATE, WAIT);
}

/// 0x98f9c : debout, vulnérable.
pub(super) fn stand(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    x.set32(f::STATE, STAND);
}

/// 0x98fac : interrompt l'action en cours (l'état est mémorisé en `+0xa2`).
pub(super) fn interrupt<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    x.set8(VULNERABLE, 0);
    let state = x.u32(f::STATE);
    x.set32(SAVED_STATE, state);
    x.set32(f::STATE, INTERRUPTED);
}

/// 0x98fc0 : marche vers un joueur (tir 0x29).
pub(super) fn chase(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    x.set8(SHOT, 0x29);
    x.set32(f::STATE, CHASE);
}

/// 0x98fd6 : idem (tir 0x2a), marche lente.
pub(super) fn chase_b<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    x.set8(VULNERABLE, 1);
    x.set8(SHOT, 0x2a);
    x.set32(f::STATE, CHASE_B);
}

/// 0x98fec : la charge si le boss est dans la bande utile de l'écran, sinon l'attaque 16.
pub(super) fn dash_or_attack(x: &mut Cx) {
    let cam = var::CAMERA1_X.get(x.b);
    let here = x.u16(0x8);
    if Fl::sub16(cam.wrapping_add(0x20), here).ge() || Fl::sub16(cam.wrapping_add(0x160), here).lt() {
        return attack_16(x);
    }
    x.set8(VULNERABLE, 0);
    x.set8(SHOT, 0x2a);
    x.set32(f::STATE, DASH);
}

fn attack<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>, state: u32) {
    x.set8(VULNERABLE, 0);
    x.set32(f::STATE, state);
}

/// 0x99032 : attaque 0.
pub(super) fn attack_0<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    attack(x, 0x0204_0000);
}

/// 0x99040 : attaque 2.
pub(super) fn attack_2<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    attack(x, 0x0204_0200);
}

/// 0x9904e : attaque 4 (une fois).
pub(super) fn attack_4<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    x.set8(VULNERABLE, 0);
    x.set8(BUDGET_4, 1);
    x.set32(f::STATE, 0x0204_0400);
}

/// 0x99062 : attaque 6 (poursuite magique) si une cible existe.
pub(super) fn attack_6(x: &mut Cx) {
    if !lib::pick_target_any(x) {
        return wait(x);
    }
    x.set8(VULNERABLE, 0);
    x.set16(CHASE_TIME, 0x168);
    x.set8(AIM_TICK, 2);
    x.set8(AIM_ON, 1);
    x.set32(f::STATE, 0x0204_0600);
}

/// 0x9908c : attaque 8.
pub(super) fn attack_8<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    attack(x, 0x0204_0800);
}

/// 0x9909a : attaque 10 tant qu'il en reste ; sinon l'attaque 20.
pub(super) fn attack_10(x: &mut Cx) {
    if (x.u8(BUDGET_A) as i8) <= 0 {
        return attack_20(x);
    }
    attack(x, 0x0204_0a00);
}

/// 0x990b0 : attaque 12 tant qu'il en reste ; sinon l'attaque 8.
pub(super) fn attack_12<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    if (x.u8(BUDGET_B) as i8) <= 0 {
        return attack_8(x);
    }
    attack(x, 0x0204_0c00);
}

/// 0x990c4 : attaque 14 (lâcher d'objets) si rien n'est déjà lâché et qu'il en reste.
pub(super) fn attack_14<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    x.set8(AIM_ON, 0);
    if (x.u8(SPAWNED) as i8) > 0 || (x.u8(BUDGET_C) as i8) <= 0 {
        return attack_8(x);
    }
    x.set8(INVULNERABLE, 1);
    attack(x, 0x0204_0e00);
}

/// 0x990e8 : attaque 16 tant qu'il en reste ; sinon l'attaque 2.
pub(super) fn attack_16(x: &mut Cx) {
    if (x.u8(BUDGET_D) as i8) <= 0 {
        return attack_2(x);
    }
    sub8(x, BUDGET_D, 1);
    attack(x, 0x0204_1000);
}

/// 0x99102 : attaque 18 (le boss est dans la fenêtre de l'écran) ou 16, 0.
pub(super) fn attack_18(x: &mut Cx) {
    if x.u8(BUDGET_E) == 0 {
        return attack_0(x);
    }
    let cam = var::CAMERA1_X.get(x.b);
    let here = x.u16(0x8);
    if Fl::sub16(cam.wrapping_add(0x30), here).ge() || Fl::sub16(cam.wrapping_add(0x140), here).lt() {
        return attack_16(x);
    }
    let camy = var::CAMERA1_Y.get(x.b);
    let y = x.u16(0x10);
    if Fl::sub16(camy.wrapping_add(0x38), y).ge() || Fl::sub16(camy.wrapping_add(0x70), y).lt() {
        return attack_0(x);
    }
    x.set8(INVULNERABLE, 1);
    attack(x, 0x0204_1200);
}

/// 0x99156 : attaque 20 (si la phase le permet) ; sinon attaque 6 ou 8.
pub(super) fn attack_20(x: &mut Cx) {
    if (x.u8(BUDGET_F) as i8) <= 0 {
        return attack_6(x);
    }
    if (x.u8(PHASE) as i8) <= 0 {
        return attack_8(x);
    }
    x.set8(INVULNERABLE, 1);
    attack(x, 0x0204_1400);
}

// --- tirages ------------------------------------------------------------------------------------

fn table8<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>, table: u32, index: u32) -> u8 {
    x.rom8(table + index)
}

fn table16<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>, table: u32, index: u32) -> u16 {
    x.rom16(table + index)
}

/// Adresse visée par l'entrée `index` (octets) d'une table de mots relatifs à son début.
fn target<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>, table: u32, index: u32) -> u32 {
    (table as i32 + table16(x, table, index) as i16 as i32) as u32
}

/// 0x98cae : délai avant le pas suivant, marche rapide ; 0x98cba : marche lente.
pub(super) fn walk_delay_fast<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    let level = var::RANK.get(x.b) as u32;
    let v = table8(x, 0x98cc6, level);
    x.set8(WALK_DELAY, v);
}

pub(super) fn walk_delay_slow<R: crate::families::cx::Banc>(x: &mut Cx<'_, R>) {
    let level = var::RANK.get(x.b) as u32;
    let v = table8(x, 0x98ce6, level);
    x.set8(WALK_DELAY, v);
}

/// 0x98d06, 0x98d18 : nombre de pas d'une marche.
pub(super) fn walk_steps_fast(x: &mut Cx) {
    let r = random(x) & 0x1f;
    let v = table8(x, 0x98d2a, r);
    x.set8(WALK_STEPS, v);
}

pub(super) fn walk_steps_slow(x: &mut Cx) {
    let r = random(x) & 0x1f;
    let v = table8(x, 0x98d4a, r);
    x.set8(WALK_STEPS, v);
}

/// 0x98d6a, 0x98d86 : un joueur est-il dans la boîte ? Rend 0, ou 1 / 2 : le côté (0x1047a).
pub(super) fn probe(x: &mut Cx, d0: i16, d1: u16, d2: u16) -> u8 {
    if !lib::target_in_box(x, d0, d1, d2) {
        return 0;
    }
    lib::turn_decision(x) as u8
}

/// Exécute la routine d'état dont une table donne l'adresse.
fn go(x: &mut Cx, routine: u32) {
    match routine {
        0x98fec => dash_or_attack(x),
        0x99032 => attack_0(x),
        0x99040 => attack_2(x),
        0x99102 => attack_18(x),
        0x9904e => attack_4(x),
        0x99062 => attack_6(x),
        0x9908c => attack_8(x),
        0x9909a => attack_10(x),
        0x990b0 => attack_12(x),
        0x990c4 => attack_14(x),
        t => unreachable!("boss 18 : routine d'état {t:x}"),
    }
}

/// 0x98da2 : un joueur est tout près.
pub(super) fn choose_close(x: &mut Cx) {
    let r = random(x) & 0xf;
    let pick = table8(x, 0x98db8, r);
    let routine = target(x, 0x98dc8, pick as u32);
    go(x, routine);
}

/// 0x98dcc : un joueur est devant (boîte 0 / 0x2c / 0xc) ; la phase règle le choix.
pub(super) fn choose_front(x: &mut Cx) {
    let r = random(x) & 0xf;
    let phase = x.u8(PHASE) as i8 as i32;
    let kind = target(x, 0x98de6, (2 * phase) as u32);
    let table = match kind {
        0x98dec => 0x98e04,
        0x98df2 => 0x98e14,
        0x98df8 => 0x98e24,
        t => unreachable!("0x98dcc : cible {t:x}"),
    };
    let pick = table8(x, table, r);
    let routine = target(x, 0x98e34, pick as u32);
    go(x, routine);
}

/// 0x98e3c : un joueur est à mi-distance ; `side` = 1 ou 2.
pub(super) fn choose_mid(x: &mut Cx, side: u8) {
    let r = random(x) & 0xf;
    let table = if side == 1 { 0x98e6e } else { 0x98e7e };
    let pick = table8(x, table, r);
    let routine = target(x, 0x98e8e, pick as u32);
    go(x, routine);
}

/// 0x98e96 : un joueur est loin ; `side` = 1 ou 2.
pub(super) fn choose_far(x: &mut Cx, side: u8) {
    let r = random(x) & 0xf;
    let phase = x.u8(PHASE) as i8 as i32;
    let entry = (side as i32 - 1) * 2 + 4 * phase;
    let routine = target(x, 0x98eba, entry as u32);
    let pick = match routine {
        0x98ec6 => table8(x, 0x98f2c, r),
        0x98ed2 | 0x98ed8 => table8(x, 0x98f3c, r),
        0x98eec => {
            let slot = (var::SCENE_LEVEL.get(x.b) as u32).min(4);
            let line = target(x, 0x98f04, 2 * slot);
            let table = match line {
                0x98f0e => 0x98f4c,
                0x98f14 => 0x98f5c,
                0x98f1a => 0x98f6c,
                0x98f20 => 0x98f7c,
                0x98f26 => 0x98f8c,
                t => unreachable!("0x98eec : ligne {t:x}"),
            };
            table8(x, table, r)
        }
        t => unreachable!("0x98e96 : cible {t:x}"),
    };
    let routine = target(x, 0x98ee4, pick as u32);
    match routine {
        0x9908c => attack_8(x),
        0x9909a => attack_10(x),
        0x990b0 => attack_12(x),
        0x990c4 => attack_14(x),
        t => unreachable!("0x98edc : cible {t:x}"),
    }
}

// --- budgets et tirages de la phase ---------------------------------------------------------------------

/// 0x9946e..: le boss est debout, choisit sa suite d'après sa phase (0x99428).
pub(super) fn back_on_feet(x: &mut Cx) {
    x.set8(REACTION, 0);
    x.set32(0x58, 0);
    x.set8(0, 1);
    x.set8(ANIM_LAST, 0xff);
    if var::GAME_ON.get(x.b) == 0 {
        return wait(x);
    }
    if !lib::pick_target_any(x) {
        return wait(x);
    }
    if lib::next_threshold(x, 0x114e2) {
        if cmp8(x, PHASE, 2).eq() {
            return attack_20(x);
        }
        return attack_14(x);
    }
    if x.u8(AIM_ON) != 0 {
        // la poursuite magique reprend sans retirer sa durée
        return x.set32(f::STATE, 0x0204_0600);
    }
    chase(x);
}
