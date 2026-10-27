//! Boss 6 : choix de l'action suivante (changements d'état) et tirages dans les tables de la ROM.
//! Même principe que `boss_17::decide` ; les tables de décision sont lues dans la ROM (adressage relatif au PC).

use super::*;

pub(super) const WAIT: u32 = 0x020a_0000;
const STAND: u32 = 0x0202_0000;
const INTERRUPTED: u32 = 0x0202_0200;
const CHASE: u32 = 0x0202_0400;
const CHASE_B: u32 = 0x0202_0600;
const CHARGE: u32 = 0x0202_0800;
const SHOOT: u32 = 0x0202_0a00;
const ATTACK_0: u32 = 0x0204_0000;
const ATTACK_2: u32 = 0x0204_0200;
const ATTACK_4: u32 = 0x0204_0400;
const ATTACK_6: u32 = 0x0204_0600;
const ATTACK_8: u32 = 0x0204_0800;

fn set_state(x: &mut Cx, vulnerable: u8, state: u32) {
    x.set8(VULNERABLE, vulnerable);
    x.set32(f::STATE, state);
}

/// 0x85962 : attend (aucun joueur en vie).
pub(super) fn wait(x: &mut Cx) {
    x.set32(f::STATE, WAIT);
}

/// 0x8588a : debout et vulnérable.
pub(super) fn stand(x: &mut Cx) {
    set_state(x, 1, STAND);
}

/// 0x8589a : interrompt l'action (état mémorisé en `+0xa2`).
pub(super) fn interrupt(x: &mut Cx) {
    x.set8(VULNERABLE, 0);
    let state = x.u32(f::STATE);
    x.set32(SAVED_STATE, state);
    x.set32(f::STATE, INTERRUPTED);
}

/// 0x858ae : marche rapide vers un joueur.
pub(super) fn chase(x: &mut Cx) {
    x.set8(SHOT, 0x2c);
    set_state(x, 1, CHASE);
}

/// 0x858c4 : marche lente.
pub(super) fn chase_b(x: &mut Cx) {
    x.set8(SHOT, 0x2d);
    set_state(x, 1, CHASE_B);
}

/// 0x858da (`at_target` faux) et 0x858ec : charge vers la caméra ou vers la cible.
pub(super) fn charge(x: &mut Cx, at_target: bool) {
    x.set8(VULNERABLE, 0);
    x.set8(CHARGE_AT_TARGET, at_target as u8);
    x.set32(f::STATE, CHARGE);
}

/// 0x85900 : tir en bond.
pub(super) fn shoot(x: &mut Cx) {
    set_state(x, 0, SHOOT);
}

pub(super) fn attack_0(x: &mut Cx) {
    set_state(x, 1, ATTACK_0);
}

pub(super) fn attack_2(x: &mut Cx) {
    set_state(x, 0, ATTACK_2);
}

pub(super) fn attack_4(x: &mut Cx) {
    set_state(x, 0, ATTACK_4);
}

/// 0x8593a : attaque 6 en invulnérable.
pub(super) fn attack_6_shielded(x: &mut Cx) {
    x.set8(INVULNERABLE, 1);
    set_state(x, 0, ATTACK_6);
}

/// 0x8594e : attaque 8 en invulnérable.
pub(super) fn attack_8_shielded(x: &mut Cx) {
    x.set8(INVULNERABLE, 1);
    set_state(x, 0, ATTACK_8);
}

/// Cible d'une table de sauts de mots relatifs à son début.
fn target(x: &mut Cx, table: u32, index: u32) -> u32 {
    (table as i32 + x.rom16(table + index) as i16 as i32) as u32
}

