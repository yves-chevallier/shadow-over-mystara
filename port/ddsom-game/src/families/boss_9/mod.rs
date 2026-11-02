//! Boss de type 9 (0x8a75c) : l'engin de guerre roulant de la scène 2 (h), avec son équipage. Voir
//! `docs/families/boss.md`.
//!
//! L'engin roule d'un bord à l'autre de l'écran (plan de la caméra `A5+0x556`) ; ses parties (ESHL 0xd et 0xe) et
//! son équipage (ENEMY 0x13 : un conducteur `+0xac` et quatre servants `+0xae..+0xb4`) sont des objets à part. Il
//! n'a pas d'IA de combat : il passe, s'arrête, tire (ses parties font le travail, `+0xa8` / `+0xa9` : demandes et
//! dégâts), et l'équipage perdu est remplacé. Ses coups reçus ne font que le secouer ; détruit, il quitte l'écran
//! et la scène continue.
//!
//! | État | `+5` | Fichier | Rôle |
//! |---|---|---|---|
//! | 0 | | `mod` | attend la place dans les pools ESHL et ENEMY, puis crée parties et conducteur |
//! | 2 | 0 | `moves` | entrée en scène (dialogue, traversée de l'écran) |
//! | 2 | 2, 4 | `moves` | passages d'un bord à l'autre, arrêts, tirs |
//! | 2 | 6 | `moves` | attente tant qu'aucun joueur n'est en vie |
//! | 4 | 0 | `hit` | coup reçu : secousse, puis reprise |
//! | 4 | 2 | `hit` | détruit : sortie de l'écran, dialogue, chute |
//!
//! Animations : 0x17a276.

use crate::families::cx::Bench;
use crate::objects::Current;
use super::boss_common::*;
use crate::objects::Obj;

mod hit;
mod moves;

/// Champs propres à ce type.
pub(self) mod g {
    /// Compteur des bouffées de fumée (`+0x42` : et logique par un masque).
    pub const PUFFS: u32 = 0x42;
    /// Clignotement après la perte de l'équipage : durée, en cours.
    pub const BLINK_TIME: u32 = 0x43;
    pub const BLINKING: u32 = 0x5c;
    /// Fumée de l'avarie : posée une fois (`+0x5d`), au bout de `+0x78` images (mot).
    pub const SMOKE_DONE: u32 = 0x5d;
    pub const SMOKE_DELAY: u32 = 0x78;
    /// Bulle de dialogue en cours (mot, objet SET).
    pub const MESSAGE: u32 = 0x68;
    /// Passages restants, attente avant de repartir.
    pub const PASSES: u32 = 0x6b;
    pub const WAIT: u32 = 0x6e;
    /// État à reprendre après un coup (long).
    pub const RESUME: u32 = 0x8a;
    /// Secousse après un coup (images).
    pub const SHAKE: u32 = 0xa0;
    /// Allure des roues (0 arrêt, 1 rapide, 2 au pas).
    pub const GAIT: u32 = 0xa1;
    /// Minuterie (mot).
    pub const TIMER: u32 = 0xa2;
    /// Avaries (bits posés par paliers de PV : 0x8b9ea), demandes aux parties (bit 0 tir, 3 et 4 bombardement).
    pub const DAMAGE: u32 = 0xa8;
    pub const ORDERS: u32 = 0xa9;
    /// Équipage à créer (3 à l'entrée), puis 4 quand il n'y a plus de servants.
    pub const CREW_LOST: u32 = 0xaa;
    /// Ruées restantes.
    pub const DASHES: u32 = 0xab;
    /// Conducteur et servants (mots).
    pub const DRIVER: u32 = 0xac;
    pub const CREW: u32 = 0xae;
    /// Servants actifs : bits, et nombre × 4.
    pub const CREW_BITS: u32 = 0xb6;
    pub const CREW_COUNT: u32 = 0xb7;
    /// Abscisse visée par l'arrêt (mot).
    pub const STOP_X: u32 = 0xb8;
    /// Compteurs : bombardements (`+0xb9`), arrêts (`+0xba`), descente/montée (`+0xbe`), attente du tir (`+0xbd`),
    /// équipage avant clignotement (`+0xbf`).
    pub const BOMBS: u32 = 0xb9;
    pub const STOPS: u32 = 0xba;
    pub const READY: u32 = 0xbd;
    pub const COUNT: u32 = 0xbe;
    pub const LIVES: u32 = 0xbf;
}
use g::*;
use crate::objects::pools;

