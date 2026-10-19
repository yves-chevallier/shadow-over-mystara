//! Préparation des états du troisième lot ENEMY (types 0 à 15, 18 et 27) pour `fuzz_enemy` : états
//! `(+4, +5, +6, +7)` que les tables de sauts de l'original acceptent (hors de ces tables il saute dans
//! des données) et champs à des valeurs qu'elles tolèrent.
//!
//! Chaque type décrit ses états par des lignes `"s4 s5 s6,s6,.. max7"` : `+4`, `+5`, les `+6` possibles et
//! la plus grande étape `+7` (paire) valide pour ces `+6`.

use super::Alea;
use ddsom_game::mem::{rd16, rd8, wr16, wr32, wr8, A5};

type Bus = cps2::Cps2Bus;

/// Le type fait partie du lot (les états et les champs sont préparés ici).
pub fn gere(kind: u8) -> bool {
    etats(kind).is_some()
}

fn etats(kind: u8) -> Option<&'static [&'static str]> {
    Some(match kind {
        1 => &[
            "0 0 0 0",
            // arrivée, debout, coups, bonds, esquives, chute d'arrivée
            "2 0 0,2,4 2",
            "2 0 6 8",
            "2 2 0,2,4,6 2",
            "2 4 0,2,4,6 10",
            "2 4 8,12 12",
            "2 4 10 6",
            "2 4 14 18",
            "2 4 16 20",
            "2 4 18,20 8",
            "2 6 0,10 2",
            "2 6 2,4,6 12",
            "2 6 8 8",
            "2 8 0,2,6 2",
            "2 8 4 10",
            "2 10 0,2,4,6 0",
            // touché
            "4 0 0 0",
            "4 2 0,2,4 0",
            "4 4 0,2,4 0",
            "4 6 0 0",
            "4 8 0,2,4,6 0",
            "4 10 0 0",
            "6 0 0 0",
        ],
        12 => &[
            "0 0 0 0",
            // arrivée (le plus grand `+6` dépend de la variante), debout, coups, coups d'après-bond, repli, repos
            "2 0 0,2,4,6,8 0",
            "2 2 0,2,4 2",
            "2 4 0,2,4,6 2",
            "2 4 8 4",
            "2 4 10 2",
            "2 4 12 10",
            "2 4 14 8",
            "2 4 16 10",
            "2 4 18 10",
            "2 4 20 10",
            "2 4 22,24 0",
            "2 6 0,2 2",
            "2 6 4 0",
            "2 6 6 6",
            "2 6 8 8",
            "2 6 10 12",
            "2 6 12 8",
            "2 8 0 2",
            "2 8 2 18",
            "2 8 4 12",
            "2 8 6 0",
            "2 10 0,2 0",
            "4 0 0,2 0",
            "4 2 0,2 0",
            "4 4 0,2,4 0",
            "4 6 0 0",
            "4 8 0,2 0",
            "6 0 0 0",
        ],
        2 => &[
            "0 0 0 0",
            // arrivée (le plus grand `+6` dépend de la variante), debout, coups, gardes, repli, repos
            "2 0 0,2,4 0",
            "2 2 0,2,4 2",
            "2 2 6 4",
            "2 4 0,2,4,6,12,14,20,22,24,26 2",
            "2 4 8,10,18 4",
            "2 4 16 8",
            "2 6 0,2,4,6,8,10,14,16,18,20,22,24 2",
            "2 6 12 4",
            "2 8 0 2",
            "2 8 2 0",
            "2 8 4 6",
            "2 8 6 0",
            "2 10 0,2 0",
            "4 0 0,2 0",
            "4 2 0,2 0",
            "4 4 0,2,4 0",
            "4 6 0 0",
            "4 8 0,2,4 0",
            "6 0 0 0",
        ],
        5 => &[
            "0 0 0 0",
            // arrivée (le plus grand `+6` dépend de la variante), debout, coups, déplacements, repos
            "2 0 0,2,4,6,8 0",
            "2 2 0,2,4,6 2",
            "2 4 0,2,8,14 2",
            "2 4 4,10,12 6",
            "2 4 6 0",
            "2 4 16 4",
            "2 6 0,2 2",
            "2 6 4,6,8 6",
            "2 6 10 2",
            "2 8 0,2,4 4",
            "2 8 6 2",
            "2 10 0,2,4 0",
            "4 0 0,2 0",
            "4 2 0,2 0",
            "4 4 0,2,4 0",
            "4 6 0 0",
            "4 8 0,2,4 0",
            "6 0 0 0",
        ],
        _ => return None,
    })
}

