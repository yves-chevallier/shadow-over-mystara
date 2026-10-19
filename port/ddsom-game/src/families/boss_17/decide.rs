//! Boss 17 : choix de l'action suivante (changements d'état) et tirages dans les tables de la ROM.
//!
//! Les états sont posés d'un coup comme longs (`+4..+7`) : `0x0202_0400` = actif, groupe 2 (combat), étape 4.
//! Les tables sont lues dans la ROM (espace des opcodes : adressage relatif au PC).

use super::*;

/// Combat : attente de joueurs (groupe 10), marche vers un joueur, charge...
pub(super) const WAIT: u32 = 0x020a_0000;
const STAND: u32 = 0x0202_0000;
const INTERRUPTED: u32 = 0x0202_0200;
const CHASE: u32 = 0x0202_0400;
const CHASE_B: u32 = 0x0202_0600;
const DASH: u32 = 0x0202_0a00;
/// Attaques (groupe 4), numérotées par leur étape `+6`.
const ATTACK_0: u32 = 0x0204_0000;
const ATTACK_2: u32 = 0x0204_0200;
const ATTACK_4: u32 = 0x0204_0400;
const ATTACK_6: u32 = 0x0204_0600;
const ATTACK_10: u32 = 0x0204_0a00;
const ATTACK_12: u32 = 0x0204_0c00;
const ATTACK_14: u32 = 0x0204_0e00;
const ATTACK_16: u32 = 0x0204_1000;

/// 0x9735a : le boss attend (aucun joueur en vie).
pub(super) fn wait(x: &mut Cx) {
    x.set32(f::STATE, WAIT);
}

/// 0x971f4 : debout, immobile et vulnérable.
pub(super) fn stand(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    x.set32(f::STATE, STAND);
}

/// 0x97204 : interrompt l'action en cours (l'état est mémorisé en `+0xa2`).
pub(super) fn interrupt(x: &mut Cx) {
    x.set8(VULNERABLE, 0);
    let state = x.u32(f::STATE);
    x.set32(SAVED_STATE, state);
    x.set32(f::STATE, INTERRUPTED);
}

/// 0x97218 : marche vers un joueur (tir 0x31).
pub(super) fn chase(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    x.set8(SHOT, 0x31);
    x.set32(f::STATE, CHASE);
}

/// 0x9722e : marche vers un joueur (tir 0x32).
pub(super) fn chase_b(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    x.set8(SHOT, 0x32);
    x.set32(f::STATE, CHASE_B);
}

/// 0x97288 : charge.
pub(super) fn dash(x: &mut Cx) {
    x.set8(VULNERABLE, 0);
    x.set32(f::STATE, DASH);
}

/// 0x97296 : attaque 0 (vulnérable).
pub(super) fn attack_0(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    x.set32(f::STATE, ATTACK_0);
}

/// 0x972a6 : attaque 2.
pub(super) fn attack_2(x: &mut Cx) {
    attack(x, ATTACK_2);
}

/// 0x972b4 : attaque 4, invulnérable.
pub(super) fn attack_4_shielded(x: &mut Cx) {
    x.set8(INVULNERABLE, 1);
    attack_4(x);
}

/// 0x972ba : attaque 4.
pub(super) fn attack_4(x: &mut Cx) {
    attack(x, ATTACK_4);
}

/// 0x972c8 : attaque 6.
pub(super) fn attack_6(x: &mut Cx) {
    attack(x, ATTACK_6);
}

fn attack(x: &mut Cx, state: u32) {
    x.set8(VULNERABLE, 0);
    x.set32(f::STATE, state);
}

/// Le boss est-il dans la fenêtre horizontale de la caméra élargie de `-0x10` à `0x190` (0x104a2) ?
/// D0 = bords, D1 = largeur ; rend D0 (0 : dedans, 1 : dehors).
fn off_screen(x: &mut Cx) -> bool {
    x.c.d[0] = (-0x10i32) as u32;
    set_dw(x, 1, 0x190);
    x.call(0x104a2);
    x.c.d[0] & 0xff != 0
}