/// Jeu d'animations.
pub(self) const SET: u32 = 0x17a276;

/// Caméra du plan de l'engin (X, Y : `A5+0x556`, `A5+0x55a`).
pub(self) const CAM_X: u32 = 0x556;
pub(self) const CAM_Y: u32 = 0x55a;
/// Places libres des pools ENEMY et ESHL (mots).
pub(self) const ENEMY_FREE: u32 = 0x7e62;
pub(self) const ESHL_FREE: u32 = 0x7e64;
/// Mode « dur » (mot non nul) : vitesses de la table rapide.
pub(self) const HARD: u32 = 0x70;


/// Un pas d'animation (0x1a56).
pub(self) fn anim<R: Bench>(x: &mut Cx<'_, R>) {
    x.anim_step();
}

/// Étape suivante (`addq.b #2,(+6)` ou `(+7)`).
pub(self) fn next6<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(f::SUB2, 2);
}
pub(self) fn next7<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(0x7, 2);
}
pub(self) fn steps<R: Bench>(x: &mut Cx<'_, R>, s6: u8, s7: u8) {
    x.set8(f::SUB2, s6);
    x.set8(0x7, s7);
}

/// `bchg.b #1,(0xd,A0)` : l'engin cahote.
pub(self) fn jolt<R: Bench>(x: &mut Cx<'_, R>) {
    let v = x.u8(0xd) ^ 2;
    x.set8(0xd, v);
}

/// Une image sur `mask` + 1 (`+0x42`), vrai.
pub(self) fn every<R: Bench>(x: &mut Cx<'_, R>, mask: u8) -> bool {
    let n = x.u8(PUFFS).wrapping_add(1) & mask;
    x.set8(PUFFS, n);
    n == 0
}

/// Deux fumées (0xb10c8) : sorte `kind`, décalage `dx`.
pub(self) fn smoke_pair<R: Bench>(x: &mut Cx<'_, R>, kind: u32, dx: u16) {
    for (dy, dz) in [(0x30, 0x28), (0x28, 8)] {
        lib::shock(x, kind, dx, dy, dz);
    }
}

/// 0x8ca22 : fumée de l'engin qui roule.
pub(self) fn smoke<R: Bench>(x: &mut Cx<'_, R>) {
    smoke_pair(x, 1, 0x100)
}

/// Une fumée toutes les `mask` + 1 images.
pub(self) fn smoke_every<R: Bench>(x: &mut Cx<'_, R>, mask: u8) {
    if every(x, mask) {
        smoke(x);
    }
}

/// Fumée plus serrée quand l'équipage est réduit (plus de deux servants : une image sur 4, sinon sur 8).
pub(self) fn crew_smoke<R: Bench>(x: &mut Cx<'_, R>) {
    let mask = if (x.u8(CREW_COUNT) as i8) > 8 { 3 } else { 7 };
    smoke_every(x, mask);
}

/// L'engin est-il sorti par le bord vers lequel il roule (`+0x2c` : 0 vers la gauche) ?
pub(self) fn past_edge<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    let cam = x.a5_u16(CAM_X);
    if x.u8(f::FLIP) == 0 {
        left_of(x, 0x80, cam)
    } else {
        right_of(x, 0x80, cam.wrapping_add(0x180))
    }
}

/// `x + d - edge` négatif (`sub.w ; bge`) : l'engin est à gauche du bord.
pub(self) fn left_of<R: Bench>(x: &mut Cx<'_, R>, d: u16, edge: u16) -> bool {
    let a = x.u16(0x8).wrapping_add(d);
    Fl::sub16(a, edge).lt()
}

/// `x - d - edge` positif (`sub.w ; ble`) : l'engin est à droite du bord.
pub(self) fn right_of<R: Bench>(x: &mut Cx<'_, R>, d: u16, edge: u16) -> bool {
    let a = x.u16(0x8).wrapping_sub(d);
    Fl::sub16(a, edge).gt()
}

