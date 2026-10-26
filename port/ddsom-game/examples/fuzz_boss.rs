//! Fuzz différentiel des boss réécrits (classe BOSS, table 0x39b12), sur le modèle de `fuzz_enemy.rs` :
//! un objet neuf du type est posé dans le pool BOSS, son état est tiré dans les tables de sauts de l'original
//! (`etats_boss`), les variables que le moteur modifie (caméra, joueurs, drapeaux de scène) sont perturbées
//! à chaque pas, et chaque appel du gestionnaire est comparé à l'original (mode contrôle de `ddsom_game`).
//!
//!   fuzz_boss <roms> <eeprom> <scénario> <images de chauffe> <tours>
//!
//! Variables : `DDSOM_GRAINE=n` (graine), `DDSOM_ENTREE=adresse` (un seul gestionnaire), `DDSOM_PAS=n` (pas par
//! trajectoire, 60), `DDSOM_ETATS=1` (états tirés), `DDSOM_COUVERTURE=fichier` (instructions de l'original vues).

use std::path::Path;

use ddsom_game::mem::{rd16, rd8, wr16, wr32, wr8, A5};
use ddsom_game::objects::pools::{BOSS, ENEMY, POOLS};

struct Alea(u64);

impl Alea {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
}

type Bus = cps2::Cps2Bus;

const Z: &[u8] = &[0];

/// États (`+4`, `+5`, `+6`) que les tables de sauts de l'original acceptent pour un type de boss.
fn etats_boss(kind: u8) -> &'static [(&'static [u8], &'static [u8], &'static [u8])] {
    match kind {
        // dragon en vol : +5 = 0 (étapes +6 de 0 à 14) ou 2 (libération)
        13 => &[(&[0], Z, Z), (&[2], &[0], &[0, 2, 4, 6, 8, 10, 12, 14]), (&[2], &[2], Z), (&[4, 6], Z, Z)],
        // boss 17 : groupes (+5) et étapes (+6) des tables de sauts de l'original
        17 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4, 6, 8]),
            (&[2], &[2], &[0, 2, 4, 6, 8, 10]),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12, 14, 16]),
            (&[2], &[6, 8, 10], &[0, 2]),
            (&[4], &[0], Z),
            (&[4], &[2], &[0, 2, 4]),
            (&[4], &[4], &[0, 2, 4, 6]),
            (&[4], &[6, 8], &[0, 2, 4, 6, 8, 10]),
            (&[4], &[10], Z),
            (&[6], Z, Z),
        ],
        // boss 6 (Manticore)
        6 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4, 6, 8, 10, 12, 14]),
            (&[2], &[2], &[0, 2, 4, 6, 8, 10]),
            (&[2], &[4], &[0, 2, 4, 6, 8]),
            (&[2], &[6, 8, 10, 12], &[0, 2]),
            (&[2], &[14], Z),
            (&[4], &[0], Z),
            (&[4], &[2], &[0, 2, 4]),
            (&[4], &[4], &[0, 2, 4]),
            (&[4], &[6, 8], &[0, 2, 4, 6, 8]),
            (&[4], &[10], Z),
            (&[6], Z, Z),
        ],
        // boss 10 (Nagpa)
        10 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4, 6]),
            (&[2], &[2], &[0, 2, 4, 6]),
            (&[2], &[4], &[0]),
            (&[2], &[4], &[2]),
            (&[2], &[6], &[0, 2]),
            (&[4], &[0], &[0, 2, 4]),
            (&[4], &[2], Z),
            (&[4], &[4], &[0, 2, 4]),
            (&[4], &[6, 8], &[0, 2, 4, 6, 8]),
            (&[4], &[10], Z),
            (&[6], Z, Z),
        ],
        // boss 5 (Ezerhorden)
        5 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 4, 8, 12]),
            (&[2], &[4], &[0, 4]),
            (&[2], &[8, 20], &[0, 4]),
            (&[2], &[12], &[0, 4, 8, 12]),
            (&[2], &[16], &[0, 4]),
            (&[2], &[24], &[0, 4, 8, 12]),
            (&[4], &[0, 2, 4], &[0, 4]),
            (&[4], &[0, 2, 4], &[0, 2, 4, 6, 8, 10, 12, 14, 16]),
            (&[4], &[6], &[0, 2]),
            (&[6], Z, Z),
        ],
        // boss 21
        21 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2]),
            (&[2], &[2], &[0, 2, 4, 6, 8, 10]),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12, 14, 16]),
            (&[2], &[6, 8, 10], &[0, 2]),
            (&[4], &[0], &[0, 2, 4]),
            (&[4], &[2], &[0, 2, 4]),
            (&[4], &[4], &[0, 2, 4]),
            (&[4], &[6, 8], &[0, 2, 4, 6, 8, 10]),
            (&[4], &[10], Z),
            (&[6], Z, Z),
        ],
        // dragons 14 et 19
        14 | 19 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4, 6, 8, 10]),
            (&[2], &[2], &[0, 2, 4, 6, 8]),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10]),
            (&[2], &[6, 8, 10], Z),
            (&[4], &[0, 4], &[0, 2]),
            (&[4], &[2], &[0, 2]),
            (&[4], &[6], &[0, 2, 4]),
            (&[6], Z, Z),
        ],
        // dragons rouges 15 et 20
        15 | 20 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4]),
            (&[2], &[2], &[0, 2]),
            (&[2], &[4], &[0, 2, 4]),
            (&[2], &[6, 8, 10], Z),
            (&[2], &[12], &[0, 2, 4]),
            (&[2], &[4], &[6, 8, 10, 16, 18]),
            (&[2], &[4], &[12, 14]),
            (&[4], &[0], &[0, 2]),
            (&[4], &[2], &[0, 2]),
            (&[4], &[4], &[0, 2, 4, 6, 8]),
            (&[4], &[6], &[0, 2, 4]),
            (&[6], Z, Z),
        ],
        // boss 18 (Lich)
        18 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4, 6]),
            (&[2], &[2], &[0, 2, 4, 6, 8, 10]),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20]),
            (&[2], &[6, 8, 10], &[0, 2]),
            (&[4], &[0], &[0, 2, 4, 6]),
            (&[4], &[2], &[0, 2, 4]),
            (&[4], &[4, 6, 8], &[0, 2, 4]),
            (&[4], &[10], &[0, 2]),
            (&[6], Z, Z),
        ],
        // boss 11 et 12 (serpents)
        11 | 12 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4, 6]),
            (&[2], &[2, 8, 10, 12], Z),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12, 14]),
            (&[2], &[6], Z),
            (&[4], &[0], &[0, 2, 4, 6]),
            (&[4], &[4], &[0, 2, 4]),
            (&[4], &[2, 6, 8, 10], &[0, 2, 4]),
            (&[6], Z, Z),
        ],
        // boss 4 (Chimère)
        4 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4]),
            (&[2], &[2], &[0, 2, 4, 6, 8, 10]),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12]),
            (&[2], &[6, 8, 10, 12], &[0, 2]),
            (&[2], &[14], Z),
            (&[4], &[0], &[0, 2]),
            (&[4], &[2, 4], &[0, 2, 4]),
            (&[4], &[6, 8], &[0, 2, 4, 6]),
            (&[4], &[10], Z),
            (&[6], Z, Z),
        ],
        // boss 8 (guerrier noir 2)
        7 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2]),
            (&[2], &[2], &[0, 2, 4, 6, 8]),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12, 14, 16]),
            (&[2], &[6], &[0, 2, 4, 6]),
            (&[2], &[8], &[0, 2, 4, 6, 8]),
            (&[2], &[10], &[0, 2]),
            (&[2], &[12], Z),
            (&[2], &[14], &[0, 2, 4]),
            (&[4], &[0], Z),
            (&[4], &[2], &[0, 2, 4]),
            (&[4], &[4], &[0, 2, 4, 6, 8]),
            (&[4], &[6], &[0, 2, 4, 6]),
            (&[4], &[8, 10], Z),
            (&[6], Z, Z),
        ],
        8 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4, 6, 8]),
            (&[2], &[2], &[0, 2, 4, 6, 8]),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12, 14, 16, 18]),
            (&[2], &[6], &[0, 2, 4, 6]),
            (&[2], &[8], &[0, 2, 4, 6, 8]),
            (&[2], &[10], &[0, 2]),
            (&[2], &[12], Z),
            (&[2], &[14], &[0, 2, 4]),
            (&[4], &[0], Z),
            (&[4], &[2], &[0, 2, 4]),
            (&[4], &[4], &[0, 2, 4, 6, 8]),
            (&[4], &[6], &[0, 2, 4, 6, 8]),
            (&[4], &[8, 10], Z),
            (&[6], Z, Z),
        ],
        // boss 1 (fauve bondissant)
        1 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4, 6, 8, 10, 12, 14]),
            (&[2], &[2], &[0, 2, 4, 6, 8, 10]),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10]),
            (&[2], &[6, 8, 10], &[0, 2]),
            (&[2], &[12, 14], &[0]),
            (&[4], &[0], &[0, 2]),
            (&[4], &[2], &[0, 2, 4]),
            (&[4], &[4], &[0, 2, 4, 6, 8]),
            (&[4], &[6, 8], &[0, 2, 4, 6]),
            (&[4], &[10], Z),
            (&[6], Z, Z),
        ],
        // boss 3 (créature volante)
        3 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[0, 2, 4, 6, 8]),
            (&[2], &[2], &[0, 2, 4, 6, 8, 10, 12, 14, 16]),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30]),
            (&[2], &[6, 8], &[0, 2, 4, 6]),
            (&[2], &[10, 12, 14], Z),
            (&[4], &[0, 4, 6], &[0, 2, 4]),
            (&[4], &[2], &[0, 2]),
            (&[4], &[10], Z),
            (&[6], Z, Z),
        ],
        // boss 9 (engin roulant)
        9 => &[
            (&[0], Z, Z),
            (&[2], &[0], &[2, 4, 6, 8, 10, 14, 16, 18, 20, 22]),
            (&[2], &[2, 6], Z),
            (&[2], &[4], &[0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26]),
            (&[4], &[0], &[0, 2, 4]),
            (&[4], &[2], &[0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20]),
            (&[6], Z, Z),
        ],
        _ => &[(&[0, 2, 4, 6], &[0, 2, 4], &[0, 2, 4])],
    }
}