fn paire(alea: &mut Alea, max: u8) -> u8 {
    2 * (alea.next() % (max as u32 / 2 + 1)) as u8
}

/// Tire un état valide et pose `+4..+7` ; `fresh` : création de l'objet (variante, série...).
pub fn prep(b: &mut Bus, alea: &mut Alea, kind: u8, obj: u32, fresh: bool) {
    let lignes = etats(kind).unwrap();
    let ligne: Vec<&str> = lignes[alea.next() as usize % lignes.len()].split(' ').collect();
    let s6: Vec<u8> = ligne[2].split(',').map(|v| v.parse().unwrap()).collect();
    let max: u8 = ligne[3].parse().unwrap();
    wr8(b, obj + 4, ligne[0].parse().unwrap());
    wr8(b, obj + 5, ligne[1].parse().unwrap());
    wr8(b, obj + 6, s6[alea.next() as usize % s6.len()]);
    // l'étape 0 est la plus fréquente (début du geste)
    let etape = if alea.next() % 3 == 0 { 0 } else { paire(alea, max) };
    wr8(b, obj + 7, etape);
    communs(b, alea, obj);
    // deuxième caméra (mode à deux écrans) et difficulté, lues par plusieurs types
    if alea.next() % 4 == 0 {
        wr8(b, A5 + 0x36, 1);
        wr16(b, A5 + 0x556, (alea.next() % 0x200) as u16);
        wr16(b, A5 + 0x55a, (alea.next() % 0x200) as u16);
    } else {
        wr8(b, A5 + 0x36, 0);
    }
    match kind {
        1 => squelette(b, alea, obj, fresh),
        12 => elfe(b, alea, obj, fresh),
        2 => troglodyte(b, alea, obj, fresh),
        5 => goule(b, alea, obj, fresh),
        _ => {}
    }
}

/// Champs que presque tous les ennemis lisent : cible, code de réaction, orientation, hauteur.
fn communs(b: &mut Bus, alea: &mut Alea, obj: u32) {
    wr16(b, obj + 0x50, 0x85de + 0x200 * (alea.next() % 4) as u16);
    wr8(b, obj + 0x49, 4 * (alea.next() % 43) as u8);
    wr8(b, obj + 0x4a, 0x20 * (alea.next() % 2) as u8);
    wr8(b, obj + 0x2c, 0x20 * (alea.next() % 2) as u8);
    // au ras du sol, parfois en l'air, parfois en train de retomber
    let h = [0u16, 0, 0, 1, 2, 0x30][alea.next() as usize % 6];
    wr16(b, obj + 0xc, h);
    if alea.next() % 2 == 0 {
        wr16(b, obj + 0x18, [0x400u16, 0xff00, 0x100, 0xfc00][alea.next() as usize % 4]);
        wr16(b, obj + 0x1a, 0xff80);
    }
    for off in [0x14u32, 0x16, 0x1c, 0x1e] {
        wr16(b, obj + off, [0u16, 0x200, 0xfe00, 0x300, 0x480][alea.next() as usize % 5]);
    }
    wr8(b, obj + 0x2d, [0u8, 8, 0x10, 0x18][alea.next() as usize % 4]);
    wr8(b, obj + 0x32, alea.next() as u8);
    wr8(b, obj + 0x33, [0u8, 1, 0xff, 0x80][alea.next() as usize % 4]);
    wr16(b, obj + 0x84, (alea.next() % 3) as u16);
    wr8(b, obj + 0x86, (alea.next() % 4 == 0) as u8);
    // le joueur visé est en vie
    let j = 0xff0000 | rd16(b, obj + 0x50) as u32;
    if alea.next() % 4 != 0 {
        wr8(b, j, 1);
    }
}