/// Plus de servants assez longtemps (`+0xbf` négatif) : l'engin clignote, et repart d'un passage.
pub(self) fn crew_gone<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if (x.u8(LIVES) as i8) >= 0 {
        return false;
    }
    steps(x, 0, 2);
    x.set8(0, 2);
    x.set8(BLINK_TIME, 0x3c);
    x.set8(BLINKING, 1);
    reset_lives(x);
    true
}

/// Fin d'un arrêt : clignotement si l'équipage est perdu, sinon nouveau passage (`+6` = 2, `+7` = 4).
pub(self) fn stop_over<R: Bench>(x: &mut Cx<'_, R>) {
    if !crew_gone(x) {
        steps(x, 2, 4);
    }
}

// --- tables par difficulté (données de la ROM) ----------------------------------------------------

fn difficulty<R: Bench>(x: &mut Cx<'_, R>) -> u32 {
    var::RANK.get(x.b) as u32
}

/// Octet de la table à une entrée par difficulté.
fn per_level<R: Bench>(x: &mut Cx<'_, R>, table: u32) -> u8 {
    let d = difficulty(x);
    x.data8(table + d)
}

/// Octet tiré (`mask`) dans la table de la difficulté, dont l'adresse est dans la table de longs `lists`.
fn draw<R: Bench>(x: &mut Cx<'_, R>, lists: u32, mask: u32) -> u8 {
    let d = difficulty(x);
    let at = x.data32(lists + 4 * d);
    let r = rand_and(x, mask);
    x.data8(at + r)
}

/// 0x8c9ee : équipage perdu avant clignotement.
pub(self) fn reset_lives<R: Bench>(x: &mut Cx<'_, R>) {
    let n = per_level(x, 0x8ca02).wrapping_sub(1);
    x.set8(LIVES, n);
}

/// 0x8c9ba : attente avant le tir.
pub(self) fn reset_count<R: Bench>(x: &mut Cx<'_, R>) {
    let n = per_level(x, 0x8c9ce).wrapping_add(5);
    x.set8(COUNT, n);
}

/// 0x8c420 : ruées autorisées.
pub(self) fn reset_dashes<R: Bench>(x: &mut Cx<'_, R>) {
    let n = per_level(x, 0x8c430);
    x.set8(DASHES, n);
}

/// 0x8beca : passages avant un arrêt.
pub(self) fn draw_passes<R: Bench>(x: &mut Cx<'_, R>) {
    let n = draw(x, 0x8bef0, 0x1f).wrapping_sub(1);
    x.set8(PASSES, n);
}

/// 0x8c450 : bombardements.
pub(self) fn draw_bombs<R: Bench>(x: &mut Cx<'_, R>) {
    let n = draw(x, 0x8c476, 0xf).wrapping_sub(1);
    x.set8(BOMBS, n);
}

/// 0x8c576 : arrêts.
pub(self) fn draw_stops<R: Bench>(x: &mut Cx<'_, R>) {
    let n = draw(x, 0x8c598, 0xf);
    x.set8(STOPS, n);
}

/// 0x8bc0c : attente avant de repartir, selon le nombre de joueurs, la difficulté et l'équipage.
pub(self) fn reset_wait<R: Bench>(x: &mut Cx<'_, R>) {
    let players = var::PLAYERS_ALIVE.get(x.b) as u32;
    let list = x.data32(0x8bc3e + 4 * players);
    let d = difficulty(x);
    let mut n = x.data8(list + d).wrapping_add(0xf);
    if (x.u8(CREW_COUNT) as i8) >= 8 {
        n = n.wrapping_add(0xf);
    }
    x.set8(WAIT, n);
}

/// 0x8bcd2 : abscisse de l'arrêt, tirée devant l'engin.
pub(self) fn draw_stop_x<R: Bench>(x: &mut Cx<'_, R>) {
    let d = difficulty(x);
    let list = (x.rom16(0x8bd08 + 4 * d) as u32) << 16 | x.rom16(0x8bd08 + 4 * d + 2) as u32;
    let r = rand_and(x, 0x1f);
    let k = x.data8(list + r) as u16;
    let cam = x.a5_u16(CAM_X);
    let goal = if x.u8(f::FLIP) != 0 { cam.wrapping_add(k) } else { cam.wrapping_add(0x180).wrapping_sub(k) };
    x.set16(STOP_X, goal);
}