/// Étape `+7` valide des boss 11 et 12.
fn etape_11(alea: &mut Alea, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (s4, s5, s6) {
        (2, 2 | 8 | 10 | 12 | 6, _) => 2,
        (2, 4, 0) => 2,
        (2, 4, 2 | 12) => 5,
        (2, 4, 8) => 5,
        (2, 4, _) => 3,
        (4, 4, _) => 2,
        _ => 1,
    };
    2 * (alea.next() % n) as u8
}

/// Étape `+7` valide du boss 8.
fn etape_8(alea: &mut Alea, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (s4, s5, s6) {
        (2, 2, 6) => 3,
        (2, 2, _) => 2,
        (2, 4, 0) => 2,
        (2, 4, 2 | 4 | 6 | 8 | 14 | 16 | 18) => 4,
        (2, 4, 10) => 5,
        (2, 4, 12) => 6,
        (2, 6, _) => 4,
        (2, 8, 0) => 3,
        (2, 8, 2 | 4 | 8) => 4,
        (2, 8, 6) => 6,
        _ => 1,
    };
    2 * (alea.next() % n) as u8
}

/// Étape `+7` valide du boss 7.
fn etape_7(alea: &mut Alea, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (s4, s5, s6) {
        (2, 0, 0) => 4,
        (2, 2, 6) => 3,
        (2, 2, _) => 2,
        (2, 4, 0) => 2,
        (2, 4, 6) => 3,
        (2, 4, 10) => 5,
        (2, 4, 12) => 6,
        (2, 4, _) => 4,
        (2, 6, _) => 4,
        (2, 8, 0) => 3,
        (2, 8, 6) => 6,
        (2, 8, _) => 4,
        _ => 1,
    };
    2 * (alea.next() % n) as u8
}

/// Étape `+7` valide du boss 4.
fn etape_4(alea: &mut Alea, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (s4, s5, s6) {
        (2, 2, 8) => 3,
        (2, 2, 10) => 5,
        (2, 2, _) => 2,
        (2, 4, 0) => 2,
        (2, 4, 8) => 7,
        (2, 4, 12) => 6,
        (2, 4, _) => 5,
        _ => 1,
    };
    2 * (alea.next() % n) as u8
}

/// Boss 4 : paramètres communs du 17, puis ses doubles (ENEMY) et ses champs propres.
fn prep_4(b: &mut Bus, alea: &mut Alea, obj: u32) {
    let pick = |a: &mut Alea, v: &[u8]| v[a.next() as usize % v.len()];
    prep_17(b, alea, obj);
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 3, 0);
    wr8(b, obj + 7, etape_4(alea, s.0, s.1, s.2));
    wr8(b, obj + 0xaa, (alea.next() % 3) as u8);
    wr8(b, obj + 0xab, 2);
    for l in [0xb4u32, 0xb6, 0xb8] {
        let d = ENEMY.slot(alea.next() % ENEMY.slots).0;
        wr16(b, obj + l, d as u16);
        wr8(b, d + 0x33, pick(alea, &[0, 0, 1]));
        wr8(b, d + 0xaf, pick(alea, &[0, 0, 1]));
    }
    wr8(b, obj + 0xb2, (alea.next() % 2) as u8);
    wr8(b, obj + 0xb3, (alea.next() % 2) as u8);
    let mark = rd16(b, obj + 8).wrapping_add((alea.next() % 0x100) as u16).wrapping_sub(0x80);
    wr16(b, obj + 0xbc, mark);
    wr8(b, obj + 0x90, (alea.next() % 4) as u8);
    wr16(b, obj + 0x84, (alea.next() % 4) as u16);
    wr8(b, obj + 0x42, (alea.next() % 16) as u8);
    wr8(b, obj + 0xaf, (alea.next() % 3) as u8);
    wr8(b, obj + 0xad, (alea.next() % 2) as u8);
    wr8(b, obj + 0x30, (alea.next() % 2) as u8);
    wr8(b, A5 + 0x30, (alea.next() % 6 != 0) as u8);
    wr16(b, obj + 0x60, [0x40u16, 0x100, 0x300][alea.next() as usize % 3]);
    wr32(b, obj + 0xa2, [0x0202_0000u32, 0x0202_0400, 0x0202_0600][alea.next() as usize % 3]);
    wr8(b, obj + 0x33, pick(alea, &[0, 1, 3, 4, 0x80, 0x81, 0x84]));
    wr16(b, obj + 0x14, [0u16, 0x200, 0xfe00, 0x900, 0xf700][alea.next() as usize % 5]);
    wr16(b, obj + 0x1a, [0u16, 0xffc0, 0xfff0, 0xffd0][alea.next() as usize % 4]);
}

/// Étape `+7` valide du boss 18.
fn etape_18(alea: &mut Alea, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (s4, s5, s6) {
        (2, 2, 0 | 2 | 10) => 2,
        (2, 2, 8) => 4,
        (2, 4, 0 | 4) => 2,
        (2, 4, 2 | 8 | 10) => 4,
        (2, 4, 6) => 3,
        (2, 4, 12) => 6,
        (2, 4, 14 | 16 | 18) => 5,
        (2, 4, 20) => 4,
        _ => 1,
    };
    2 * (alea.next() % n) as u8
}

/// Étape `+7` valide des dragons 15 et 20.
fn etape_15(alea: &mut Alea, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (s4, s5, s6) {
        (2, 2, 0 | 2) => 2,
        (2, 4, 0) => 7,
        (2, 4, 2) => 13,
        (2, 4, 4) => 10,
        (2, 4, 6 | 16) => 4,
        (2, 4, 8 | 10 | 18) => 2,
        (2, 4, 12) => 8,
        (2, 4, 14) => 4,
        (2, 12, _) => 3,
        _ => 1,
    };
    2 * (alea.next() % n) as u8
}

/// Étape `+7` valide des dragons 14 et 19.
fn etape_19(alea: &mut Alea, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (s4, s5, s6) {
        (2, 2, 0 | 8) => 2,
        (2, 4, 0) => 3,
        (2, 4, 8 | 10) => 4,
        (2, 4, _) => 2,
        _ => 1,
    };
    2 * (alea.next() % n) as u8
}

/// Étape `+7` valide du boss 21 pour un état `(+4, +5, +6)`.
fn etape_21(alea: &mut Alea, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (s4, s5, s6) {
        (2, 0, 0) => 8,
        (2, 0, 2) => 3,
        (2, 2, 8 | 10) => 5,
        (2, 2, _) => 2,
        (2, 4, 0 | 2) => 2,
        (2, 4, 12) => 6,
        (2, 4, _) => 5,
        _ => 1,
    };
    2 * (alea.next() % n) as u8
}

/// Étape `+7` valide du boss 10 pour un état actif `(+4, +5, +6)`.
fn etape_10(alea: &mut Alea, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (s4, s5, s6) {
        (2, 2, 6) => 3,
        (2, 4, 0) => 2,
        (2, 4, 2) => 6,
        _ => 2,
    };
    2 * (alea.next() % n) as u8
}

/// Étape `+7` valide du boss 6 pour un état actif `(+4, +5, +6)`.
fn etape_6(alea: &mut Alea, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (s4, s5, s6) {
        (2, 2, 8) => 3,
        (2, 2, 10) => 5,
        (2, 4, 0) => 2,
        (2, 4, 2) => 5,
        (2, 4, 4) => 5,
        (2, 4, 6) => 5,
        (2, 4, 8) => 7,
        _ => 1,
    };
    2 * (alea.next() % n) as u8
}

/// Étape `+7` valide du boss 17 pour un état `(+4, +5, +6)` (états actifs ; les états touchés n'en ont pas besoin).
fn etape_17(alea: &mut Alea, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (s4, s5, s6) {
        (2, 0, 0) => 7,
        (2, 0, 2) => 4,
        (2, 0, 4) => 28,
        (2, 0, 6 | 8) => 8,
        (2, 2, 0 | 2) => 2,
        (2, 2, 8 | 10) => 5,
        (2, 4, 0 | 2) => 2,
        (2, 4, 12) => 6,
        (2, 4, _) => 5,
        _ => 1,
    };
    2 * (alea.next() % n) as u8
}

/// Préparation propre à chaque type (champs que le créateur ou une autre routine pose).

/// Étapes `+7` valides des boss 1, 3 et 9 (nombre d'entrées des tables de sauts).
fn etape_139(kind: u8, alea: &mut Alea, s4: u8, s5: u8, s6: u8) -> u8 {
    let n: u32 = match (kind, s4, s5, s6) {
        (1, 2, 2, 0 | 2) => 2,
        (1, 2, 2, 4 | 6 | 8) => 6,
        (1, 2, 2, 10) => 5,
        (1, 2, 4, 0..=6) => 2,
        (1, 2, 4, 8) => 3,
        (1, 2, 4, 10) => 5,
        (1, 2, 12, _) => 3,
        (3, 2, 2, 12) => 1,
        (3, 2, 2, _) => 2,
        (3, 2, 4, 4 | 18 | 22 | 26) => 3,
        (3, 2, 4, 16 | 24) => 5,
        (3, 2, 4, 20 | 28) => 4,
        (3, 2, 4, 30) => 7,
        (3, 2, 4, _) => 2,
        (3, 2, 6, 4) => 3,
        (3, 2, 6, _) => 2,
        (3, 2, 8, 0 | 2) => 3,
        (3, 2, 8, _) => 2,
        (3, 2, 12, _) => 3,
        (9, 2, 4, 2 | 18) => 3,
        (9, 2, 4, 6 | 8 | 10 | 20 | 22) => 4,
        (9, 2, 4, _) => 2,
        _ => 1,
    };
    2 * (alea.next() % n) as u8
}