/// 0x972e0 : attaque 10 si le boss est à l'écran et qu'il lui en reste.
pub(super) fn attack_10(x: &mut Cx) {
    if off_screen(x) || (x.u8(BUDGET_A) as i8) <= 0 {
        return attack_6(x);
    }
    attack(x, ATTACK_10);
}

/// 0x97302 : attaque 12.
pub(super) fn attack_12(x: &mut Cx) {
    if off_screen(x) || (x.u8(BUDGET_B) as i8) <= 0 {
        return attack_6(x);
    }
    attack(x, ATTACK_12);
}

/// 0x97324 : attaque 14 (lâcher d'objets), tant qu'il en reste à lâcher.
pub(super) fn attack_14(x: &mut Cx) {
    if x.u8(BUDGET_C) == 0 {
        return attack_6(x);
    }
    if (x.u8(SPAWNED) as i8) > 0 {
        return chase(x);
    }
    attack(x, ATTACK_14);
}

/// 0x97340 : attaque 16 (fin du ralenti).
pub(super) fn attack_16(x: &mut Cx) {
    if (x.u8(SLOW) as i8) > 0 {
        return chase(x);
    }
    x.set8(STRIKES, 0);
    attack(x, ATTACK_16);
}

/// 0x97252 : la charge si le boss est dans la bande utile de l'écran, sinon l'attaque 4 quand il regarde du bon côté.
pub(super) fn dash_or_attack(x: &mut Cx) {
    let here = x.u16(0x8);
    let mut edge = camera_x(x).wrapping_add(0x38);
    let mut side = 0x20;
    if !Fl::sub16(edge, here).ge() {
        side = 0;
        edge = edge.wrapping_add(0x110);
        if !Fl::sub16(edge, here).le() {
            return dash(x);
        }
    }
    if x.u8(f::FLIP) == side {
        attack_4(x);
    } else {
        dash(x);
    }
}

// --- états de départ ----------------------------------------------------------------------------

/// 0x97162 : variante 0, arrivée en scène.
pub(super) fn start_intro(x: &mut Cx) {
    x.set8(ANIM_KIND, 0x30);
    x.set32(f::STATE, 0x0200_0000);
}

/// 0x97172 : sortie de la cinématique d'arrivée.
pub(super) fn start_appear(x: &mut Cx) {
    x.set8(ANIM_KIND, 0x31);
    x.set32(f::STATE, 0x0200_0200);
}

/// 0x97182 : mort, début de la cinématique de fin.
pub(super) fn start_death(x: &mut Cx) {
    x.set8(ANIM_KIND, 0x32);
    x.set8(SLOW, 0);
    x.set8(0, 2);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set_a5_u8(0x42, 1);
    x.set_a5_u8(0x50, 0);
    let kind = x.u8(0x2);
    set_abs8(x, 0xff2a84, kind);
    set_abs8(x, 0xff2a85, 1);
    let bar = x.u8(0x7e);
    set_abs8(x, 0xff2a86, bar);
    if x.u16(0x54) != 0 {
        let killer = x.linked(0x54);
        killer.set_u8(x.b, 0x18a, 1);
    }
    x.set32(f::STATE, 0x0200_0400);
}

/// 0x971ce : variante 1.
pub(super) fn start_variant1(x: &mut Cx) {
    x.set8(ANIM_KIND, 0x33);
    x.set_a5_u8(0x66, 1);
    x.set32(f::STATE, 0x0200_0600);
}

/// 0x971e4 : variante 2.
pub(super) fn start_variant2(x: &mut Cx) {
    x.set8(ANIM_KIND, 0x34);
    x.set32(f::STATE, 0x0200_0800);
}

// --- tirages ------------------------------------------------------------------------------------

/// Octet de table lu dans la ROM (adressage `(table,PC,Dn)`).
fn table8(x: &mut Cx, table: u32, index: u32) -> u8 {
    x.rom8(table + index)
}

fn table16(x: &mut Cx, table: u32, index: u32) -> u16 {
    x.rom16(table + index)
}