/// 0x8be88 : profondeur du passage selon celle de la cible (trois voies).
pub(self) fn pick_lane<R: Bench>(x: &mut Cx<'_, R>) {
    let lane: u32 = if x.u16(f::TARGET) == 0 {
        1
    } else {
        let t = load_target(x);
        let z = t.u16(x.b, 0x10) as i16;
        if z > 0x60 {
            0
        } else if z > 0x48 {
            1
        } else {
            2
        }
    };
    let z = x.data16(0x8bec4 + 2 * lane);
    x.set16(0x10, z);
}

/// 0x8c988 : nouvelle cible (0x424e ou 0x44ee, au hasard).
pub(self) fn retarget<R: Bench>(x: &mut Cx<'_, R>) {
    let r = rand_and(x, 0xf);
    let k = x.data8(0x8c9aa + r);
    if k == 0 {
        lib::pick_target(x);
    } else {
        lib::random_player(x);
    }
}

/// 0x8bff0 : compte les servants actifs (état 0x0200) : bits en `+0xb6`, nombre × 4 en `+0xb7` ; aucun : `+0xaa` = 4.
pub(self) fn count_crew<R: Bench>(x: &mut Cx<'_, R>) {
    let (mut bits, mut count) = (0u8, 0u8);
    for k in 0..4u32 {
        let link = x.u16(CREW + 2 * k);
        let m = Obj(link as i16 as i32 as u32 & 0xff_ffff);
        if m.u16(x.b, 4) == 0x0200 {
            count = count.wrapping_add(4);
            bits |= 1 << k;
        }
    }
    *x.c.d3() = bits as u32; // relu : D3 relu par la suite (DDSOM_POISON : covb_long3p_02)
    *x.c.d4() = 4;
    *x.c.d5() = 0xffff;
    x.set8(CREW_BITS, bits);
    x.set8(CREW_COUNT, count);
    if count == 0 {
        x.set8(CREW_LOST, 4);
    }
}

/// 0x8c026 : vitesse de roulement selon l'équipage (table rapide en mode dur), allure des roues.
pub(self) fn set_speed<R: Bench>(x: &mut Cx<'_, R>) {
    let hard = x.a5_u16(HARD) != 0;
    let mut top = if hard { 0x800u16 } else { 0x680 };
    if x.u8(f::FLIP) == 0 {
        top = top.wrapping_neg();
    }
    if top == x.u16(0x14) {
        return;
    }
    let i = x.u8(CREW_COUNT) as u32 * 2;
    let table = if hard { 0x8c092 } else { 0x8c0ba };
    let mut vx = x.rom16(table + i);
    let mut vz = x.rom16(table + 2 + i);
    let gait = x.rom8(table + 5 + i);
    *x.c.d2() = gait as u32;
    if x.u8(f::FLIP) == 0 {
        vx = vx.wrapping_neg();
        vz = vz.wrapping_neg();
    }
    x.set8(GAIT, gait);
    x.set16(0x14, vx);
    x.set16(0x16, vz);
}

/// 0x8c1a0 : ruée (équipage au complet, tirage par joueurs et difficulté) : pleine vitesse ; vrai si lancée.
pub(self) fn try_dash<R: Bench>(x: &mut Cx<'_, R>) -> bool {
    if x.u8(CREW_COUNT) != 0x10 || x.u8(DASHES) == 0 {
        return false;
    }
    let players = var::PLAYERS_ALIVE.get(x.b) as u32;
    let list = x.data32(0x8c20c + 4 * players);
    let d = difficulty(x);
    let chances = x.data32(list + 4 * d);
    *x.c.d2() = chances;
    let r = rand_and(x, 0x1f);
    if chances >> r & 1 == 0 {
        return false;
    }
    x.set8(GAIT, 1);
    let mut v = if x.a5_u16(HARD) != 0 { 0x800u16 } else { 0x680 };
    if x.u8(f::FLIP) == 0 {
        v = v.wrapping_neg();
    }
    x.set16(0x14, v);
    x.add8(DASHES, 0xff);
    true
}