/// Champs des boss 1, 3 et 9 tirés dans des plages plausibles (étapes `+7` comprises).
fn prep_1(b: &mut Bus, alea: &mut Alea, obj: u32) {
    let pick = |a: &mut Alea, v: &[u8]| v[a.next() as usize % v.len()];
    wr8(b, obj + 3, 0);
    wr8(b, obj + 0x2c, if alea.next() % 2 == 0 { 0 } else { 0x20 });
    wr16(b, obj + 0x50, (0x85de + 0x200 * (alea.next() % 4)) as u16);
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 7, etape_139(1, alea, s.0, s.1, s.2));
    wr8(b, obj + 0xaa, (alea.next() % 3) as u8);
    wr8(b, obj + 0xab, 2);
    for off in [0xadu32, 0xae, 0xaf, 0xb2, 0xb3] {
        wr8(b, obj + off, pick(alea, &[0, 0, 1, 2, 5]));
    }
    wr8(b, obj + 0xb4, pick(alea, &[7, 0xf]));
    wr16(b, obj + 0xb0, if alea.next() % 3 == 0 { ENEMY.slot(alea.next() % ENEMY.slots).0 as u16 } else { 0 });
    let x = rd16(b, obj + 8).wrapping_add((alea.next() % 0x100) as u16).wrapping_sub(0x80);
    wr16(b, obj + 0xbc, x);
    let v = rd16(b, obj + 0x10).wrapping_add((alea.next() % 0x40) as u16).wrapping_sub(0x20);
    wr16(b, obj + 0xbe, v);
    wr8(b, obj + 0x90, (alea.next() % 4) as u8);
    wr16(b, obj + 0x84, (alea.next() % 4) as u16);
    wr8(b, obj + 0xa0, pick(alea, &[0, 1, 2, 3, 4, 5, 0x10]));
    wr8(b, obj + 0xa1, pick(alea, &[0, 2, 3, 0xff]));
    wr32(b, obj + 0xa2, [0x0202_0000u32, 0x0202_0400, 0x0202_0600][alea.next() as usize % 3]);
    wr16(b, obj + 0xa6, (alea.next() % 4) as u16);
    wr16(b, obj + 0xa8, (alea.next() % 4) as u16);
    wr8(b, obj + 0x6b, (alea.next() % 3) as u8);
    wr8(b, obj + 0x6e, (alea.next() % 3) as u8);
    wr8(b, obj + 0x49, pick(alea, &[0, 4, 8, 0x4c, 0x4c]));
    wr8(b, obj + 0x57, pick(alea, &[0, 4]));
    wr8(b, obj + 0x53, pick(alea, &[0, 0, 0, 1]));
    wr8(b, obj + 0x33, pick(alea, &[0, 0, 1, 0x81]));
    wr16(b, obj + 0x14, [0u16, 0x200, 0xfe00, 0xf000][alea.next() as usize % 4]);
    wr16(b, obj + 0x18, [0u16, 0x80, 0x100, 0x300, 0xff00, 0xfe00][alea.next() as usize % 6]);
    wr16(b, obj + 0xc, [0u16, 1, 0x30, 0x80][alea.next() as usize % 4]);
    wr16(b, obj + 0x62, [0u16, 1, 10, 40][alea.next() as usize % 4]);
    wr16(b, obj + 0x60, 0x100);
    wr16(b, obj + 0x54, if alea.next() % 2 == 0 { 0 } else { (0x85de + 0x200 * (alea.next() % 4)) as u16 });
    if alea.next() % 2 == 0 {
        let t = 0xff85de + 0x200 * (alea.next() % 4);
        wr16(b, obj + 0x50, t as u16);
        wr8(b, t, 1);
        wr8(b, t + 0x188, 1);
    }
}

fn prep_3(b: &mut Bus, alea: &mut Alea, obj: u32) {
    let pick = |a: &mut Alea, v: &[u8]| v[a.next() as usize % v.len()];
    wr8(b, obj + 3, pick(alea, &[0, 0, 1]));
    wr8(b, obj + 0x2c, if alea.next() % 2 == 0 { 0 } else { 0x20 });
    wr16(b, obj + 0x50, if alea.next() % 8 == 0 { 0 } else { (0x85de + 0x200 * (alea.next() % 4)) as u16 });
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 7, etape_139(3, alea, s.0, s.1, s.2));
    for off in [0xa2u32, 0xa5, 0xb5] {
        wr8(b, obj + off, pick(alea, &[0, 0, 1, 2]));
    }
    wr8(b, obj + 0xa3, 2 * (alea.next() % 3) as u8);
    wr16(b, obj + 0xa6, [0u16, 1, 2, 0x100, 0x2000][alea.next() as usize % 5]);
    wr32(b, obj + 0xaa, [0x0202_0000u32, 0x0202_0400, 0x0208_0004][alea.next() as usize % 3]);
    let x = rd16(b, obj + 8).wrapping_add((alea.next() % 0x40) as u16).wrapping_sub(0x20);
    wr16(b, obj + 0xb0, x);
    let v = rd16(b, obj + 0x10).wrapping_add((alea.next() % 0x20) as u16).wrapping_sub(0x10);
    wr16(b, obj + 0xb2, v);
    for off in [0x6bu32, 0x6e, 0x71] {
        wr8(b, obj + off, (alea.next() % 3) as u8);
    }
    wr16(b, obj + 0x84, (alea.next() % 4) as u16);
    wr8(b, obj + 0x49, pick(alea, &[0, 4, 8, 0x4c]));
    wr8(b, obj + 0x57, pick(alea, &[0, 4]));
    wr8(b, obj + 0x53, pick(alea, &[0, 0, 0, 1]));
    wr8(b, obj + 0x33, pick(alea, &[0, 0, 1, 2, 0x81]));
    wr16(b, obj + 0x14, [0u16, 0x200, 0xfe00, 0x900, 0xf600][alea.next() as usize % 5]);
    wr16(b, obj + 0x18, [0u16, 0x80, 0x400, 0xff00, 0xfc00][alea.next() as usize % 5]);
    wr16(b, obj + 0xc, [0u16, 8, 0x30, 0x50, 0x70, 0xc0, 0xff80][alea.next() as usize % 7]);
    wr16(b, obj + 0x62, [0u16, 1, 10, 40, 0x100][alea.next() as usize % 5]);
    wr16(b, obj + 0x60, 0x100);
    if alea.next() % 2 == 0 {
        let t = 0xff85de + 0x200 * (alea.next() % 4);
        wr16(b, obj + 0x50, t as u16);
        wr8(b, t, 1);
        wr8(b, t + 0x188, 1);
    }
}

fn prep_9(b: &mut Bus, alea: &mut Alea, obj: u32) {
    let pick = |a: &mut Alea, v: &[u8]| v[a.next() as usize % v.len()];
    wr8(b, obj + 0x2c, if alea.next() % 2 == 0 { 0 } else { 0x20 });
    wr16(b, obj + 0x50, if alea.next() % 8 == 0 { 0 } else { (0x85de + 0x200 * (alea.next() % 4)) as u16 });
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 7, etape_139(9, alea, s.0, s.1, s.2));
    for l in [0xacu32, 0xae, 0xb0, 0xb2, 0xb4] {
        let d = if alea.next() % 4 == 0 { 0 } else { ENEMY.slot(alea.next() % ENEMY.slots).0 };
        wr16(b, obj + l, d as u16);
        if d != 0 {
            wr16(b, d + 4, [0x0200u16, 0x0200, 0x0400, 0][alea.next() as usize % 4]);
        }
    }
    wr8(b, obj + 0xaa, pick(alea, &[0, 0, 0, 3, 4]));
    wr8(b, obj + 0xa8, (alea.next() & 0xff) as u8);
    wr8(b, obj + 0xa9, (alea.next() & 0xff) as u8);
    wr8(b, obj + 0xa0, pick(alea, &[0, 0, 1, 5]));
    wr8(b, obj + 0xa1, pick(alea, &[0, 1, 2]));
    wr16(b, obj + 0xa2, [0u16, 1, 0x14, 0xffff][alea.next() as usize % 4]);
    for off in [0xb9u32, 0xba, 0xbd, 0xbe, 0xbf, 0x6b, 0x6e, 0x43, 0x5c, 0x5d, 0xab] {
        wr8(b, obj + off, pick(alea, &[0, 0, 1, 2, 0xff]));
    }
    wr16(b, obj + 0x78, [0u16, 1, 0x300][alea.next() as usize % 3]);
    let v = rd16(b, obj + 8).wrapping_add((alea.next() % 0x40) as u16).wrapping_sub(0x20);
    wr16(b, obj + 0xb8, v);
    wr32(b, obj + 0x8a, [0u32, 0x0202_0000, 0x0204_0002, 0x0204_0400][alea.next() as usize % 4]);
    wr16(b, obj + 0x68, 0);
    wr8(b, obj + 0x53, pick(alea, &[0, 0, 0, 1, 0xff]));
    wr16(b, obj + 0x14, [0u16, 0x80, 0x500, 0xfb00, 0x800, 0xf800][alea.next() as usize % 6]);
    wr16(b, obj + 0x1c, [0x120u16, 0xfee0][alea.next() as usize % 2]);
    wr16(b, obj + 0x10, [0x40u16, 0x48, 0x60, 0x78][alea.next() as usize % 4]);
    wr16(b, obj + 0x62, [0u16, 1, 10, 40, 0x100][alea.next() as usize % 5]);
    wr16(b, obj + 0x60, 0x100);
    wr16(b, obj + 0x54, if alea.next() % 2 == 0 { 0 } else { (0x85de + 0x200 * (alea.next() % 4)) as u16 });
    let v = rd16(b, obj + 8).wrapping_sub((alea.next() % 0x200) as u16);
    wr16(b, A5 + 0x556, v);
    wr16(b, A5 + 0x70, (alea.next() % 2) as u16);
    wr16(b, obj + 0x62, (alea.next() % 0x110) as u16);
    wr8(b, obj + 0xab, pick(alea, &[0, 1, 2]));
    if alea.next() % 2 == 0 {
        // équipage au complet
        for k in 0..4u32 {
            let d = ENEMY.slot(k).0;
            wr16(b, obj + 0xae + 2 * k, d as u16);
            wr16(b, d + 4, 0x0200);
        }
    }
}