fn squelette(b: &mut Bus, alea: &mut Alea, obj: u32, fresh: bool) {
    if fresh {
        wr8(b, obj + 3, (alea.next() % 6) as u8);
        wr8(b, obj + 0x2f, (alea.next() % 2) as u8);
        wr16(b, obj + 0x42, (alea.next() % 3) as u16);
    }
    wr8(b, obj + 0x8f, [0u8, 0, 0, 0, 1, 2, 3][alea.next() as usize % 7]);
    for off in [0x6bu32, 0x6e, 0x71, 0x77] {
        wr8(b, obj + off, (alea.next() % 5) as u8);
    }
    wr8(b, obj + 0x68, [3u8, 8, 9, 2][alea.next() as usize % 4]);
    wr8(b, obj + 0x79, [0xbu8, 0xc][alea.next() as usize % 2]);
    wr16(b, obj + 0xa2, (alea.next() % 4) as u16);
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    wr16(b, obj + 0xa4, ox.wrapping_add((alea.next() % 0x80) as u16).wrapping_sub(0x40));
    wr8(b, obj + 0xa6, [0u8, 2, 3][alea.next() as usize % 3]);
    wr8(b, obj + 0xa7, (alea.next() % 6 == 0) as u8);
    wr8(b, obj + 0xa8, [0u8, 0, 1, 0xff][alea.next() as usize % 4]);
    wr8(b, obj + 0xa9, [0u8, 2][alea.next() as usize % 2]);
    wr16(b, obj + 0x7a, ox.wrapping_add((alea.next() % 0x80) as u16));
    wr16(b, obj + 0x7c, oy.wrapping_add((alea.next() % 0x40) as u16));
    wr32(b, obj + 0x4c, [0x1c74b6u32, 0x1c7336][alea.next() as usize % 2]);
    // l'ennemi qui a créé celui-ci (compteur `+0xaf`) : un autre objet existant
    wr8(b, obj + 0xac, (alea.next() % 3 == 0) as u8);
    wr16(b, obj + 0xaa, 0x85de + 0x200 * (alea.next() % 4) as u16);
    wr8(b, obj + 0x29, (alea.next() % 2) as u8);
    let _ = rd8(b, obj);
}

fn elfe(b: &mut Bus, alea: &mut Alea, obj: u32, fresh: bool) {
    if fresh {
        // la variante est un indice de table : 0, 2, 4 ou 6
        wr8(b, obj + 3, 2 * (alea.next() % 4) as u8);
    }
    // l'arrivée n'a que quelques `+6` selon la variante
    if rd8(b, obj + 4) == 2 && rd8(b, obj + 5) == 0 {
        let plus_grand = match rd8(b, obj + 3) {
            0 => 2,
            2 | 6 => 6,
            _ => 8,
        };
        let s6 = rd8(b, obj + 6).min(plus_grand);
        wr8(b, obj + 6, s6);
    }
    // coup prévu après l'approche : un `+6` valide du groupe courant
    let plus_grand = match rd8(b, obj + 5) {
        6 => 12,
        8 => 6,
        _ => 24,
    };
    wr8(b, obj + 0xa5, paire(alea, plus_grand));
    wr8(b, obj + 0xa6, (alea.next() % 3) as u8);
    for off in [0x6bu32, 0x6e, 0x71, 0x77, 0xa2] {
        wr8(b, obj + off, (alea.next() % 5) as u8);
    }
    wr8(b, obj + 0xa1, alea.next() as u8);
    wr8(b, obj + 0xa4, (alea.next() % 4) as u8);
    wr8(b, obj + 0x79, 0x1a + (alea.next() % 3) as u8);
    wr8(b, obj + 0x67, (alea.next() % 2) as u8);
    wr8(b, obj + 0x1, 0);
    wr16(b, obj + 0x84, (alea.next() % 3) as u16);
    wr8(b, A5 + 0x30, (alea.next() % 5 != 0) as u8);
    wr8(b, obj + 0x3b, 0);
}