/// 0x96ef0 : délai avant le pas suivant, selon la difficulté.
pub(super) fn walk_delay_fast(x: &mut Cx) {
    let level = x.a5_u16(var::DIFFICULTY) as u32;
    let v = table8(x, 0x96f08, level);
    x.set8(WALK_DELAY, v);
}

/// 0x96efc : idem, table des marches lentes.
pub(super) fn walk_delay_slow(x: &mut Cx) {
    let level = x.a5_u16(var::DIFFICULTY) as u32;
    let v = table8(x, 0x96f28, level);
    x.set8(WALK_DELAY, v);
}

/// 0x96f48 : nombre de pas d'une marche.
pub(super) fn walk_steps_fast(x: &mut Cx) {
    let r = random(x) & 0x1f;
    let v = table8(x, 0x96f6c, r);
    x.set8(WALK_STEPS, v);
}

/// 0x96f5a.
pub(super) fn walk_steps_slow(x: &mut Cx) {
    let r = random(x) & 0x1f;
    let v = table8(x, 0x96f8c, r);
    x.set8(WALK_STEPS, v);
}

/// Index de la table de difficulté : `(difficulté & 0x1e) × k + (joueurs - 1)`.
fn level_index(x: &mut Cx, k: u32) -> u32 {
    let level = (x.a5_u16(var::DIFFICULTY) & 0x1e) as u32 * k;
    let players = x.a5_u8(var::PLAYERS_ALIVE) as u32;
    level + players.saturating_sub(1)
}

/// 0x97364 : nombre de coups avant la charge, selon la difficulté et le nombre de joueurs.
pub(super) fn pick_strike_limit(x: &mut Cx) {
    let i = level_index(x, 2);
    let v = table8(x, 0x97382, i);
    x.set8(STRIKE_LIMIT, v);
}

/// 0x973c2 : durée du ralenti (mot).
pub(super) fn pick_slow_time(x: &mut Cx) {
    let level = (x.a5_u16(var::DIFFICULTY) & 0x1e) as u32 * 4;
    let players = x.a5_u8(var::PLAYERS_ALIVE) as u32;
    let i = level + 2 * players.saturating_sub(1);
    let v = table16(x, 0x973e4, i);
    x.set16(SLOW_TIME, v);
}

/// 0x97464 : mode du ralenti (1 ou 2), tiré dans un masque de bits par difficulté et nombre de joueurs.
pub(super) fn pick_slow_mode(x: &mut Cx) {
    let players = x.a5_u8(var::PLAYERS_ALIVE) as u32;
    let row = players.saturating_sub(1) << 3;
    let level = ((x.a5_u16(var::DIFFICULTY) & 0x18) >> 2) as u32;
    let i = row + level;
    let bit = random(x) & 0x1f & 7;
    let (table, set_means_one) = if x.u8(VARIANT) == 0 { (0x974aa, true) } else { (0x974ca, false) };
    let set = table8(x, table, i) >> bit & 1 != 0;
    x.set8(SLOW, if set == set_means_one { 1 } else { 2 });
}

/// 0x974ea : lâche un nombre d'objets lu dans la table 0x97530 (difficulté, joueurs).
pub(super) fn spawn_by_level(x: &mut Cx) {
    let i = level_index(x, 2);
    let count = table8(x, 0x97530, i) as i8 as i16;
    spawn_ring(x, count);
}

/// 0x97506 : lâche `count + 1` objets autour de la caméra (0x57c8c) ; `+0xaf` compte ceux qui ont pu naître.
pub(super) fn spawn_ring(x: &mut Cx, count: i16) {
    let mut d5 = count as u16;
    loop {
        let i = (d5 as u32).wrapping_mul(4) & 0xffff;
        let dx = table16(x, 0x97570, i);
        let dy = table16(x, 0x97572, i);
        let px = dx.wrapping_add(x.a5_u16(var::CAMERA1_X));
        let py = dy.wrapping_add(x.a5_u16(var::CAMERA1_Y));
        set_dw(x, 0, px);
        set_dw(x, 2, py);
        x.c.d[1] = 0;
        x.call(call::SPAWN_RING);
        if !carry(x) {
            x.add8(SPAWNED, 1);
        }
        if d5 == 0 {
            break;
        }
        d5 = d5.wrapping_sub(1);
    }
}