fn prep(kind: u8, b: &mut Bus, alea: &mut Alea, obj: u32) {
    match kind {
        1 => prep_1(b, alea, obj),
        3 => prep_3(b, alea, obj),
        9 => prep_9(b, alea, obj),
        _ => {}
    }
    if kind == 17 {
        prep_17(b, alea, obj);
    }
    if kind == 6 {
        prep_6(b, alea, obj);
    }
    if kind == 10 {
        prep_10(b, alea, obj);
    }
    if kind == 5 {
        prep_5(b, alea, obj);
    }
    if kind == 21 {
        prep_21(b, alea, obj);
    }
    if kind == 14 || kind == 19 {
        prep_19(b, alea, obj, kind);
    }
    if kind == 15 || kind == 20 {
        prep_15(b, alea, obj);
    }
    if kind == 18 {
        prep_18(b, alea, obj);
    }
    if kind == 4 {
        prep_4(b, alea, obj);
    }
    if kind == 8 || kind == 7 {
        prep_8(b, alea, obj);
    }
    if kind == 7 {
        let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
        wr8(b, obj + 7, etape_7(alea, s.0, s.1, s.2));
        wr8(b, A5 + 0x30, (alea.next() % 6 != 0) as u8);
        wr16(b, obj + 0x60, [0x40u16, 0x100, 0x300][alea.next() as usize % 3]);
    }
    if kind == 11 || kind == 12 {
        prep_11(b, alea, obj);
    }
    if kind == 13 {
        // minuterie de flottement courte, rang d'étape de la table STOPS (4 valeurs), message de cinématique
        wr8(b, obj + 0xa1, (alea.next() % 4) as u8);
        wr8(b, obj + 0xa0, (alea.next() % 2) as u8);
        wr8(b, obj + 0xa7, (alea.next() % 4) as u8);
        wr16(b, obj + 0xa2, [0, 1, 2, 0x12c][alea.next() as usize % 4]);
        wr8(b, obj + 0xa5, (alea.next() % 2) as u8);
        wr8(b, obj + 7, 2 * (alea.next() % 2) as u8);
        wr8(b, obj + 0x42, (alea.next() % 4) as u8);
        wr16(b, obj + 0xb0, if alea.next() % 2 == 0 { 0 } else { ENEMY.slot(alea.next() % ENEMY.slots).0 as u16 });
    }
}

/// Boss 17 : paramètres d'IA, phase, budgets, cible, liens et variables de scène plausibles.
fn prep_17(b: &mut Bus, alea: &mut Alea, obj: u32) {
    let pick = |a: &mut Alea, v: &[u8]| v[a.next() as usize % v.len()];
    wr8(b, obj + 3, (alea.next() % 3) as u8);
    wr8(b, obj + 0x2c, if alea.next() % 2 == 0 { 0 } else { 0x20 });
    wr16(b, obj + 0x50, (0x85de + 0x200 * (alea.next() % 4)) as u16);
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 7, etape_17(alea, s.0, s.1, s.2));
    wr8(b, obj + 0xaa, (alea.next() % 5) as u8);
    wr8(b, obj + 0xab, 4);
    wr8(b, obj + 0xac, (alea.next() % 0x40) as u8);
    for off in [0xb2u32, 0xb3, 0xb4, 0xaf] {
        wr8(b, obj + off, pick(alea, &[0, 1, 2, 5, 0xff]));
    }
    wr8(b, obj + 0xad, (alea.next() % 4 == 0) as u8);
    wr8(b, obj + 0xbb, pick(alea, &[0, 0, 0, 1, 2]));
    wr8(b, obj + 0xba, (alea.next() % 4) as u8);
    wr16(b, obj + 0xb8, (alea.next() % 4) as u16);
    wr8(b, obj + 0xb5, (alea.next() % 12) as u8);
    wr8(b, obj + 0xb6, (alea.next() % 5) as u8);
    wr8(b, obj + 0xb7, (alea.next() % 6) as u8);
    wr8(b, obj + 0xa0, pick(alea, &[0, 1, 2, 3, 5, 0x30, 0x31, 0x32, 0x33, 0x34]));
    wr8(b, obj + 0xa1, pick(alea, &[0, 1, 2, 3, 5, 0xff]));
    wr16(b, obj + 0xa6, (alea.next() % 6) as u16);
    wr16(b, obj + 0xa8, (alea.next() % 8) as u16);
    wr8(b, obj + 0x6b, (alea.next() % 4) as u8);
    wr8(b, obj + 0x6e, (alea.next() % 4) as u8);
    wr8(b, obj + 0x71, (alea.next() % 4) as u8);
    wr8(b, obj + 0x79, pick(alea, &[0x31, 0x32]));
    wr8(b, obj + 0x49, 4 * (alea.next() % 43) as u8);
    wr8(b, obj + 0x56, (alea.next() % 6) as u8);
    wr8(b, obj + 0x53, pick(alea, &[0, 0, 0, 1]));
    wr8(b, obj + 0x33, pick(alea, &[0, 0, 1, 0x81, 0xff]));
    wr8(b, obj + 0x2d, (alea.next() % 32) as u8);
    wr16(b, obj + 0x14, [0u16, 0x200, 0xfe00, 0x400, 0xfc00][alea.next() as usize % 5]);
    wr16(b, obj + 0x18, [0u16, 0x80, 0x100, 0x120, 0x300, 0xff00, 0xff60, 0xff80][alea.next() as usize % 8]);
    wr16(b, obj + 0x1c, [0u16, 1, 0xff00, 0xff80][alea.next() as usize % 4]);
    wr16(b, obj + 0xc, [0u16, 1, 0x30, 0x50, 0x60][alea.next() as usize % 5]);
    wr16(b, obj + 0x62, [0u16, 1, 10, 40, 100][alea.next() as usize % 5]);
    // liens : message (+0x42, +0xb0) vers un emplacement d'effet ; joueur qui a frappé (+0x54)
    for l in [obj + 0x42, obj + 0xb0] {
        let m = ddsom_game::objects::pools::EFFCT.slot(alea.next() % 3).0;
        wr16(b, l, m as u16);
        wr8(b, m + 0x22, (alea.next() % 2) as u8);
        wr16(b, m + 4, [0x020a, 0x021e, 0x0202, 0][alea.next() as usize % 4]);
    }
    wr16(b, obj + 0x54, if alea.next() % 3 == 0 { 0x85de + 0x200 * (alea.next() % 4) as u16 } else { 0 });
    // variables de scène
    wr8(b, A5 + 0x34, pick(alea, &[13, 0x2c, 0x6f, 5]));
    wr8(b, A5 + 0x35, (alea.next() % 5) as u8);
    wr8(b, A5 + 0x36, (alea.next() % 4 == 0) as u8);
    wr8(b, A5 + 0x2c, pick(alea, &[0, 0xc, 4]));
    wr8(b, A5 + 0x500, (alea.next() % 2) as u8);
    wr16(b, A5 + 0x7f92, (alea.next() % 32) as u16);
    wr8(b, 0xff2a38, pick(alea, &[0, 0xc, 0x10]));
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    for (cx, cy, kx, ky) in [(0x5a6u32, 0x5aau32, 0x506u32, 0x50au32), (0x5ae, 0x5b2, 0x556, 0x55a)] {
        wr16(b, A5 + cx, ox.wrapping_sub((alea.next() % 0x200) as u16).wrapping_add(0x40));
        wr16(b, A5 + cy, oy.wrapping_sub((alea.next() % 0x100) as u16));
        wr16(b, A5 + kx, ox.wrapping_sub((alea.next() % 0x200) as u16).wrapping_add(0x40));
        wr16(b, A5 + ky, [0u16, 0x100, 0x1c0, 0x1c8, 0x1d0][alea.next() as usize % 5]);
    }
    for j in 0..4u32 {
        let q = 0xff85de + 0x200 * j;
        let dx = (alea.next() % 0x200) as u16;
        wr8(b, q, (alea.next() % 4 != 0) as u8);
        wr16(b, q + 8, ox.wrapping_add(dx).wrapping_sub(0x100));
        wr16(b, q + 0x10, oy.wrapping_add((alea.next() % 0x20) as u16).wrapping_sub(0x10));
        wr16(b, q + 0xc, (alea.next() % 4) as u16);
    }
}

/// Boss 6 : mêmes variables de scène que le 17 ; paramètres propres.
fn prep_6(b: &mut Bus, alea: &mut Alea, obj: u32) {
    let pick = |a: &mut Alea, v: &[u8]| v[a.next() as usize % v.len()];
    prep_17(b, alea, obj);
    wr8(b, obj + 3, if alea.next() % 4 == 0 { 1 } else { 0 });
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 7, etape_6(alea, s.0, s.1, s.2));
    wr8(b, obj + 0xaa, (alea.next() % 3) as u8);
    wr8(b, obj + 0xab, 2);
    wr8(b, obj + 0xb2, (alea.next() % 2) as u8);
    wr8(b, obj + 0xb3, (alea.next() % 2) as u8);
    wr16(b, obj + 0xbc, (alea.next() % 0x400) as u16);
    wr16(b, obj + 0x84, (alea.next() % 4) as u16);
    wr8(b, obj + 0x90, (alea.next() % 20) as u8);
    wr8(b, obj + 0x33, pick(alea, &[0, 0, 1, 2, 3, 4, 0x81, 0x82, 0x84, 0xff]));
    wr16(b, obj + 0x46, ENEMY.slot(alea.next() % ENEMY.slots).0 as u16);
    wr8(b, obj + 0x32, (alea.next() % 2) as u8);
    // un tir lié pour les attaques qui le pilotent
    let shot = ddsom_game::objects::pools::ESHL.slot(alea.next() % 4).0;
    wr16(b, obj + 0xb0, shot as u16);
    wr8(b, shot, (alea.next() % 4 != 0) as u8);
    wr8(b, shot + 0x5a, (alea.next() % 2) as u8);
}