/// 0x8c124 / 0x8c132 : remplace les servants perdus (ENEMY 0x13, variante 2 sans aucun servant, 1 sinon).
pub(self) fn refill_crew<R: Bench>(x: &mut Cx<'_, R>, all: bool) {
    if all {
        if x.u8(CREW_COUNT) == 0x10 {
            return;
        }
    } else if (x.a5_u16(ENEMY_FREE) as i16) < 4 || (x.u8(CREW_COUNT) as i8) > 4 {
        return;
    }
    let mut slot = 5u8;
    for k in 0..4u32 {
        *x.c.d4() = k;
        if x.u8(CREW_BITS) >> k & 1 == 0 {
            x.set16(CREW + 2 * k, 0);
            if let Some(m) = x.alloc(&pools::ENEMY) {
                let variant = if x.u8(CREW_COUNT) == 0 { 2 } else { 1 };
                m.set_u8(x.b, 3, variant);
                m.set_u8(x.b, 0, 1);
                m.set_u8(x.b, 0xbc, slot);
                m.set_u8(x.b, 2, 0x13);
                m.set_u16(x.b, 0x42, x.o.0 as u16);
                x.set16(CREW + 2 * k, m.0 as u16);
            }
        }
        slot = slot.wrapping_sub(1);
    }
    *x.c.d2() = 0xffff;
    *x.c.d4() = 4;
    *x.c.d5() = slot as u32;
}

/// 0x8c8d2 : une cible au sol devant l'engin ? 1 tout près, 0 à portée de tir, 2 sinon.
pub(self) fn target_range<R: Bench>(x: &mut Cx<'_, R>) -> u8 {
    let r = if x.u16(f::TARGET) == 0 {
        2
    } else {
        let t = load_target(x);
        if t.u16(x.b, 0xc) != 0 {
            2
        } else if lib::target_in_box(x, -0x40, 0x40, 0x100) {
            1
        } else if lib::target_in_box(x, -0x100, 0x80, 0x100) {
            0
        } else {
            2
        }
    };
    r
}

/// 0x8c698 / 0x8c6a0 : arrêt de tir tiré par difficulté (`base` : 2 cible proche, 0xe cible à portée).
pub(self) fn draw_halt<R: Bench>(x: &mut Cx<'_, R>, base: u8) {
    *x.c.d2() = base as u32;
    let level = (var::RANK.get(x.b) >> 1) << 5;
    let at = 0x8c6d2u32.wrapping_add(level as i16 as u32);
    let r = rand_and(x, 0x1f);
    let k = x.data8(at + r).wrapping_add(base);
    x.set8(f::SUB2, k);
    x.set16(f::STATE, 0x0204);
    x.set8(0x7, 0);
}

/// 0x8b9ea : avaries par paliers de huitièmes de PV (bits de `+0xa8`, cumulés).
pub(self) fn damage_bits<R: Bench>(x: &mut Cx<'_, R>) {
    let q = x.u16(0x60) >> 3;
    let hp = x.u16(f::HP) as i16;
    let mut stage = 7u8;
    let mut d1 = q.wrapping_mul(6);
    for k in 0..7u8 {
        if (d1 as i16) < hp {
            stage = k;
            break;
        }
        if k < 6 {
            d1 = d1.wrapping_sub(q);
        }
    }
    const MASKS: [u8; 8] = [0, 0x40, 0xc0, 0xc4, 0xcc, 0xdc, 0xdd, 0xdd];
    let v = x.u8(DAMAGE) | MASKS[stage as usize];
    x.set8(DAMAGE, v);
}

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    match x.u8(f::STATE) {
        st::INIT => init(x),
        st::ACTIVE => active(x),
        st::HIT => hit::run(x),
        _ => x.free(&pools::BOSS),
    }
}

// --- état 0 : création --------------------------------------------------------------------------

