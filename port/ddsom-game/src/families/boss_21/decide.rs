//! Boss 21 : choix de l'action suivante (changements d'état) et tirages dans les tables de la ROM.
//!
//! Les états sont posés d'un coup comme longs (`+4..+7`) : `0x0202_0400` = actif, groupe 2 (combat), étape 4.
//! Les tables sont lues dans la ROM (espace des opcodes : adressage relatif au PC). Les choix à deux niveaux
//! (octet de la table 1, puis mot de la table 2) donnent l'adresse d'une des routines d'état ci-dessous ;
//! `go` retrouve la routine par cette adresse.

use super::*;

/// Attente : aucun joueur en vie (groupe 10).
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
const ATTACK_16: u32 = 0x0204_1000;

/// 0x9ac32 : le boss attend (aucun joueur en vie).
pub(super) fn wait(x: &mut Cx) {
    x.set32(f::STATE, WAIT);
}

/// 0x9aaf2 : debout, immobile et vulnérable.
pub(super) fn stand(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    x.set32(f::STATE, STAND);
}

/// 0x9ab02 : interrompt l'action en cours (l'état est mémorisé en `+0xa2`).
pub(super) fn interrupt(x: &mut Cx) {
    x.set8(VULNERABLE, 0);
    let state = x.u32(f::STATE);
    x.set32(SAVED_STATE, state);
    x.set32(f::STATE, INTERRUPTED);
}

/// 0x9ab16 : marche vers un joueur (tir 0x31).
pub(super) fn chase(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    x.set8(SHOT, 0x31);
    x.set32(f::STATE, CHASE);
}

/// 0x9ab2c : marche vers un joueur (tir 0x32).
pub(super) fn chase_b(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    x.set8(SHOT, 0x32);
    x.set32(f::STATE, CHASE_B);
}

/// 0x9ab7c : charge.
pub(super) fn dash(x: &mut Cx) {
    x.set8(VULNERABLE, 0);
    x.set32(f::STATE, DASH);
}

/// 0x9ab8a : attaque 0 (vulnérable).
pub(super) fn attack_0(x: &mut Cx) {
    x.set8(VULNERABLE, 1);
    x.set32(f::STATE, ATTACK_0);
}

fn attack(x: &mut Cx, state: u32) {
    x.set8(VULNERABLE, 0);
    x.set32(f::STATE, state);
}

/// 0x9ab9a : attaque 2.
pub(super) fn attack_2(x: &mut Cx) {
    attack(x, ATTACK_2);
}

/// 0x9aba8 : attaque 4, invulnérable.
pub(super) fn attack_4_shielded(x: &mut Cx) {
    x.set8(INVULNERABLE, 1);
    attack_4(x);
}

/// 0x9abae : attaque 4.
pub(super) fn attack_4(x: &mut Cx) {
    attack(x, ATTACK_4);
}

/// 0x9abbc : attaque 6.
pub(super) fn attack_6(x: &mut Cx) {
    attack(x, ATTACK_6);
}

/// 0x9abd4 : attaque 10 tant qu'il en reste.
pub(super) fn attack_10(x: &mut Cx) {
    if (x.u8(BUDGET_A) as i8) <= 0 {
        return attack_6(x);
    }
    attack(x, ATTACK_10);
}

/// 0x9abe8 : attaque 12 tant qu'il en reste.
pub(super) fn attack_12(x: &mut Cx) {
    if (x.u8(BUDGET_B) as i8) <= 0 {
        return attack_6(x);
    }
    attack(x, ATTACK_12);
}

/// 0x9ac18 : attaque 16 (fin du ralenti).
pub(super) fn attack_16(x: &mut Cx) {
    if (x.u8(SLOW) as i8) > 0 {
        return chase(x);
    }
    x.set8(STRIKES, 0);
    attack(x, ATTACK_16);
}