fn troglodyte(b: &mut Bus, alea: &mut Alea, obj: u32, fresh: bool) {
    if fresh {
        wr8(b, obj + 3, (alea.next() % 6) as u8);
        wr8(b, obj + 0x2f, (alea.next() % 2) as u8);
    }
    // l'arrivée n'a que quelques `+6` selon la variante
    if rd8(b, obj + 4) == 2 && rd8(b, obj + 5) == 0 {
        let plus_grand = match rd8(b, obj + 3) {
            0 | 3 | 4 => 2,
            _ => 4,
        };
        let s6 = rd8(b, obj + 6).min(plus_grand);
        wr8(b, obj + 6, s6);
    }
    // coup à reprendre : un `+6` valide du groupe courant
    let plus_grand = match rd8(b, obj + 5) {
        6 => 24,
        8 => 6,
        _ => 26,
    };
    wr8(b, obj + 0xa3, paire(alea, plus_grand));
    for off in [0x6bu32, 0x6e, 0x71, 0x77, 0xa2] {
        wr8(b, obj + off, (alea.next() % 5) as u8);
    }
    wr8(b, obj + 0x68, [3u8, 4, 9][alea.next() as usize % 3]);
    wr8(b, obj + 0x79, 0x14 + (alea.next() % 2) as u8);
    wr8(b, obj + 0x67, (alea.next() % 2) as u8);
    for off in [0xa0u32, 0xa1] {
        wr8(b, obj + off, [0u8, 1, 0xe, 0xdf, 0xe0, 0xf0, 0x7f, 0xee, alea.next() as u8][alea.next() as usize % 9]);
    }
    // clignotement : phase, alternance, attente, compteurs
    wr8(b, obj + 0xa5, [0xffu8, 0xff, 0, 1, 2, 3][alea.next() as usize % 6]);
    wr8(b, obj + 0xa6, (alea.next() % 2) as u8);
    wr8(b, obj + 0xa7, (alea.next() % 2) as u8);
    wr16(b, obj + 0xa8, [0u16, 1, 5, 0xc, 0xffff][alea.next() as usize % 5]);
    wr16(b, obj + 0xaa, [0u16, 1, 5, 0xffff][alea.next() as usize % 4]);
    wr16(b, obj + 0xae, [0u16, 0xffff, 0x258, 0x8000][alea.next() as usize % 4]);
    wr8(b, obj + 0xb0, (alea.next() % 4 == 0) as u8);
    wr8(b, obj + 0x84, (alea.next() % 3) as u8);
}

fn goule(b: &mut Bus, alea: &mut Alea, obj: u32, fresh: bool) {
    if fresh {
        wr8(b, obj + 3, (alea.next() % 6) as u8);
    }
    // l'arrivée n'a que quelques `+6` selon la variante
    if rd8(b, obj + 4) == 2 && rd8(b, obj + 5) == 0 {
        let plus_grand = match rd8(b, obj + 3) {
            0 | 4 => 2,
            2 | 3 => 4,
            1 => 6,
            _ => 8,
        };
        let s6 = rd8(b, obj + 6).min(plus_grand);
        wr8(b, obj + 6, s6);
    }
    wr8(b, obj + 0x8f, [0u8, 0, 0, 0, 1, 2, 3][alea.next() as usize % 7]);
    for off in [0x6bu32, 0x6e, 0x71, 0xa2] {
        wr8(b, obj + off, (alea.next() % 5) as u8);
    }
    for off in [0xa0u32, 0xa1] {
        wr8(b, obj + off, [0u8, 2, 4, 6, 8, 0xff, alea.next() as u8][alea.next() as usize % 7]);
    }
    wr8(b, obj + 0x79, [0u8, 9][alea.next() as usize % 2]);
    wr8(b, obj + 0xa6, (alea.next() % 2) as u8);
    wr16(b, obj + 0xa4, 0x85de + 0x200 * (alea.next() % 4) as u16);
    let ox = rd16(b, obj + 8);
    wr16(b, obj + 0xa8, ox.wrapping_add((alea.next() % 4) as u16));
    wr16(b, obj + 0x84, (alea.next() % 3) as u16);
    wr8(b, obj + 0x86, (alea.next() % 4 == 0) as u8);
    wr16(b, obj + 0x42, [0x703u16, 0x307, 0x110, alea.next() as u16 & 0x707][alea.next() as usize % 4]);
}