/// Boss 5 : mêmes variables de scène que le 17 ; paramètres propres.
fn prep_5(b: &mut Bus, alea: &mut Alea, obj: u32) {
    let pick = |a: &mut Alea, v: &[u8]| v[a.next() as usize % v.len()];
    prep_17(b, alea, obj);
    wr8(b, obj + 3, 0);
    wr8(b, obj + 7, 2 * (alea.next() % 2) as u8);
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    // hors du recul (+6 = 0 ou 4), seul le code 0xa8 (chute en tourbillon) a des étapes
    let code = if s.0 == 4 && s.1 <= 4 && s.2 != 0 && s.2 != 4 { 0xa8 } else { pick(alea, &[0xa8, 0xa8, 0x38, 0x4c, 0]) };
    wr8(b, obj + 0x49, code);
    wr8(b, obj + 0x87, (alea.next() % 3) as u8);
    // état mémorisé (+0xa8) : un état actif valide
    wr32(b, obj + 0xa8, 0x0200_0000 | (pick(alea, &[0, 4, 8, 12, 16, 20, 24]) as u32) << 16);
    wr8(b, obj + 0x90, (alea.next() % 20) as u8);
    wr8(b, obj + 0x91, pick(alea, &[0, 0x80, 0x85, 3]));
    wr8(b, obj + 0x33, pick(alea, &[0, 0, 1, 0xff]));
    wr8(b, obj + 0xa2, pick(alea, &[0, 1, 2, 0x20]));
    wr8(b, obj + 0xa3, (alea.next() % 6) as u8);
    wr8(b, obj + 0xa4, pick(alea, &[0, 1, 2]));
    wr8(b, obj + 0xa5, pick(alea, &[0, 1, 2]));
    wr8(b, obj + 0xa6, (alea.next() % 3) as u8);
    wr8(b, obj + 0xa7, (alea.next() % 2) as u8);
    wr16(b, obj + 0xa0, (alea.next() % 4) as u16);
    wr8(b, obj + 0x56, (alea.next() % 6) as u8);
    wr8(b, A5 + 0x2e, pick(alea, &[0, 4]));
    wr8(b, A5 + 0x2c, pick(alea, &[0, 0xc, 4]));
}

/// Dragons 14 et 19 : variante, hôte (variante 1 de 19), réactions, état mémorisé.
fn prep_19(b: &mut Bus, alea: &mut Alea, obj: u32, kind: u8) {
    let pick = |a: &mut Alea, v: &[u8]| v[a.next() as usize % v.len()];
    prep_17(b, alea, obj);
    let variant = if kind == 19 { (alea.next() % 3 == 0) as u8 } else { 0 };
    wr8(b, obj + 3, variant);
    // la mort du dragon de Nagpa n'a que deux étapes
    if variant != 0 && rd8(b, obj + 4) == 4 && rd8(b, obj + 5) == 6 && rd8(b, obj + 6) == 4 {
        wr8(b, obj + 6, 2);
    }
    // l'arrivée du dragon de Nagpa n'a que trois étapes
    if variant != 0 && rd8(b, obj + 4) == 2 && rd8(b, obj + 5) == 0 && rd8(b, obj + 6) >= 6 {
        wr8(b, obj + 6, 4);
    }
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 7, etape_19(alea, s.0, s.1, s.2));
    wr8(b, A5 + 0x30, (alea.next() % 4 != 0) as u8);
    wr8(b, A5 + 0x2c, pick(alea, &[0, 4, 0xc, 0xd]));
    wr16(b, obj + 0x60, [40u16, 100, 700][alea.next() as usize % 3]);
    wr8(b, obj + 0x49, pick(alea, &[0, 4, 0x60, 0x68, 0x70, 0x78, 0x80, 0x90, 0x94, 0xa8, 0xa8]));
    wr8(b, obj + 0x56, (alea.next() % 6) as u8);
    wr8(b, obj + 0x87, (alea.next() % 4) as u8);
    wr8(b, obj + 0x90, (alea.next() % 20) as u8);
    wr8(b, obj + 0xa0, pick(alea, &[0, 2, 4, 0xff]));
    wr8(b, obj + 0xa1, pick(alea, &[0, 1, 2, 4, 5, 7, 0xff]));
    wr8(b, obj + 0xa3, (alea.next() % 2) as u8);
    wr8(b, obj + 0xa4, (alea.next() % 2) as u8);
    wr8(b, obj + 0xa5, pick(alea, &[0, 0xff]));
    wr16(b, obj + 0xa6, [0u16, 1, 2, 5, 0x20, 0x64, 0x65, 0x80][alea.next() as usize % 8]);
    wr16(b, obj + 0xc, [0x1fu16, 0x20, 0x21, 0, 0x30, 0x60, 0x100, 0xff00][alea.next() as usize % 8]);
    wr8(b, obj + 0xad, (alea.next() % 0x22) as u8);
    wr8(b, obj + 0xac, (alea.next() % 6) as u8);
    wr8(b, obj + 0xae, (alea.next() % 3) as u8);
    wr8(b, obj + 0xaf, (alea.next() % 5) as u8);
    wr8(b, obj + 0xb2, pick(alea, &[0, 0, 1, 2, 0x30]));
    wr8(b, obj + 0xb0, (alea.next() % 5) as u8);
    wr8(b, obj + 0xb1, (alea.next() % 3) as u8);
    // état mémorisé : le souffle (attaque 4) une fois sur trois
    let saved = if alea.next() % 3 == 0 { 0x0204_0400 } else { 0x0202_0000 };
    wr32(b, obj + 0xa8, saved);
    // hôte : un ennemi vivant ou non, avec ou sans PV
    let host = ENEMY.slot(alea.next() % ENEMY.slots).0;
    wr16(b, obj + 0x46, if alea.next() % 4 == 0 { 0 } else { host as u16 });
    wr8(b, host, (alea.next() % 4 != 0) as u8);
    wr16(b, host + 0x62, [0u16, 1, 50][alea.next() as usize % 3]);
    if s.0 == 4 && s.1 == 0 && (s.2 == 2) {
        // l'étape 2 de la réaction ne sert qu'à la glissade (code 0x68, 0x80, 0xa8 ou ordinaire)
    }
    close_players(b, alea, obj);
}

/// Dragons rouges 15 et 20 : positions mémorisées, armure, état précédent, tables du script.
fn prep_15(b: &mut Bus, alea: &mut Alea, obj: u32) {
    let pick = |a: &mut Alea, v: &[u8]| v[a.next() as usize % v.len()];
    prep_17(b, alea, obj);
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 7, etape_15(alea, s.0, s.1, s.2));
    wr8(b, A5 + 0x2c, pick(alea, &[0, 4, 0xc, 0xd]));
    wr16(b, obj + 0x60, [40u16, 100, 700][alea.next() as usize % 3]);
    wr8(b, obj + 0x49, pick(alea, &[0, 4, 8, 0x60, 0xa8]));
    wr8(b, obj + 0x56, (alea.next() % 6) as u8);
    wr16(b, obj + 0x84, (alea.next() % 4) as u16);
    wr8(b, obj + 0x90, (alea.next() % 20) as u8);
    wr8(b, obj + 0xa0, pick(alea, &[0, 2, 4, 0xff]));
    wr8(b, obj + 0xa1, pick(alea, &[0, 1, 2, 4, 5, 0x13, 0x14, 0x15, 0x16, 0x22, 0xff]));
    wr8(b, obj + 0xa4, (alea.next() % 2) as u8);
    wr8(b, obj + 0xa5, (alea.next() % 40) as u8);
    wr16(b, obj + 0xa6, [0u16, 1, 2, 5, 0x20, 0x40, 0xf0][alea.next() as usize % 7]);
    wr16(b, obj + 0xc, [0x1fu16, 0x20, 0x21, 0x30, 0x40, 0x60, 0x2c0, 0x300, 0x380, 0xff00][alea.next() as usize % 10]);
    let (ox, oz, oy) = (rd16(b, obj + 8), rd16(b, obj + 0xc), rd16(b, obj + 0x10));
    wr16(b, obj + 0xa8, ox);
    wr16(b, obj + 0xaa, oz);
    wr16(b, obj + 0xac, oy);
    wr8(b, obj + 0xae, (alea.next() % 3) as u8);
    wr8(b, obj + 0xaf, (alea.next() % 4) as u8);
    wr8(b, obj + 0xb4, (alea.next() % 0x42) as u8);
    wr8(b, obj + 0xb5, (alea.next() % 5) as u8);
    wr8(b, obj + 0xb7, (alea.next() % 3) as u8);
    wr8(b, obj + 0xb8, pick(alea, &[0, 0, 1, 2, 0x10]));
    wr8(b, obj + 0x33, pick(alea, &[0, 0, 1, 0xff]));
    wr8(b, obj + 0x43, (alea.next() % 4 == 0) as u8);
    wr8(b, obj + 0x2d, (alea.next() % 32) as u8);
    // table de vecteurs du souffle (+0x4c) : nulle ou la table des angles de la ROM
    wr32(b, obj + 0x4c, if alea.next() % 2 == 0 { 0 } else { 0x1c6636 });
    wr8(b, A5 + 0x42, (alea.next() % 4) as u8);
    // état de l'image précédente : actif (02) ou autre
    let saved = [0x0202_0000u32, 0x0202_0400, 0x0204_0200, 0x0000_0000][alea.next() as usize % 4];
    wr32(b, obj + 0xb0, saved);
    wr8(b, obj + 0xb9, (alea.next() % 9) as u8);
    close_players(b, alea, obj);
    // joueurs saisissables (+0x188) et 0x91f6c : à la position exacte d'attaque
    for j in 0..4u32 {
        wr8(b, 0xff85de + 0x200 * j + 0x188, (alea.next() % 2) as u8);
    }
    if (s.0, s.1, s.2) == (2, 4, 10) && alea.next() % 2 == 0 {
        let edge = if rd8(b, obj + 0x2c) != 0 { 0x48u16 } else { 0x138 };
        let scroll = rd16(b, A5 + 0x506);
        wr16(b, obj + 8, scroll.wrapping_add(edge));
    }
}