// --- choix d'attaque d'après les boîtes de portée ------------------------------------------------

/// 0x96fac, 0x96fc8 : un joueur est-il dans la boîte `(d0, d1, d2)` ? Rend 0 sinon, ou 1 / 2 : le côté (0x1047a).
pub(super) fn probe(x: &mut Cx, d0: i16, d1: u16, d2: u16) -> u8 {
    set_dw(x, 0, d0 as u16);
    set_dw(x, 1, d1);
    set_dw(x, 2, d2);
    x.call(ai::TARGET_IN_BOX);
    if zero(x) {
        return 0;
    }
    x.call(0x1047a);
    x.c.d[0] as u8
}

/// 0x96fe4 : un joueur est tout près devant ; la table 0x96ffa choisit l'entrée de 0x9700a.
pub(super) fn choose_close(x: &mut Cx) {
    let r = random(x) & 0xf;
    let pick = table8(x, 0x96ffa, r);
    let target = 0x9700a_i32 + table16(x, 0x9700a, pick as u32) as i16 as i32;
    match target as u32 {
        0x97288 => dash(x),
        t => unreachable!("0x96fe4 : cible {t:x}"),
    }
}

/// 0x9700c : un joueur est devant : selon la phase de combat, une charge ou une attaque.
pub(super) fn choose_front(x: &mut Cx) {
    let r = random(x) & 0xf;
    let phase = x.u8(PHASE) as i8 as i32;
    let entry = x.rom16(0x97026 + (2 * phase) as u32) as i16 as i32;
    let kind = 0x97026 + entry;
    let pick = match kind as u32 {
        0x97030 | 0x97036 => table8(x, 0x97054, r),
        0x9703c | 0x97042 | 0x97048 => table8(x, 0x97064, r),
        t => unreachable!("0x9700c : cible {t:x}"),
    };
    let target = 0x97074_i32 + table16(x, 0x97074, pick as u32) as i16 as i32;
    match target as u32 {
        0x97252 => dash_or_attack(x),
        0x972ba => attack_4(x),
        t => unreachable!("0x9700c : cible {t:x}"),
    }
}

/// 0x97078 : un joueur est à mi-distance ; `side` = 1 ou 2 (0x1047a).
pub(super) fn choose_mid(x: &mut Cx) {
    let r = random(x) & 0xf;
    let pick = table8(x, 0x970aa, r);
    let target = 0x970ba_i32 + table16(x, 0x970ba, pick as u32) as i16 as i32;
    match target as u32 {
        0x97296 => attack_0(x),
        0x972a6 => attack_2(x),
        0x972ba => attack_4(x),
        t => unreachable!("0x97078 : cible {t:x}"),
    }
}

/// 0x970c0 : un joueur est loin ; `side` = 1 ou 2.
pub(super) fn choose_far(x: &mut Cx, side: u8) {
    let r = random(x) & 0xf;
    let entry = (side as u32 - 1) * 2 + 4 * (x.u8(PHASE) as i8 as i32 as u32);
    let which = x.rom16(0x970e4 + entry) as i16 as i32;
    let routine = 0x970e4 + which;
    // dix routines : les quatre premières lisent 0x9713a, les autres 0x9714a
    let table = if (0x970f8..0x97110).contains(&routine) { 0x9713a } else { 0x9714a };
    let pick = table8(x, table, r);
    let target = 0x9715a_i32 + table16(x, 0x9715a, pick as u32) as i16 as i32;
    match target as u32 {
        0x97296 => attack_0(x),
        0x972c8 => attack_6(x),
        0x972e0 => attack_10(x),
        0x97302 => attack_12(x),
        t => unreachable!("0x970c0 : cible {t:x}"),
    }
}