/// Applique le changement d'état dont l'adresse d'origine est `routine`.
fn apply(x: &mut Cx, routine: u32) {
    match routine {
        0x858ec => charge(x, true),
        0x85900 => shoot(x),
        0x8590e => attack_0(x),
        0x8591e => attack_2(x),
        0x8592c => attack_4(x),
        0x85940 => {
            x.set8(VULNERABLE, 0);
            x.set32(f::STATE, ATTACK_6);
        }
        0x8594e => attack_8_shielded(x),
        0x85954 => {
            x.set8(VULNERABLE, 0);
            x.set32(f::STATE, ATTACK_8);
        }
        t => unreachable!("boss 6 : cible {t:x}"),
    }
}

// --- tirages ------------------------------------------------------------------------------------

pub(super) fn walk_delay_fast(x: &mut Cx) {
    let level = var::RANK.get(x.b) as u32;
    let v = x.rom8(0x85614 + level);
    x.set8(WALK_DELAY, v);
}

pub(super) fn walk_delay_slow(x: &mut Cx) {
    let level = var::RANK.get(x.b) as u32;
    let v = x.rom8(0x85634 + level);
    x.set8(WALK_DELAY, v);
}

pub(super) fn walk_steps_fast(x: &mut Cx) {
    let r = random(x) & 0x1f;
    let v = x.rom8(0x85678 + r);
    x.set8(WALK_STEPS, v);
}

pub(super) fn walk_steps_slow(x: &mut Cx) {
    let r = random(x) & 0x1f;
    let v = x.rom8(0x85698 + r);
    x.set8(WALK_STEPS, v);
}

/// Un joueur est-il dans la boîte `(d0, d1, d2)` ? 0 sinon, ou 1 / 2 : le côté (0x1047a).
pub(super) fn probe(x: &mut Cx, d0: i16, d1: u16, d2: u16) -> u8 {
    if !lib::target_in_box(x, d0, d1, d2) {
        return 0;
    }
    lib::turn_decision(x) as u8
}

/// 0x856f2 : un joueur est tout près devant.
pub(super) fn choose_close(x: &mut Cx) {
    let r = random(x) & 0xf;
    let pick = x.rom8(0x85708 + r);
    let t = target(x, 0x85718, pick as u32);
    apply(x, t);
}

/// 0x8571a : un joueur est devant ; la table dépend de la phase.
pub(super) fn choose_front(x: &mut Cx) {
    let r = random(x) & 0xf;
    let phase = x.u8(PHASE) as i8 as i32;
    let routine = target(x, 0x85734, (2 * phase) as u32);
    let table = match routine {
        0x8573a => 0x85752,
        0x85740 => 0x85762,
        0x85746 => 0x85772,
        t => unreachable!("boss 6 : cible {t:x}"),
    };
    let pick = x.rom8(table + r);
    let t = target(x, 0x85782, pick as u32);
    apply(x, t);
}

/// 0x85788 : un joueur est à mi-distance (côté `side`).
pub(super) fn choose_mid(x: &mut Cx, side: u8) {
    let r = random(x) & 0xf;
    let index = (side as u32 - 1) * 2 + 4 * (x.u8(PHASE) as i8 as i32 as u32);
    // les six routines lisent la même table d'octets
    let _ = target(x, 0x857ac, index);
    let pick = x.rom8(0x857e2 + r);
    let t = target(x, 0x857f2, pick as u32);
    apply(x, t);
}

/// 0x857f6 : un joueur est loin (côté `side`).
pub(super) fn choose_far(x: &mut Cx, side: u8) {
    let r = random(x) & 0xf;
    let index = (side as u32 - 1) * 2 + 4 * (x.u8(PHASE) as i8 as i32 as u32);
    let routine = target(x, 0x8581a, index);
    // six routines : les deux premières lisent 0x85850, les deux suivantes 0x85860, les deux dernières 0x85870
    let table = match routine {
        0x85826 | 0x8582c => 0x85850,
        0x85832 | 0x85838 => 0x85860,
        _ => 0x85870,
    };
    let pick = x.rom8(table + r);
    let t = target(x, 0x85880, pick as u32);
    apply(x, t);
}