/// Boss 11 et 12 : douze anneaux autour de la tête, pose et rang suivis, codes de coup cohérents avec l'étape.
fn prep_11(b: &mut Bus, alea: &mut Alea, obj: u32) {
    let pick = |a: &mut Alea, v: &[u8]| v[a.next() as usize % v.len()];
    prep_17(b, alea, obj);
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 3, 0);
    wr8(b, obj + 7, etape_11(alea, s.0, s.1, s.2));
    // anneaux : rangs 0 à 11 dans les douze premiers emplacements ESHL, les autres vides
    let esh = &ddsom_game::objects::pools::ESHL;
    for i in 0..esh.slots {
        let o = esh.slot(i).0;
        if i < 12 && s.0 != 0 {
            wr8(b, o, 1);
            wr8(b, o + 2, 5);
            wr8(b, o + 0x2f, i as u8);
            wr16(b, o + 0x46, obj as u16);
            let bx = rd16(b, obj + 8);
            let by = rd16(b, obj + 0x10);
            wr16(b, o + 8, bx.wrapping_add((alea.next() % 0x80) as u16).wrapping_sub(0x40));
            wr16(b, o + 0xc, [0u16, 3, 0x0f, 0xfffd, 0xfff0, 0xffe0][alea.next() as usize % 6]);
            wr16(b, o + 0x10, by.wrapping_add((alea.next() % 0x20) as u16).wrapping_sub(0x10));
            wr8(b, o + 0x42, (alea.next() % 32) as u8);
            wr8(b, o + 0x43, if alea.next() % 3 == 0 { (alea.next() % 32) as u8 } else { 0 });
            wr8(b, o + 3, (alea.next() % 3) as u8);
        } else {
            wr8(b, o, 0);
        }
    }
    match alea.next() % 24 {
        0 => wr8(b, esh.slot(alea.next() % 12).0, 0),
        1 => wr8(b, esh.slot(alea.next() % 12).0 + 0x2f, 0x40),
        2 => wr8(b, esh.slot(12).0, 0), // pas de douzième anneau
        _ => {}
    }
    // pose (ligne de 32 octets de 0x1069f8) et rang suivi, autour du seuil de la ligne
    let aa = (alea.next() % 22) as u8;
    wr8(b, obj + 0xaa, aa);
    let thr = rd8(b, 0x1069f8 + 32 * aa as u32 + 2);
    let seg = match alea.next() % 4 {
        0 => thr,
        1 => thr.wrapping_sub(1),
        2 => thr.wrapping_add(1),
        _ => (alea.next() % 12) as u8,
    };
    wr8(b, obj + 0xa8, if alea.next() % 10 == 0 { 0 } else { seg % 12 });
    wr8(b, obj + 0x2d, (alea.next() % 32) as u8);
    let (bx, by) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    wr16(b, obj + 0xa2, bx.wrapping_add((alea.next() % 8) as u16));
    wr16(b, obj + 0xa4, [0u16, 1, 2, 3, 0x11, 0x40, 0x5f, 0x60, 0xfff0][alea.next() as usize % 9]);
    wr16(b, obj + 0xa6, by.wrapping_add((alea.next() % 4) as u16));
    wr16(b, obj + 0xa0, [0u16, 1, 8, 9, 0x0200, 0x0402, 0xffff][alea.next() as usize % 7]);
    wr8(b, obj + 0xa0, pick(alea, &[0, 0, 0, 2, 4]));
    wr8(b, obj + 0xa1, pick(alea, &[0, 1, 8, 9, 7, 3, 0x24]));
    wr32(b, obj + 0x4c, 0x1c6636);
    wr8(b, obj + 0xb2, (alea.next() % 2) as u8);
    wr8(b, obj + 0xb6, (alea.next() % 4) as u8);
    wr8(b, obj + 0xb7, (alea.next() % 2) as u8);
    wr8(b, obj + 0xb8, (alea.next() % 5) as u8);
    wr8(b, obj + 0xb9, pick(alea, &[0, 0, 1, 2]));
    wr16(b, obj + 0xb0, (alea.next() % 5) as u16);
    if alea.next() % 2 == 0 {
        wr8(b, obj + 0xb0, 1 + (alea.next() % 4) as u8);
    }
    wr8(b, obj + 0xb1, (alea.next() % 4) as u8);
    wr8(b, obj + 0xb3, (alea.next() % 32) as u8);
    wr8(b, obj + 0xb4, (alea.next() % 16) as u8);
    wr8(b, obj + 0xb5, (alea.next() % 6) as u8);
    wr8(b, obj + 0xac, pick(alea, &[0, 1, 0xff]));
    wr8(b, obj + 0xad, (alea.next() % 2) as u8);
    let tgt = ENEMY.slot(alea.next() % ENEMY.slots).0;
    wr8(b, tgt, 1);
    wr8(b, tgt + 2, if alea.next() % 2 == 0 { 0x47 } else { 2 });
    wr16(b, obj + 0x5c, if alea.next() % 2 == 0 { 0 } else { tgt as u16 });
    wr16(b, obj + 0x60, [0x40u16, 0x100, 0x300][alea.next() as usize % 3]);
    wr16(b, obj + 0x62, [0u16, 1, 0x20, 0x60, 0x100, 0x2ff][alea.next() as usize % 6]);
    wr16(b, obj + 0x84, (alea.next() % 4) as u16);
    wr8(b, obj + 0x86, (alea.next() % 2) as u8);
    wr8(b, obj + 0x87, (alea.next() % 4) as u8);
    wr8(b, obj + 0x90, (alea.next() % 4) as u8);
    wr8(b, obj + 0x29, (alea.next() % 2) as u8);
    wr8(b, obj + 0x33, pick(alea, &[0, 0, 1, 0x81, 0xff]));
    wr8(b, obj + 0x3e, (alea.next() % 22) as u8);
    wr8(b, obj + 0x56, (alea.next() % 6) as u8);
    wr8(b, obj + 0x53, pick(alea, &[0, 0, 0, 1]));
    wr16(b, obj + 0x14, [0u16, 0x200, 0xfe00, 0x400, 0xfc00][alea.next() as usize % 5]);
    wr16(b, obj + 0x16, [0u16, 0x20, 0xffe0][alea.next() as usize % 3]);
    wr16(b, obj + 0x18, [0u16, 0x80, 0x100, 0x600, 0xff00, 0xff80][alea.next() as usize % 6]);
    wr16(b, obj + 0x1a, [0u16, 0xff80, 0xffc0, 0xffe0][alea.next() as usize % 4]);
    wr16(b, obj + 0xc, [0u16, 1, 0x10, 0x11, 0x40, 0xfff0][alea.next() as usize % 6]);
    wr8(b, A5 + 0x2c, pick(alea, &[0, 5, 0xb, 0xc, 0xd]));
    wr16(b, A5 + 0x6e, (alea.next() % 8) as u16);
    wr16(b, A5 + 0x7f92, (alea.next() % 32) as u16);
    // code de coup cohérent avec l'étape de réaction
    if s.0 == 4 && s.1 == 0 {
        let codes: &[u8] = match s.2 {
            0 => &[0, 4, 0x1c, 0x4c, 0x68, 0xa4, 0xa8, 0x78, 0x38],
            2 => &[0, 4, 0x1c, 0x4c, 0x68, 0xa4, 0x78],
            4 => &[4, 0xa4, 0x14],
            _ => &[0xa4],
        };
        wr8(b, obj + 0x49, pick(alea, codes));
    }
    close_players(b, alea, obj);
}

/// Boss 8 : variables de scène du 17, paramètres propres.
fn prep_8(b: &mut Bus, alea: &mut Alea, obj: u32) {
    let pick = |a: &mut Alea, v: &[u8]| v[a.next() as usize % v.len()];
    prep_17(b, alea, obj);
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 3, 0);
    wr8(b, obj + 7, etape_8(alea, s.0, s.1, s.2));
    wr8(b, A5 + 0x5a, (alea.next() % 4) as u8);
    wr8(b, obj + 0xad, 1 + (alea.next() % 3) as u8);
    wr8(b, obj + 0x42, (alea.next() % 4 == 0) as u8);
    wr8(b, obj + 0x43, (alea.next() % 4 == 0) as u8);
    wr16(b, obj + 0x62, [0u16, 1, 10, 40, 100, 0x160][alea.next() as usize % 6]);
    wr16(b, obj + 0x18, [0u16, 0x100, 0xfd00, 0xfc00, 0x300][alea.next() as usize % 5]);
    wr8(b, obj + 0x3e, (alea.next() % 3 == 0) as u8);
    wr8(b, obj + 0x4a, pick(alea, &[0, 0x20]));
    wr8(b, obj + 0x46, (alea.next() % 2) as u8);
    wr8(b, obj + 0x87, (alea.next() % 2) as u8);
    wr16(b, obj + 0x60, [0x40u16, 0x100, 0x300][alea.next() as usize % 3]);
    wr16(b, obj + 0xb6, [0u16, 0x20, 0x50, 0x90][alea.next() as usize % 4]);
    wr16(b, obj + 0xb4, [0u16, 1, 0xf0, 0xffff][alea.next() as usize % 4]);
    wr8(b, obj + 0xb8, (alea.next() % 2) as u8);
    wr8(b, obj + 0xb9, (alea.next() % 2) as u8);
    wr8(b, obj + 0xba, (alea.next() % 3) as u8);
    wr8(b, obj + 0xbb, (alea.next() % 4) as u8);
    wr16(b, obj + 0xbc, [0u16, 1, 2, 0x12c][alea.next() as usize % 4]);
    wr16(b, obj + 0xbe, [0u16, 0xffff, 0x3c, 1][alea.next() as usize % 4]);
    wr8(b, obj + 0xab, pick(alea, &[0, 0, 1, 0x10]));
    wr8(b, obj + 0xac, (alea.next() % 4) as u8);
    wr8(b, obj + 0xaa, (alea.next() % 3) as u8);
    wr8(b, obj + 0xae, (alea.next() % 2) as u8);
    wr8(b, obj + 0xaf, 0);
    wr8(b, obj + 0xa2, pick(alea, &[0, 1, 3, 4, 0x7f]));
    wr8(b, obj + 0xa3, pick(alea, &[0, 1, 3, 4, 0x7f]));
    wr8(b, obj + 0xa5, pick(alea, &[0, 2, 4, 6]));
    wr32(b, obj + 0xa6, [0x0202_0000u32, 0x0202_0004, 0x0208_0004, 0x0204_0000][alea.next() as usize % 4]);
    wr16(b, obj + 0xa0, [0u16, 1, 2, 5, 0x14, 0x32][alea.next() as usize % 6]);
    wr16(b, obj + 0x84, (alea.next() % 4) as u16);
    wr8(b, obj + 0x6b, (alea.next() % 4) as u8);
    wr8(b, obj + 0x6e, (alea.next() % 4) as u8);
    wr8(b, obj + 0x71, (alea.next() % 4) as u8);
    wr16(b, A5 + 0x7f92, (alea.next() % 32) as u16);
    wr16(b, A5 + 0x6e, (alea.next() % 8) as u16);
    // les tirs de la rafale visent des joueurs distincts : tous en vie
    if s.0 == 2 && s.1 == 4 && (s.2 == 4 || s.2 == 6 || s.2 == 14) {
        for j in 0..4u32 {
            wr8(b, 0xff85de + 0x200 * j, 1);
        }
        wr8(b, A5 + 0x35, 1 + (alea.next() % 3) as u8);
    }
    close_players(b, alea, obj);
}