/// 0x9ab50 : la charge si le boss est dans la bande utile de l'écran, sinon l'attaque 4 quand il regarde du bon côté.
pub(super) fn dash_or_attack(x: &mut Cx) {
    let here = x.u16(0x8);
    let mut edge = x.a5_u16(var::CAMERA1_X).wrapping_add(0x38);
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

/// 0x9aac4 : arrivée en scène.
pub(super) fn start_intro(x: &mut Cx) {
    x.set8(ANIM_KIND, 0x30);
    x.set32(f::STATE, 0x0200_0000);
}

/// 0x9aad4 : mort de l'hôte, début de la sortie de scène.
pub(super) fn start_death(x: &mut Cx) {
    x.set8(ANIM_KIND, 0x31);
    x.set32(f::IMMUNITY, 0xffff_ffff);
    x.set_a5_u8(0x5c, 1);
    x.set32(f::STATE, 0x0200_0200);
}

/// 0x9ae76 : l'hôte (`+0x46`) est mort (état 0x0406).
pub(super) fn host_dead(x: &mut Cx) -> bool {
    let host = x.linked(f::HOST);
    host.u16(x.b, f::STATE) == 0x0406
}

/// Fin d'attaque : la mort de l'hôte, puis l'absence de joueurs, l'emportent ; sinon `then`.
pub(super) fn after_attack(x: &mut Cx, then: fn(&mut Cx)) {
    if host_dead(x) {
        return start_death(x);
    }
    if x.a5_u8(var::PLAYERS_ALIVE) == 0 {
        return wait(x);
    }
    then(x);
}

// --- tirages ------------------------------------------------------------------------------------

/// Octet de table lu dans la ROM (adressage `(table,PC,Dn)`).
fn table8(x: &mut Cx, table: u32, index: u32) -> u8 {
    x.rom8(table + index)
}

fn table16(x: &mut Cx, table: u32, index: u32) -> u16 {
    x.rom16(table + index)
}

/// Adresse visée par l'entrée `index` (octets) d'une table de mots relatifs à son début.
fn target(x: &mut Cx, table: u32, index: u32) -> u32 {
    (table as i32 + table16(x, table, index) as i16 as i32) as u32
}

/// 0x9a850 : délai avant le pas suivant, selon la difficulté.
pub(super) fn walk_delay_fast(x: &mut Cx) {
    let level = x.a5_u16(var::DIFFICULTY) as u32;
    let v = table8(x, 0x9a868, level);
    x.set8(WALK_DELAY, v);
}

/// 0x9a85c : idem, table des marches lentes.
pub(super) fn walk_delay_slow(x: &mut Cx) {
    let level = x.a5_u16(var::DIFFICULTY) as u32;
    let v = table8(x, 0x9a888, level);
    x.set8(WALK_DELAY, v);
}

/// 0x9a8a8 : nombre de pas d'une marche.
pub(super) fn walk_steps_fast(x: &mut Cx) {
    let r = random(x) & 0x1f;
    let v = table8(x, 0x9a8cc, r);
    x.set8(WALK_STEPS, v);
}

/// 0x9a8ba.
pub(super) fn walk_steps_slow(x: &mut Cx) {
    let r = random(x) & 0x1f;
    let v = table8(x, 0x9a8ec, r);
    x.set8(WALK_STEPS, v);
}

/// Index de la table de difficulté : `(difficulté & 0x1e) × k + (joueurs - 1)`.
fn level_index(x: &mut Cx, k: u32) -> u32 {
    let level = (x.a5_u16(var::DIFFICULTY) & 0x1e) as u32 * k;
    let players = x.a5_u8(var::PLAYERS_ALIVE) as u32;
    level + players.saturating_sub(1)
}

/// 0x9ac3c : nombre de coups avant l'attaque 16, selon la difficulté et le nombre de joueurs.
pub(super) fn pick_strike_limit(x: &mut Cx) {
    let i = level_index(x, 2);
    let v = table8(x, 0x9ac5a, i);
    x.set8(STRIKE_LIMIT, v);
}

/// 0x9ac9a : durée du ralenti (mot).
pub(super) fn pick_slow_time(x: &mut Cx) {
    let level = (x.a5_u16(var::DIFFICULTY) & 0x1e) as u32 * 4;
    let players = x.a5_u8(var::PLAYERS_ALIVE) as u32;
    let i = level + 2 * players.saturating_sub(1);
    let v = table16(x, 0x9acbc, i);
    x.set16(SLOW_TIME, v);
}

/// 0x9ad3c : mode du ralenti (1 ou 2), tiré dans un masque de bits par difficulté et nombre de joueurs.
pub(super) fn pick_slow_mode(x: &mut Cx) {
    let players = x.a5_u8(var::PLAYERS_ALIVE) as u32;
    let row = players.saturating_sub(1) << 3;
    let level = ((x.a5_u16(var::DIFFICULTY) & 0x18) >> 2) as u32;
    let bit = random(x) & 0x1f & 7;
    let set = table8(x, 0x9ad74, row + level) >> bit & 1 != 0;
    x.set8(SLOW, if set { 2 } else { 1 });
}

/// 0x9ad94 : lâche des objets autour de la caméra (0x57c8c) ; `+0xaf` compte ceux qui ont pu naître.
pub(super) fn spawn_by_level(x: &mut Cx) {
    let i = level_index(x, 2);
    let mut d5 = table8(x, 0x9adda, i) as i8 as i16 as u16;
    loop {
        let i = (d5 as u32).wrapping_mul(4) & 0xffff;
        let dx = table16(x, 0x9ae1a, i);
        let dy = table16(x, 0x9ae1c, i);
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

/// 0x9a90c, 0x9a928 : un joueur est-il dans la boîte `(d0, d1, d2)` ? Rend 0 sinon, ou 1 / 2 : le côté (0x1047a).
pub(super) fn probe(x: &mut Cx, d0: i16, d1: u16, d2: u16) -> u8 {
    if !lib::target_in_box(x, d0, d1, d2) {
        return 0;
    }
    lib::turn_decision(x) as u8
}

/// Exécute la routine d'état dont la table donne l'adresse.
fn go(x: &mut Cx, routine: u32) {
    match routine {
        0x9ab50 => dash_or_attack(x),
        0x9ab7c => dash(x),
        0x9ab8a => attack_0(x),
        0x9ab9a => attack_2(x),
        0x9abae => attack_4(x),
        0x9abbc => attack_6(x),
        0x9abd4 => attack_10(x),
        0x9abe8 => attack_12(x),
        t => unreachable!("boss 21 : routine d'état {t:x}"),
    }
}

/// 0x9a944 : un joueur est devant, à courte portée : la charge.
pub(super) fn choose_close(x: &mut Cx) {
    let r = random(x) & 0xf;
    let pick = table8(x, 0x9a95a, r);
    let routine = target(x, 0x9a96a, pick as u32);
    go(x, routine);
}

/// 0x9a96c : un joueur est devant ; la phase règle le choix.
pub(super) fn choose_front(x: &mut Cx) {
    let r = random(x) & 0xf;
    let phase = x.u8(PHASE) as i8 as i32;
    let entry = table16(x, 0x9a986, (2 * phase) as u32) as i16 as i32;
    let kind = 0x9a986 + entry;
    let pick = match kind as u32 {
        0x9a990 | 0x9a996 => table8(x, 0x9a9b4, r),
        0x9a99c | 0x9a9a2 | 0x9a9a8 => table8(x, 0x9a9c4, r),
        t => unreachable!("0x9a96c : cible {t:x}"),
    };
    let routine = target(x, 0x9a9d4, pick as u32);
    go(x, routine);
}

/// 0x9a9d8 : un joueur est à mi-distance (les deux côtés lisent les mêmes tables).
pub(super) fn choose_mid(x: &mut Cx) {
    let r = random(x) & 0xf;
    let pick = table8(x, 0x9aa0a, r);
    let routine = target(x, 0x9aa1a, pick as u32);
    go(x, routine);
}

/// 0x9aa20 : un joueur est loin ; `side` = 1 ou 2.
pub(super) fn choose_far(x: &mut Cx, side: u8) {
    let r = random(x) & 0xf;
    let phase = x.u8(PHASE) as i8 as i32;
    let entry = (side as i32 - 1) * 2 + 4 * phase;
    let which = table16(x, 0x9aa44, entry as u32) as i16 as i32;
    let routine = 0x9aa44 + which;
    // dix routines : les quatre premières lisent 0x9aa9a, les autres 0x9aaaa
    let table = if (0x9aa58..0x9aa70).contains(&routine) { 0x9aa9a } else { 0x9aaaa };
    let pick = table8(x, table, r);
    let routine = target(x, 0x9aaba, pick as u32);
    go(x, routine);
}