/// 0x8a772 : hors cinématique, et dès qu'il y a la place (10 ESHL, 5 ENEMY) : parties, conducteur, position.
fn init<R: Bench>(x: &mut Cx<'_, R>) {
    if var::PAUSE.get(x.b) != 0 {
        return;
    }
    let r = random(x);
    x.set8(PUFFS, r as u8);
    if (x.a5_u16(ESHL_FREE) as i16) < 0xa || (x.a5_u16(ENEMY_FREE) as i16) < 5 {
        return;
    }
    x.add8(f::STATE, 2);
    x.set_a5_u8(0x50, 1);
    crate::mem::wr16(x.b, 0xff8000 - 0x55ba, x.o.0 as u16);
    lib::clear_popup(x);
    lib::empty_popups(x);
    lib::clear_all_popups(x);
    lib::create_set_15(x);
    x.set8(0x93, 1);
    x.set8(0x9a, 1);
    x.set8(0x6f, 2);
    x.set8(f::BOX_SET, 0x29);
    lib::initial_hp_boss(x);
    lib::set_immunities(x);
    // deux parties 0xd (liées par `+0xa4`, `+0xa6`), huit parties 0xe
    for (kind, last, linked) in [(0xdu8, 1u8, true), (0xe, 7, false)] {
        for k in (0..=last).rev() {
            *x.c.d5() = k as u32;
            let Some(p) = x.alloc(&pools::ESHL) else { break };
            let active = x.u8(0);
            p.set_u8(x.b, 0, active);
            p.set_u8(x.b, 3, k);
            p.set_u8(x.b, 2, kind);
            p.set_u16(x.b, f::OWNER, x.o.0 as u16);
            if linked {
                x.set16(0xa4 + 2 * (last - k) as u32, p.0 as u16);
            }
        }
    }
    if let Some(d) = x.alloc(&pools::ENEMY) {
        let active = x.u8(0);
        d.set_u8(x.b, 0, active);
        d.set_u8(x.b, 0xbc, 6);
        d.set_u8(x.b, 3, 1);
        d.set_u8(x.b, 2, 0x13);
        d.set_u16(x.b, 0x42, x.o.0 as u16);
        x.set16(DRIVER, d.0 as u16);
    }
    x.set8(0x7e, 0x21);
    x.set16(SMOKE_DELAY, 0x300);
    for off in [SMOKE_DONE, DAMAGE, ORDERS, CREW_LOST, 0x38, 0x67] {
        x.set8(off, 0);
    }
    x.set8(GAIT, 2);
    x.set8(0, 2);
    x.set8(f::FLIP, 0x20);
    x.set8(0x2d, 8);
    x.set16(0x10, 0x48);
    // `move.w #-0x8000,D3 ; swap ; asr.l #16` : moins une demi-unité
    *x.c.d3() = 0xffff_8000;
    let z = x.u32(0x10).wrapping_add(0xffff_8000);
    x.set32(0x10, z);
    x.set16(0x8, 0x80);
    lib::pick_target_any(x);
    x.set32(0xc, 0);
    x.anim_start(SET, 6);
}

// --- état 2 : actif -----------------------------------------------------------------------------

/// 0x8a8c0 : gel, clignotement, un groupe par image, ombre ; sans cible ni joueur : attente.
fn active<R: Bench>(x: &mut Cx<'_, R>) {
    let freeze = x.u8(f::FREEZE);
    if freeze != 0 && (freeze as i8) > 0 {
        x.set8(f::FREEZE, freeze - 1);
    } else {
        if freeze != 0 {
            x.set8(f::FREEZE, 0);
        }
        if var::PAUSE.get(x.b) != 0 {
            return;
        }
        if x.u8(BLINKING) != 0 && sub8(x, BLINK_TIME, 1).mi() {
            x.set8(0, 1);
            x.set8(BLINKING, 0);
            x.set8(BLINK_TIME, 0);
        }
        match x.u8(f::SUB) {
            0 => moves::intro(x),
            2 => moves::start(x),
            4 => moves::fight(x),
            _ => moves::wait_for_players(x),
        }
        lib::collide_terrain(x);
    }
    let rolling = x.u8(GAIT) != 2;
    x.set8(0x3a, rolling as u8);
    if x.u16(f::TARGET) == 0 || var::PLAYERS_ALIVE.get(x.b) == 0 {
        retarget(x);
        x.set32(f::STATE, 0x0206_0000);
    }
    x.reinsert();
}

handlers! {
    0x8a75c "boss_9" #[without] run,
}

use super::cx::handlers;