/// Boss 18 : phases, budgets, tir lié, codes de coup valides pour l'étape de réaction.
fn prep_18(b: &mut Bus, alea: &mut Alea, obj: u32) {
    let pick = |a: &mut Alea, v: &[u8]| v[a.next() as usize % v.len()];
    prep_17(b, alea, obj);
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 7, etape_18(alea, s.0, s.1, s.2));
    wr8(b, A5 + 0x30, (alea.next() % 5 != 0) as u8);
    wr8(b, obj + 0xaa, (alea.next() % 3) as u8);
    wr8(b, obj + 0xab, 2);
    for off in [0xb2u32, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7, 0xae, 0xaf] {
        wr8(b, obj + off, pick(alea, &[0, 0, 1, 2, 5]));
    }
    wr16(b, obj + 0xa6, [0u16, 1, 2, 5, 0x14][alea.next() as usize % 5]);
    wr16(b, obj + 0xa8, (alea.next() % 6) as u16);
    wr16(b, obj + 0xb8, [0u16, 1, 5, 0x168][alea.next() as usize % 4]);
    wr8(b, obj + 0xba, (alea.next() % 4) as u8);
    wr8(b, obj + 0xbb, (alea.next() % 2) as u8);
    wr8(b, obj + 0x33, pick(alea, &[0, 0, 1, 0x81, 0xff]));
    wr8(b, obj + 0x90, (alea.next() % 20) as u8);
    wr8(b, obj + 0x4a, pick(alea, &[0, 0x20]));
    wr8(b, obj + 0x56, (alea.next() % 6) as u8);
    wr32(b, obj + 0xa2, [0x0202_0000u32, 0x0202_0400, 0x0202_0600, 0x0204_0400][alea.next() as usize % 4]);
    wr8(b, obj + 0xa0, pick(alea, &[0, 1, 2, 3, 4, 5, 0x10, 0x20]));
    wr8(b, obj + 0xa1, pick(alea, &[0, 1, 2, 3, 5, 0xff]));
    // code de coup : toutes les routines de réaction propres (les étapes avancées n'en acceptent que certaines)
    let codes: &[u8] = if s.0 == 4 && s.1 == 0 && s.2 >= 4 {
        &[0x20, 0x38, 0x44, 0x50, 0x5c, 0x9c, 0x8, 0x60]
    } else if s.0 == 4 && s.1 == 0 && s.2 == 2 {
        &[0, 0x1c, 0x20, 0x38, 0x44, 0x50, 0x5c, 0x9c, 0xa8]
    } else {
        &[0, 4, 0x1c, 0x20, 0x38, 0x44, 0x50, 0x5c, 0x9c, 0xa8]
    };
    wr8(b, obj + 0x49, pick(alea, codes));
    let shot = ddsom_game::objects::pools::ESHL.slot(alea.next() % 4).0;
    wr16(b, obj + 0xb0, shot as u16);
    wr8(b, shot, (alea.next() % 4 != 0) as u8);
    wr8(b, shot + 0x5a, (alea.next() % 2) as u8);
    close_players(b, alea, obj);
    for j in 0..4u32 {
        let q = 0xff85de + 0x200 * j;
        wr16(b, q + 0xc, [0u16, 0, 0x10][alea.next() as usize % 3]);
        wr32(b, q + 4, [0x0202_0000u32, 0x0200_2200, 0x0200_1400, 0x0204_0e00][alea.next() as usize % 4]);
        wr8(b, q + 0x66, (alea.next() % 3 == 0) as u8);
        wr8(b, q + 0x8b, (alea.next() % 6) as u8);
    }
}

/// Boss 21 : mêmes variables de scène que le 17 ; hôte (`+0x46`) parfois mort, tir lié.
fn prep_21(b: &mut Bus, alea: &mut Alea, obj: u32) {
    let pick = |a: &mut Alea, v: &[u8]| v[a.next() as usize % v.len()];
    prep_17(b, alea, obj);
    wr8(b, obj + 3, 0);
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 7, etape_21(alea, s.0, s.1, s.2));
    if s.0 == 4 && s.1 == 0 && s.2 != 0 {
        wr8(b, obj + 0x49, 0x4c);
    }
    let host = ENEMY.slot(alea.next() % ENEMY.slots).0;
    wr16(b, obj + 0x46, host as u16);
    wr16(b, host + 4, if alea.next() % 6 == 0 { 0x0406 } else { 0x0202 });
    wr8(b, obj + 0x33, pick(alea, &[0, 0, 1, 0x81, 0xff]));
    wr16(b, obj + 0x84, (alea.next() % 4) as u16);
    wr8(b, obj + 0x90, (alea.next() % 20) as u8);
    wr16(b, obj + 0xa6, (alea.next() % 6) as u16);
    wr8(b, obj + 0xa0, pick(alea, &[0, 1, 2, 3, 5, 0x10, 0x20, 0x30, 0x31]));
    wr8(b, obj + 0xaa, (alea.next() % 5) as u8);
    wr8(b, obj + 0xab, 4);
    wr8(b, A5 + 0x2e, pick(alea, &[0, 4]));
    let shot = ddsom_game::objects::pools::ESHL.slot(alea.next() % 4).0;
    wr16(b, obj + 0xb0, shot as u16);
    wr8(b, shot, (alea.next() % 4 != 0) as u8);
    wr8(b, shot + 0x5a, (alea.next() % 2) as u8);
    close_players(b, alea, obj);
}

/// Joueurs tout près du boss (une fois sur deux) pour atteindre les choix d'attaque selon la portée.
fn close_players(b: &mut Bus, alea: &mut Alea, obj: u32) {
    if alea.next() % 2 == 0 {
        return;
    }
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    for j in 0..4u32 {
        let q = 0xff85de + 0x200 * j;
        wr8(b, q, 1);
        wr16(b, q + 8, ox.wrapping_add((alea.next() % 0x120) as u16).wrapping_sub(0x90));
        wr16(b, q + 0x10, oy.wrapping_add((alea.next() % 0x10) as u16).wrapping_sub(8));
    }
}

/// Boss 10 : mêmes variables de scène que le 17 ; paramètres propres.
fn prep_10(b: &mut Bus, alea: &mut Alea, obj: u32) {
    let pick = |a: &mut Alea, v: &[u8]| v[a.next() as usize % v.len()];
    prep_17(b, alea, obj);
    wr8(b, obj + 3, 0);
    let s = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    wr8(b, obj + 7, etape_10(alea, s.0, s.1, s.2));
    if s.0 == 4 && s.1 == 0 && s.2 != 0 {
        // les étapes 2 et 4 de la réaction sont celles de l'étourdissement (code 0x4c)
        wr8(b, obj + 0x49, 0x4c);
    }
    wr8(b, A5 + 0x2e, pick(alea, &[0, 6, 6]));
    wr16(b, obj + 0x84, (alea.next() % 6) as u16);
    wr8(b, obj + 0x90, (alea.next() % 20) as u8);
    wr8(b, obj + 0xae, pick(alea, &[0, 1, 2, 0xfa]));
    wr16(b, obj + 0xa6, (alea.next() % 5) as u16);
    wr16(b, obj + 0xaa, (alea.next() % 0x400) as u16);
    wr16(b, obj + 0xac, (alea.next() % 0x200) as u16);
    wr8(b, obj + 0x33, pick(alea, &[0, 0, 1, 0xff]));
    let shot = ddsom_game::objects::pools::ESHL.slot(alea.next() % 4).0;
    wr16(b, obj + 0xa8, shot as u16);
    wr8(b, shot, (alea.next() % 4 != 0) as u8);
    wr8(b, shot + 0x5a, (alea.next() % 2) as u8);
}

/// Variables globales que la boucle de jeu modifie entre deux appels.
fn perturber(b: &mut Bus, alea: &mut Alea, obj: u32, meneur: u32) {
    for (off, valeurs) in [(0x4eu32, &[0u8, 0, 0, 0, 1][..]), (0x3d, &[0, 0, 0, 1]), (0x35, &[0, 1, 2, 3]), (0x36, &[0, 1])] {
        if alea.next() % 3 == 0 {
            wr8(b, A5 + off, valeurs[alea.next() as usize % valeurs.len()]);
        }
    }
    wr8(b, 0xff0ec6, [0u8, 0, 0, 0, 1][alea.next() as usize % 5]);
    if alea.next() % 3 == 0 {
        wr16(b, A5 + 0x506, [0u16, 0x100, 0x2e0, 0xe00, 0xf00, 0x1000, 0x1e00, 0x2d00, 0x3c00][alea.next() as usize % 9]);
    }
    if alea.next() % 4 == 0 {
        wr16(b, A5 + 0x51e, [0u16, 4, 8][alea.next() as usize % 3]);
    }
    // le meneur de la caméra : position autour du boss, vitesse
    if alea.next() % 3 == 0 {
        let x = rd16(b, obj + 8).wrapping_add((alea.next() % 0x300) as u16).wrapping_sub(0x100);
        wr16(b, meneur + 8, x);
        wr16(b, meneur + 0x10, (alea.next() % 0x100) as u16 + 0x60);
        wr16(b, meneur + 0x58, [0u16, 1, 2, 0xffff, 0xfffe][alea.next() as usize % 5]);
    }
    for j in 0..4u32 {
        if alea.next() % 4 == 0 {
            wr8(b, 0xff85de + 0x200 * j, [0u8, 1, 1][alea.next() as usize % 3]);
        }
    }
    // octet 0x22 du message de cinématique : terminé ou non
    let msg = rd16(b, obj + 0xb0) as u32 | 0xff0000;
    if rd16(b, obj + 0xb0) != 0 && alea.next() % 3 == 0 {
        wr8(b, msg + 0x22, (alea.next() % 2) as u8);
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, chauffe, tours] = &a[..] else {
        eprintln!("usage : fuzz_boss <roms> <eeprom> <scénario> <images de chauffe> <tours>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    ddsom_game::install(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    for f in 0..chauffe.parse::<usize>().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    ddsom_game::set_check(true);
    std::panic::set_hook(Box::new(|_| {}));
    let graine: u64 = std::env::var("DDSOM_GRAINE").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut alea = Alea(0x1234_5678_9abc_def1 ^ graine.wrapping_mul(0x9e37_79b9_7f4a_7c15));
    let filtre = std::env::var("DDSOM_ENTREE").ok().and_then(|v| u32::from_str_radix(&v, 16).ok());
    let duree: u32 = std::env::var("DDSOM_PAS").ok().and_then(|v| v.parse().ok()).unwrap_or(60);
    let etats = std::env::var("DDSOM_ETATS").is_ok();
    let couverture = std::env::var("DDSOM_COUVERTURE").is_ok();
    let mut vus: std::collections::BTreeSet<u32> = Default::default();
    let mut entrees: Vec<(u32, u8)> = Vec::new();
    {
        use m68k::decode::Fetch;
        for k in 0..23u32 {
            let e = (m.bus.fetch16(0x39b12 + 4 * k) as u32) << 16 | m.bus.fetch16(0x39b12 + 4 * k + 2) as u32;
            if ddsom_game::all_natives().any(|n| n.entry == e) && filtre.map_or(true, |f| f == e) {
                entrees.push((e, k as u8));
            }
        }
    }
    let (c0, b0) = (m.cpu.clone(), m.bus.clone());
    let (mut essais, mut affiches) = (0, 0);
    for _ in 0..tours.parse::<u32>().unwrap() {
        for &(entree, kind) in &entrees {
            let (mut c, mut b) = (c0.clone(), b0.clone());
            b.irq_horizon = u64::MAX;
            let obj = BOSS.slot(alea.next() % BOSS.slots);
            for off in (0..BOSS.stride).step_by(2) {
                wr16(&mut b, obj.0 + off, 0);
            }
            // le meneur de la caméra : un joueur, ou un ennemi quelconque
            let meneur = 0xff85de + 0x200 * (alea.next() % 4);
            wr16(&mut b, A5 + 0x46, if alea.next() % 8 == 0 { 0 } else { meneur as u16 });
            wr8(&mut b, obj.0, 1);
            wr8(&mut b, obj.0 + 2, kind);
            wr8(&mut b, obj.0 + 3, 0);
            wr8(&mut b, obj.0 + 0x28, BOSS.class);
            wr16(&mut b, obj.0 + 0x62, 40);
            for off in [0x8u32, 0xc, 0x10] {
                let v = (rd16(&mut b, meneur + off) as u32) << 16 | (alea.next() & 0xffff);
                wr32(&mut b, obj.0 + off, v);
            }
            if etats && alea.next() % 2 == 0 {
                let boites = etats_boss(kind);
                let (s4, s5, s6) = boites[alea.next() as usize % boites.len()];
                wr8(&mut b, obj.0 + 4, s4[alea.next() as usize % s4.len()]);
                wr8(&mut b, obj.0 + 5, s5[alea.next() as usize % s5.len()]);
                wr8(&mut b, obj.0 + 6, s6[alea.next() as usize % s6.len()]);
            }
            prep(kind, &mut b, &mut alea, obj.0);
            for pas in 0..duree {
                if rd8(&mut b, obj.0) == 0 {
                    break;
                }
                if pas > 0 {
                    perturber(&mut b, &mut alea, obj.0, meneur);
                    if etats && alea.next() % 3 == 0 {
                        let boites = etats_boss(kind);
                        let (s4, s5, s6) = boites[alea.next() as usize % boites.len()];
                        wr8(&mut b, obj.0 + 4, s4[alea.next() as usize % s4.len()]);
                        wr8(&mut b, obj.0 + 5, s5[alea.next() as usize % s5.len()]);
                        wr8(&mut b, obj.0 + 6, s6[alea.next() as usize % s6.len()]);
                        prep(kind, &mut b, &mut alea, obj.0);
                    }
                }
                // compteurs de places libres cohérents avec le contenu des pools
                for q in POOLS {
                    let libres = (0..q.slots).filter(|&i| rd8(&mut b, q.slot(i).0) == 0).count();
                    wr16(&mut b, A5 + q.free, libres as u16);
                }
                wr8(&mut b, obj.0 + 0x40, (alea.next() % 40 == 0) as u8);
                // le drapeau X n'a pas de sens pour un gestionnaire ; les routines sonores natives le conservent alors que
                // l'original le recalcule : on l'abaisse pour que cet écart connu ne masque pas les autres
                c.sr &= !0x10;
                c.a[0] = obj.0;
                c.a[5] = A5;
                c.d[7] = pas;
                c.d[0] = 4 * kind as u32;
                c.a[1] = entree;
                essais += 1;
                let avant_noms: std::collections::BTreeMap<String, u64> =
                    ddsom_game::report().iter().map(|(n, s)| (n.to_string(), s.mismatches)).collect();
                let avant = avant_noms.values().sum::<u64>();
                let (cs, bs) = (c.clone(), b.clone());
                if couverture {
                    let (mut c2, mut b2) = (cs.clone(), bs.clone());
                    c2.pc = entree;
                    c2.a[7] = cs.a[7].wrapping_sub(4);
                    ddsom_game::mem::wr32(&mut b2, c2.a[7], 0xfffe);
                    for _ in 0..20_000 {
                        if c2.pc == 0xfffe {
                            break;
                        }
                        vus.insert(c2.pc);
                        c2.step(&mut b2);
                    }
                }
                let avant_obj: Vec<u8> = (0..BOSS.stride).map(|i| rd8(&mut b, obj.0 + i)).collect();
                if std::env::var("DDSOM_PAS_ETAT").is_ok() {
                    println!("pas {pas} états {:02x?}", &avant_obj[4..8]);
                }
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ddsom_game::call(&mut c, &mut b, entree)));
                if r.is_err() {
                    // l'original est sorti des rails (entrée incohérente) : trajectoire abandonnée
                    break;
                }
                let apres = ddsom_game::report().values().map(|s| s.mismatches).sum::<u64>();
                // `DDSOM_SEULS=préfixe` : n'afficher que les écarts des routines dont le nom commence ainsi
                let seuls = std::env::var("DDSOM_SEULS").ok();
                let concerne = seuls.as_deref().map_or(true, |p| {
                    ddsom_game::report().iter().any(|(n, s)| {
                        n.starts_with(p) && s.mismatches > avant_noms.get(&n.to_string()).copied().unwrap_or(0)
                    })
                });
                if apres > avant && concerne && affiches < 12 {
                    affiches += 1;
                    // routines dont le compte d'écarts a augmenté pendant cet appel
                    let fautives: Vec<String> = ddsom_game::report()
                        .iter()
                        .filter(|(n, s)| s.mismatches > avant_noms.get(&n.to_string()).copied().unwrap_or(0))
                        .map(|(n, s)| format!("{n} : {}", s.last.clone().unwrap_or_default()))
                        .collect();
                    println!("écart en {entree:06x} type {kind} pas {pas} objet {:06x} : {}", obj.0, fautives.join(" ; "));
                    println!("  objet avant : {:02x?}", avant_obj);
                    if std::env::var("DDSOM_TRACE_ORIG").is_ok() {
                        let (mut c2, mut b2) = (cs.clone(), bs.clone());
                        c2.pc = entree;
                        c2.a[7] = cs.a[7].wrapping_sub(4);
                        ddsom_game::mem::wr32(&mut b2, c2.a[7], 0xfffe);
                        for _ in 0..200_000 {
                            if c2.pc == 0xfffe {
                                break;
                            }
                            let ins = m68k::decode(&mut b2, c2.pc);
                            if std::env::var("DDSOM_SANS_TRACE").is_err() {
                                println!("    {:06x} {:?}", c2.pc, ins.op);
                            }
                            c2.step(&mut b2);
                        }
                        // octets de la RAM qui diffèrent entre la native (b) et l'original (b2)
                        let mut n = 0;
                        for a in 0xff8000u32..0x1000000 {
                            let (u, v) = (rd8(&mut b, a), rd8(&mut b2, a));
                            if u != v && n < 24 {
                                println!("    DIFF {a:06x} (objet+{:x}) natif {u:02x} original {v:02x}", a.wrapping_sub(obj.0));
                                n += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    let mut total = 0;
    for (name, st) in ddsom_game::report() {
        total += st.mismatches;
        println!("  {name:28} {:>7} appels {:>5} écarts{}", st.calls, st.mismatches, st.first.map(|f| format!(" — {f}")).unwrap_or_default());
    }
    println!("{essais} appels de gestionnaires");
    if let Ok(fichier) = std::env::var("DDSOM_COUVERTURE") {
        let texte: Vec<String> = vus.iter().map(|a| format!("{a:06x}")).collect();
        std::fs::write(fichier, texte.join("\n")).expect("écriture couverture");
    }
    println!("{}", if total == 0 { "VALIDE : aucun écart" } else { "ÉCHEC" });
}
