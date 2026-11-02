//! Préparation des états du troisième lot ENEMY (types 0 à 15, 18 et 27) pour `fuzz_enemy` : états
//! `(+4, +5, +6, +7)` que les tables de sauts de l'original acceptent (hors de ces tables il saute dans
//! des données) et champs à des valeurs qu'elles tolèrent.
//!
//! Chaque type décrit ses états par des lignes `"s4 s5 s6,s6,.. max7"` : `+4`, `+5`, les `+6` possibles et
//! la plus grande étape `+7` (paire) valide pour ces `+6`.

use super::Rng;
use ddsom_game::mem::{rd16, rd8, wr16, wr32, wr8, A5};

type Bus = cps2::Cps2Bus;

/// Le type fait partie du lot (les états et les champs sont préparés ici).
pub fn handles(kind: u8) -> bool {
    states(kind).is_some()
}

fn states(kind: u8) -> Option<&'static [&'static str]> {
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
        9 => &[
            "0 0 0 0",
            // marche, trajectoires par groupe (le plus grand `+7` dépend de `+0x2f`), demi-tour, bond, dormant
            "2 0 0,2 2",
            "2 2 0 2",
            "2 4 0 6",
            "2 4 2,4,6,8 8",
            "2 6 0,2,4 0",
            "2 8 0 0",
            "4 0 0,2,4 0",
            "4 2 0,2 0",
            "4 4 0,2 0",
            "6 0 0 0",
        ],
        8 => &[
            "0 0 0 0",
            // arrivée, déplacements, coups, repos, saisie
            "2 0 0 6",
            "2 0 2 6",
            "2 2 0,2,4,6 2",
            "2 2 8 10",
            "2 2 10 8",
            "2 4 0,2 4",
            "2 4 4 2",
            "2 4 6 6",
            "2 4 8 10",
            "2 4 10 8",
            "2 4 12 8",
            "2 6 0,2 0",
            "2 8 0,2 0",
            "2 10 0,2 0",
            "2 12 0 4",
            "2 14 0 0",
            "4 0 0,2 0",
            "4 2 0,2,4 0",
            "4 4 0,2,4 0",
            "4 6 0,2,4,6,8,10,12,14 0",
            "4 8 0,2,4,6 0",
            "4 10 0 0",
            "6 0 0 0",
        ],
        0 => &[
            "0 0 0 0",
            // arrivée (sous-états 0 à 20), debout, coups, gestes d'affût et de repli, promenade, touché
            "2 0 0 4",
            "2 0 2 2",
            "2 0 4,6,8,18,20 6",
            "2 0 10,12,16 4",
            "2 0 14 2",
            "2 2 0,8 2",
            "2 2 2,4 2",
            "2 2 6 4",
            "2 4 0,2,6 6",
            "2 4 4,8 8",
            "2 4 10,20 10",
            "2 4 12 12",
            "2 4 14,34 6",
            "2 4 16,36,18 10",
            "2 4 22,24,26,28,32 6",
            "2 4 30 14",
            "2 6 0 8",
            "2 6 2 6",
            "2 6 4,6,10 10",
            "2 6 8 12",
            "2 8 0,6 4",
            "2 8 2,4 8",
            "2 10 0,2 0",
            "4 0 0,2 0",
            "4 2 0,2 0",
            "4 4 0,2,4 0",
            "4 6 0 0",
            "4 8 0,2,4,6,8,10,12,14 0",
            "6 0 0 0",
        ],
        6 => &[
            "0 0 0 0",
            // arrivée, debout, coups, gestes d'affût et de repli, promenade, morsure, touché
            "2 0 0,2,8,10 4",
            "2 0 4,6 6",
            "2 2 0,8 2",
            "2 2 2,4 2",
            "2 2 6 4",
            "2 2 10 6",
            "2 4 0 14",
            "2 4 2,4 6",
            "2 4 6 8",
            "2 4 8 12",
            "2 4 10,12,14 6",
            "2 4 16 8",
            "2 6 0,2 6",
            "2 6 4,6 10",
            "2 6 8,10 12",
            "2 6 12 14",
            "2 8 0,6 4",
            "2 8 2 6",
            "2 8 4 8",
            "2 10 0,2,4 0",
            "2 12 0,2 2",
            "4 0 0 0",
            "4 2 0,2 0",
            "4 4 0,2,4,6,8,10 0",
            "4 6 0 0",
            "4 8 0,2,4 0",
            "6 0 0 0",
        ],
        18 => &[
            "0 0 0 0",
            // arrivée, déplacements, coups (bras lancés), repos, touché
            "2 0 0 14",
            "2 0 2,4,6 4",
            "2 2 0,2,4,6 2",
            "2 2 8 4",
            "2 2 10 8",
            "2 4 0 2",
            "2 4 2,4 8",
            "2 4 6 8",
            "2 4 8 12",
            "2 6 0,2 0",
            "2 8 0,2 0",
            "2 10 0,2 0",
            "4 0 0 0",
            "4 2 0,2,4 0",
            "4 4 0,2,4 0",
            "4 6 0,2,4,6,8 0",
            "4 8 0,2 0",
            "4 10 0 0",
            "6 0 0 0",
        ],
        10 => &[
            "0 0 0 0",
            // arrivée (le plus grand `+7` de `+6` = 4), debout, coups, attente, coup spécial, coups, touché
            "2 0 2,6,8 0",
            "2 0 4 4",
            "2 2 0,2,4 4",
            "2 4 0 0",
            "2 4 2,4 2",
            "2 4 6 6",
            "2 6 0,2 0",
            "2 8 0 6",
            "2 10 0 0",
            "2 10 2,4,6 2",
            "2 10 8 2",
            "4 0 0 0",
            "4 2 0,2 0",
            "4 4 0,2,4 0",
            "6 0 0 0",
        ],
        27 => &[
            "0 0 0 0",
            // arrivée, roule vers le joueur, gestes de l'arme (4), tourne sur place (6, 8, 10), touché
            "2 0 0 6",
            "2 0 2,4 4",
            "2 2 0,2,4,6 2",
            "2 2 6 2",
            "2 2 6 2",
            "2 2 8 6",
            "2 4 0,2,4 2",
            "2 4 6 6",
            "2 4 8 8",
            "2 4 10 6",
            "2 4 12 4",
            "2 6 0,2 0",
            "2 8 0,2 0",
            "2 10 0,2 0",
            "4 0 0 0",
            "4 0 2 0",
            "4 2 0,2,4 0",
            "4 4 0,2,4 0",
            "4 6 0,2,4,6,8 0",
            "4 8 0,2 0",
            "4 10 0,2 0",
            "6 0 0 0",
        ],
        11 => &[
            "0 0 0 0",
            // debout piloté par les touches, coups (4), gestes de repli (6, 8), touché
            "2 0 0 0",
            "2 0 2 2",
            "2 2 0 0",
            "2 2 2 2",
            "2 4 0 2",
            "2 4 2,4,6 10",
            "2 4 8,12 12",
            "2 4 10 6",
            "2 4 14 18",
            "2 4 16 20",
            "2 4 18,20 8",
            "2 6 0 2",
            "2 6 2,4,6 12",
            "2 6 8 8",
            "2 6 10 2",
            "2 8 0,6 2",
            "2 8 2 2",
            "2 8 4 10",
            "4 0 0 0",
            "4 2 0,2 0",
            "4 4 0,2,4 0",
            "4 6 0 0",
            "4 8 0,2,4,6 0",
            "4 10 0 0",
            "6 0 0 0",
        ],
        3 => &[
            "0 0 0 0",
            // arrivée (toutes variantes), debout, tirs (4 : +6 = 0 premier geste, 2 second geste), pas et sauts (6),
            // repos (8), attente (10), demi-tour (12), touché
            "2 0 0,2,4,6 0",
            "2 2 0,2,4 0",
            "2 4 0 10",
            "2 4 2 14",
            "2 4 4 0",
            "2 6 0 2",
            "2 6 2,8 6",
            "2 6 4,6 6",
            "2 8 0 0",
            "2 8 2,4 2",
            "2 8 6 0",
            "2 10 0,2 0",
            "2 12 0,2 0",
            "4 0 0 0",
            "4 2 0 0",
            "4 4 0,2,4 0",
            "4 6 0 0",
            "4 8 0,2,4 0",
            "6 0 0 0",
        ],
        13 => &[
            "0 0 0 0",
            // arrivée (+6 = 6 : variantes 0 à 8, +6 = 8 : variantes 0 et 4, voir `gnoll`), debout, gestes, repos, touché
            "2 0 0,2,4 0",
            "2 0 6 0",
            "2 0 8 0",
            "2 2 0,2,4 2",
            "2 4 0,2,4,6 2",
            "2 4 8 4",
            "2 4 10 2",
            "2 4 12 10",
            "2 4 14 8",
            "2 4 16 10",
            "2 4 18 14",
            "2 4 20 12",
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
            "4 0 0 0",
            "4 2 0,2 0",
            "4 4 0,2,4 2",
            "4 6 0 0",
            "4 8 0,2 0",
            "6 0 0 0",
        ],
        14 => &[
            "0 0 0 0",
            // arrivée (+6 = 6 : variantes 2 et 4 seulement), debout, gestes (4 : lancer et sauts, 6 : pas, sauts, 8 : suite)
            "2 0 0,2,4 0",
            "2 0 6 0",
            "2 2 0,2,4 2",
            "2 2 6 4",
            "2 4 0,2,12,14,16 2",
            "2 4 4 12",
            "2 4 6,8 10",
            "2 4 10 0",
            "2 4 18 2",
            "2 4 20 4",
            "2 6 0,2 2",
            "2 6 4 0",
            "2 6 6 6",
            "2 6 8 10",
            "2 6 10 12",
            "2 6 12 8",
            "2 8 0 2",
            "2 8 2 10",
            "2 8 4 12",
            "2 8 6 0",
            "2 10 0,2 0",
            "4 0 0 0",
            "4 2 0,2 0",
            "4 4 0,2,4 0",
            "4 6 0 0",
            "4 8 0,2 0",
            "6 0 0 0",
        ],
        15 => &[
            "0 0 0 0",
            // arrivée (+6 = 6 : variantes 2 et 4 seulement), debout, gestes (4 : séries, 6 : pas de côté, 8 : suite), repos, touché
            "2 0 0,2,4 0",
            "2 0 6 0",
            "2 2 0,2,4 2",
            "2 2 6 4",
            "2 4 0,2,12,14,16 2",
            "2 4 4 2",
            "2 4 6,8 10",
            "2 4 10 0",
            "2 4 18 2",
            "2 4 20 4",
            "2 6 0,2 2",
            "2 6 4 0",
            "2 6 6 6",
            "2 6 8 6",
            "2 6 10,12 8",
            "2 8 0 2",
            "2 8 2 6",
            "2 8 4 12",
            "2 8 6 0",
            "2 10 0,2 0",
            "4 0 0 0",
            "4 2 0,2 0",
            "4 4 0,2,4 0",
            "4 6 0 0",
            "4 8 0,2 0",
            "6 0 0 0",
        ],
        _ => return None,
    })
}

fn pair(rng: &mut Rng, max: u8) -> u8 {
    2 * (rng.next() % (max as u32 / 2 + 1)) as u8
}

/// Tire un état valide et pose `+4..+7` ; `fresh` : création de l'objet (variante, série...).
pub fn prep(b: &mut Bus, rng: &mut Rng, kind: u8, obj: u32, fresh: bool) {
    let lines = states(kind).unwrap();
    let line: Vec<&str> = lines[rng.next() as usize % lines.len()].split(' ').collect();
    let s6: Vec<u8> = line[2].split(',').map(|v| v.parse().unwrap()).collect();
    let max: u8 = line[3].parse().unwrap();
    wr8(b, obj + 4, line[0].parse().unwrap());
    wr8(b, obj + 5, line[1].parse().unwrap());
    wr8(b, obj + 6, s6[rng.next() as usize % s6.len()]);
    // l'étape 0 est la plus fréquente (début du geste)
    let step = if rng.next() % 3 == 0 { 0 } else { pair(rng, max) };
    wr8(b, obj + 7, step);
    common(b, rng, obj);
    // deuxième caméra (mode à deux écrans) et difficulté, lues par plusieurs types
    if rng.next() % 4 == 0 {
        wr8(b, A5 + 0x36, 1);
        wr16(b, A5 + 0x556, (rng.next() % 0x200) as u16);
        wr16(b, A5 + 0x55a, (rng.next() % 0x200) as u16);
    } else {
        wr8(b, A5 + 0x36, 0);
    }
    match kind {
        1 => skeleton(b, rng, obj, fresh),
        12 => elf(b, rng, obj, fresh),
        2 => troglodyte(b, rng, obj, fresh),
        5 => ghoul(b, rng, obj, fresh),
        9 => beetle(b, rng, obj, fresh),
        10 => scorpion(b, rng, obj, fresh),
        8 => gargoyle(b, rng, obj, fresh),
        6 => dog(b, rng, obj, fresh),
        0 => kobold(b, rng, obj, fresh),
        18 => ogre(b, rng, obj, fresh),
        15 => goblin(b, rng, obj, fresh),
        14 => troll(b, rng, obj, fresh),
        13 => gnoll(b, rng, obj, fresh),
        3 => archer(b, rng, obj, fresh),
        11 => golem(b, rng, obj, fresh),
        27 => machine(b, rng, obj, fresh),
        _ => {}
    }
}

/// Champs que presque tous les ennemis lisent : cible, code de réaction, orientation, hauteur.
fn common(b: &mut Bus, rng: &mut Rng, obj: u32) {
    wr16(b, obj + 0x50, 0x85de + 0x200 * (rng.next() % 4) as u16);
    wr8(b, obj + 0x49, 4 * (rng.next() % 43) as u8);
    wr8(b, obj + 0x4a, 0x20 * (rng.next() % 2) as u8);
    wr8(b, obj + 0x2c, 0x20 * (rng.next() % 2) as u8);
    // au ras du sol, parfois en l'air, parfois en train de retomber
    let h = [0u16, 0, 0, 1, 2, 0x30][rng.next() as usize % 6];
    wr16(b, obj + 0xc, h);
    if rng.next() % 2 == 0 {
        wr16(b, obj + 0x18, [0x400u16, 0xff00, 0x100, 0xfc00][rng.next() as usize % 4]);
        wr16(b, obj + 0x1a, 0xff80);
    }
    for off in [0x14u32, 0x16, 0x1c, 0x1e] {
        wr16(b, obj + off, [0u16, 0x200, 0xfe00, 0x300, 0x480][rng.next() as usize % 5]);
    }
    wr8(b, obj + 0x2d, [0u8, 8, 0x10, 0x18][rng.next() as usize % 4]);
    wr8(b, obj + 0x32, rng.next() as u8);
    wr8(b, obj + 0x33, [0u8, 1, 0xff, 0x80][rng.next() as usize % 4]);
    wr16(b, obj + 0x84, (rng.next() % 3) as u16);
    wr8(b, obj + 0x86, (rng.next() % 4 == 0) as u8);
    // le joueur visé est en vie
    let j = 0xff0000 | rd16(b, obj + 0x50) as u32;
    if rng.next() % 4 != 0 {
        wr8(b, j, 1);
    }
}

fn skeleton(b: &mut Bus, rng: &mut Rng, obj: u32, fresh: bool) {
    if fresh {
        wr8(b, obj + 3, (rng.next() % 6) as u8);
        wr8(b, obj + 0x2f, (rng.next() % 2) as u8);
        wr16(b, obj + 0x42, (rng.next() % 3) as u16);
    }
    wr8(b, obj + 0x8f, [0u8, 0, 0, 0, 1, 2, 3][rng.next() as usize % 7]);
    for off in [0x6bu32, 0x6e, 0x71, 0x77] {
        wr8(b, obj + off, (rng.next() % 5) as u8);
    }
    wr8(b, obj + 0x68, [3u8, 8, 9, 2][rng.next() as usize % 4]);
    wr8(b, obj + 0x79, [0xbu8, 0xc][rng.next() as usize % 2]);
    wr16(b, obj + 0xa2, (rng.next() % 4) as u16);
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    wr16(b, obj + 0xa4, ox.wrapping_add((rng.next() % 0x80) as u16).wrapping_sub(0x40));
    wr8(b, obj + 0xa6, [0u8, 2, 3][rng.next() as usize % 3]);
    wr8(b, obj + 0xa7, (rng.next() % 6 == 0) as u8);
    wr8(b, obj + 0xa8, [0u8, 0, 1, 0xff][rng.next() as usize % 4]);
    wr8(b, obj + 0xa9, [0u8, 2][rng.next() as usize % 2]);
    wr16(b, obj + 0x7a, ox.wrapping_add((rng.next() % 0x80) as u16));
    wr16(b, obj + 0x7c, oy.wrapping_add((rng.next() % 0x40) as u16));
    wr32(b, obj + 0x4c, [0x1c74b6u32, 0x1c7336][rng.next() as usize % 2]);
    // l'ennemi qui a créé celui-ci (compteur `+0xaf`) : un autre objet existant
    wr8(b, obj + 0xac, (rng.next() % 3 == 0) as u8);
    wr16(b, obj + 0xaa, 0x85de + 0x200 * (rng.next() % 4) as u16);
    wr8(b, obj + 0x29, (rng.next() % 2) as u8);
    let _ = rd8(b, obj);
}

fn elf(b: &mut Bus, rng: &mut Rng, obj: u32, fresh: bool) {
    if fresh {
        // la variante est un indice de table : 0, 2, 4 ou 6
        wr8(b, obj + 3, 2 * (rng.next() % 4) as u8);
    }
    // l'arrivée n'a que quelques `+6` selon la variante
    if rd8(b, obj + 4) == 2 && rd8(b, obj + 5) == 0 {
        let largest = match rd8(b, obj + 3) {
            0 => 2,
            2 | 6 => 6,
            _ => 8,
        };
        let s6 = rd8(b, obj + 6).min(largest);
        wr8(b, obj + 6, s6);
    }
    // coup prévu après l'approche : un `+6` valide du groupe courant
    let largest = match rd8(b, obj + 5) {
        6 => 12,
        8 => 6,
        _ => 24,
    };
    wr8(b, obj + 0xa5, pair(rng, largest));
    wr8(b, obj + 0xa6, (rng.next() % 3) as u8);
    for off in [0x6bu32, 0x6e, 0x71, 0x77, 0xa2] {
        wr8(b, obj + off, (rng.next() % 5) as u8);
    }
    wr8(b, obj + 0xa1, rng.next() as u8);
    wr8(b, obj + 0xa4, (rng.next() % 4) as u8);
    wr8(b, obj + 0x79, 0x1a + (rng.next() % 3) as u8);
    wr8(b, obj + 0x67, (rng.next() % 2) as u8);
    wr8(b, obj + 0x1, 0);
    wr16(b, obj + 0x84, (rng.next() % 3) as u16);
    wr8(b, A5 + 0x30, (rng.next() % 5 != 0) as u8);
    wr8(b, obj + 0x3b, 0);
}

fn troglodyte(b: &mut Bus, rng: &mut Rng, obj: u32, fresh: bool) {
    if fresh {
        wr8(b, obj + 3, (rng.next() % 6) as u8);
        wr8(b, obj + 0x2f, (rng.next() % 2) as u8);
    }
    // l'arrivée n'a que quelques `+6` selon la variante
    if rd8(b, obj + 4) == 2 && rd8(b, obj + 5) == 0 {
        let largest = match rd8(b, obj + 3) {
            0 | 3 | 4 => 2,
            _ => 4,
        };
        let s6 = rd8(b, obj + 6).min(largest);
        wr8(b, obj + 6, s6);
    }
    // coup à reprendre : un `+6` valide du groupe courant
    let largest = match rd8(b, obj + 5) {
        6 => 24,
        8 => 6,
        _ => 26,
    };
    wr8(b, obj + 0xa3, pair(rng, largest));
    for off in [0x6bu32, 0x6e, 0x71, 0x77, 0xa2] {
        wr8(b, obj + off, (rng.next() % 5) as u8);
    }
    wr8(b, obj + 0x68, [3u8, 4, 9][rng.next() as usize % 3]);
    wr8(b, obj + 0x79, 0x14 + (rng.next() % 2) as u8);
    wr8(b, obj + 0x67, (rng.next() % 2) as u8);
    for off in [0xa0u32, 0xa1] {
        wr8(b, obj + off, [0u8, 1, 0xe, 0xdf, 0xe0, 0xf0, 0x7f, 0xee, rng.next() as u8][rng.next() as usize % 9]);
    }
    // clignotement : phase, alternance, attente, compteurs
    wr8(b, obj + 0xa5, [0xffu8, 0xff, 0, 1, 2, 3][rng.next() as usize % 6]);
    wr8(b, obj + 0xa6, (rng.next() % 2) as u8);
    wr8(b, obj + 0xa7, (rng.next() % 2) as u8);
    wr16(b, obj + 0xa8, [0u16, 1, 5, 0xc, 0xffff][rng.next() as usize % 5]);
    wr16(b, obj + 0xaa, [0u16, 1, 5, 0xffff][rng.next() as usize % 4]);
    wr16(b, obj + 0xae, [0u16, 0xffff, 0x258, 0x8000][rng.next() as usize % 4]);
    wr8(b, obj + 0xb0, (rng.next() % 4 == 0) as u8);
    wr8(b, obj + 0x84, (rng.next() % 3) as u8);
}

fn ghoul(b: &mut Bus, rng: &mut Rng, obj: u32, fresh: bool) {
    if fresh {
        wr8(b, obj + 3, (rng.next() % 6) as u8);
    }
    // l'arrivée n'a que quelques `+6` selon la variante
    if rd8(b, obj + 4) == 2 && rd8(b, obj + 5) == 0 {
        let largest = match rd8(b, obj + 3) {
            0 | 4 => 2,
            2 | 3 => 4,
            1 => 6,
            _ => 8,
        };
        let s6 = rd8(b, obj + 6).min(largest);
        wr8(b, obj + 6, s6);
    }
    wr8(b, obj + 0x8f, [0u8, 0, 0, 0, 1, 2, 3][rng.next() as usize % 7]);
    for off in [0x6bu32, 0x6e, 0x71, 0xa2] {
        wr8(b, obj + off, (rng.next() % 5) as u8);
    }
    for off in [0xa0u32, 0xa1] {
        wr8(b, obj + off, [0u8, 2, 4, 6, 8, 0xff, rng.next() as u8][rng.next() as usize % 7]);
    }
    wr8(b, obj + 0x79, [0u8, 9][rng.next() as usize % 2]);
    wr8(b, obj + 0xa6, (rng.next() % 2) as u8);
    wr16(b, obj + 0xa4, 0x85de + 0x200 * (rng.next() % 4) as u16);
    let ox = rd16(b, obj + 8);
    wr16(b, obj + 0xa8, ox.wrapping_add((rng.next() % 4) as u16));
    wr16(b, obj + 0x84, (rng.next() % 3) as u16);
    wr8(b, obj + 0x86, (rng.next() % 4 == 0) as u8);
    wr16(b, obj + 0x42, [0x703u16, 0x307, 0x110, rng.next() as u16 & 0x707][rng.next() as usize % 4]);
}

fn beetle(b: &mut Bus, rng: &mut Rng, obj: u32, fresh: bool) {
    if fresh {
        wr8(b, obj + 3, (rng.next() % 5) as u8);
        wr8(b, obj + 0x2f, (rng.next() % 8) as u8);
        wr8(b, obj + 0x42, (rng.next() % 4) as u8);
    }
    // `+7` est une étape de la table du groupe `+0x2f` : la dérive (groupes 2 et 3) n'en a que deux
    if rd8(b, obj + 4) == 2 && rd8(b, obj + 5) == 4 && rd8(b, obj + 6) == 0 {
        let largest = if matches!(rd8(b, obj + 0x2f), 2 | 3) { 2 } else { 6 };
        let e = rd8(b, obj + 7).min(largest);
        wr8(b, obj + 7, e);
    }
    wr8(b, obj + 0xa4, 0x10 * (rng.next() % 2) as u8);
    // étape à reprendre après une marche latérale (2 ou 6 dans le jeu)
    wr8(b, obj + 0xa6, [2u8, 6][rng.next() as usize % 2]);
    wr8(b, obj + 0xa7, 2 * (rng.next() % 3) as u8);
    wr8(b, obj + 0xa2, 1 + (rng.next() % 4) as u8);
    wr8(b, obj + 0xa3, 1 + (rng.next() % 3) as u8);
    wr8(b, obj + 0xb8, (rng.next() % 2) as u8);
    wr16(b, obj + 0xb4, [0u16, 0x300, 0xfd00][rng.next() as usize % 3]);
    wr16(b, obj + 0xb6, [0u16, 0x80, 0x40, 0xfff0, 0x140][rng.next() as usize % 5]);
    wr8(b, obj + 0x6e, 1 + (rng.next() % 4) as u8);
    wr8(b, obj + 0x56, (rng.next() % 4) as u8);
    wr8(b, obj + 0x53, (rng.next() % 4 == 0) as u8);
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    wr16(b, obj + 0xa8, ox.wrapping_add((rng.next() % 0x80) as u16));
    wr16(b, obj + 0xac, oy.wrapping_add((rng.next() % 0x40) as u16));
    wr16(b, obj + 0xb0, ox);
    wr16(b, obj + 0xb2, oy);
    wr32(b, obj + 0x4c, [0x1c9036u32, 0x1c7336][rng.next() as usize % 2]);
    wr16(b, obj + 0x32, [0u16, 0x8000, 0x0080, 0xff00][rng.next() as usize % 4]);
    wr8(b, obj + 0x29, (rng.next() % 2) as u8);
    wr16(b, obj + 0x5c, 0x85de + 0x200 * (rng.next() % 4) as u16);
    // saisie par un joueur (0x2810) : rare
    wr8(b, A5 + 0x42, (rng.next() % 4 == 0) as u8);
    if rng.next() % 2 == 0 {
        wr16(b, obj + 0x46, 0);
    }
    // la morsure (0x661ce) ne se déclenche que si la cible est dans l'état 0x200, étape 0xc
    if rng.next() % 3 == 0 {
        let p = 0xff0000 | rd16(b, obj + 0x50) as u32;
        wr16(b, p + 4, 0x200);
        wr8(b, p + 6, 0xc);
    }
}

fn scorpion(b: &mut Bus, rng: &mut Rng, obj: u32, fresh: bool) {
    if fresh {
        wr8(b, obj + 3, (rng.next() % 6) as u8);
        wr8(b, obj + 0x2f, (rng.next() % 5) as u8);
    }
    wr8(b, obj + 0xa2, 1 + (rng.next() % 4) as u8);
    wr8(b, obj + 0xa6, (rng.next() % 3) as u8);
    wr8(b, obj + 0xa7, (rng.next() % 2) as u8);
    wr8(b, obj + 0xa8, [0u8, 0, 1, 0xff][rng.next() as usize % 4]);
    wr8(b, obj + 0x6e, 1 + (rng.next() % 4) as u8);
    wr8(b, obj + 0x53, (rng.next() % 4 == 0) as u8);
    wr8(b, obj + 0x56, (rng.next() % 4) as u8);
    wr16(b, obj + 0x32, [0u16, 0x8000, 0x0080, 0xff00][rng.next() as usize % 4]);
    wr8(b, obj + 0x29, (rng.next() % 2) as u8);
    let ox = rd16(b, obj + 8);
    wr16(b, obj + 0xa4, ox.wrapping_add((rng.next() % 0x40) as u16));
    // parent des scorpions « enfants » (+6 = 2 de l'état debout) : un objet existant, dans un état tiré
    wr16(b, obj + 0x5c, 0x85de + 0x200 * (rng.next() % 4) as u16);
    if rng.next() % 6 == 0 {
        let parent = 0xff0000 | rd16(b, obj + 0x5c) as u32;
        wr8(b, parent + 4, 4);
    }
    wr8(b, A5 + 0x42, (rng.next() % 4 == 0) as u8);
    if rng.next() % 2 == 0 {
        wr16(b, obj + 0x46, 0);
    }
}

fn gargoyle(b: &mut Bus, rng: &mut Rng, obj: u32, fresh: bool) {
    if fresh {
        // la variante 2 saute dans des données dans l'original : elle n'apparaît jamais dans le jeu
        wr8(b, obj + 3, (rng.next() % 2) as u8);
    }
    // états valides à restaurer (`+0xa2` après un demi-tour, `+0xb8` après un coup qui n'a pas touché)
    let valid = [0x0202_0000u32, 0x0202_0400, 0x0202_0600, 0x0202_0800, 0x0204_0000, 0x0204_0200, 0x0204_0400, 0x0204_0c00, 0x020a_0000];
    wr32(b, obj + 0xa2, valid[rng.next() as usize % valid.len()]);
    wr32(b, obj + 0xb8, valid[rng.next() as usize % valid.len()]);
    wr8(b, obj + 0xb7, 4 * (rng.next() % 43) as u8);
    // cible : parfois aucune (mot positif)
    if rng.next() % 8 == 0 {
        wr16(b, obj + 0x50, 0);
    }
    // victime saisie
    let victim = 0x85de + 0x200 * (rng.next() % 4) as u16;
    wr16(b, obj + 0x54, if rng.next() % 5 == 0 { 0 } else { victim });
    let v = 0xff0000 | victim as u32;
    wr8(b, v + 0x10d, [0xdu8, 0x10, 0x11, 0x13, 0x14, 0x0, 0xf][rng.next() as usize % 7]);
    wr8(b, v + 0x188, (rng.next() % 4 != 0) as u8);
    wr8(b, v, (rng.next() % 5 != 0) as u8);
    wr8(b, obj + 0x57, [0u8, 1, 4, 5][rng.next() as usize % 4]);
    wr8(b, obj + 0xaf, (rng.next() % 2) as u8);
    wr8(b, obj + 0xaa, (rng.next() % 3) as u8);
    wr8(b, obj + 0xab, 2);
    wr8(b, obj + 0xac, 2 * (rng.next() % 16) as u8);
    wr16(b, obj + 0x62, (rng.next() % 40) as u16);
    wr16(b, obj + 0x64, 40);
    wr8(b, obj + 0xad, (rng.next() % 3 == 0) as u8);
    wr8(b, obj + 0xb2, (rng.next() % 2) as u8);
    wr16(b, obj + 0xb4, [0u16, 0, 1, 5, 0x1e][rng.next() as usize % 5]);
    for off in [0x6bu32, 0x6e, 0x71] {
        wr8(b, obj + off, (rng.next() % 4) as u8);
    }
    for off in [0xa6u32, 0xa8] {
        wr16(b, obj + off, [0u16, 1, 2, 3, 0x14][rng.next() as usize % 5]);
    }
    wr8(b, obj + 0xa0, [0u8, 1, 2, 3, 4, 5, 0x10, 0x20, 0x30, 0x40][rng.next() as usize % 10]);
    wr8(b, obj + 0xa1, [0u8, 1, 2, 3, 4, 5, 0xff][rng.next() as usize % 7]);
    wr8(b, obj + 0x33, [0u8, 4, 4, 1, 0x80][rng.next() as usize % 5]);
    wr8(b, obj + 0x32, rng.next() as u8);
    wr8(b, obj + 0x86, (rng.next() % 4 == 0) as u8);
    wr8(b, obj + 0x67, (rng.next() % 2) as u8);
    wr8(b, obj + 0x79, 0x3b);
    // paramètres d'IA lus par les décisions 0xa4b4 et 0xa4c2
    for off in [0x72u32, 0x73] {
        wr8(b, obj + off, (rng.next() % 4) as u8);
    }
    wr16(b, obj + 0x84, (rng.next() % 3) as u16);
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    wr16(b, obj + 0xbc, ox.wrapping_add((rng.next() % 0x80) as u16));
    wr16(b, obj + 0xbe, oy.wrapping_add((rng.next() % 0x40) as u16));
    // joueurs placés aux distances que testent les boîtes de décision (devant ou derrière, profondeur proche)
    if rng.next() % 2 == 0 {
        let offsets = [-0x90i16, -0x3a, -0x10, 0, 0x20, 0x40, 0x80, 0x90];
        for j in 0..4u32 {
            let q = 0xff85de + 0x200 * j;
            let off = offsets[rng.next() as usize % offsets.len()] + (rng.next() % 0x12) as i16 - 9;
            wr8(b, q, (rng.next() % 5 != 0) as u8);
            wr16(b, q + 8, ox.wrapping_add(off as u16));
            wr16(b, q + 0x10, oy.wrapping_add((rng.next() % 9) as u16).wrapping_sub(4));
            wr8(b, q + 0x2c, 0x20 * (rng.next() % 2) as u8);
        }
    }
    wr8(b, A5 + 0x30, (rng.next() % 3 == 0) as u8);
    wr8(b, A5 + 0x35, (rng.next() % 5) as u8);
    wr8(b, A5 + 0x42, (rng.next() % 4 == 0) as u8);
    if rng.next() % 2 == 0 {
        wr16(b, obj + 0x46, 0);
    }
    // les caméras sont tantôt près de l'objet (hors zone sinon)
    let cam = ox.wrapping_sub(0x80 + (rng.next() % 0x100) as u16);
    wr16(b, A5 + 0x5a6, cam);
    wr16(b, A5 + 0x5ae, cam);
}

fn dog(b: &mut Bus, rng: &mut Rng, obj: u32, fresh: bool) {
    if fresh {
        wr8(b, obj + 3, (rng.next() % 8) as u8);
        wr8(b, obj + 0x2f, (rng.next() % 2) as u8);
    }
    // état à reprendre après un demi-tour : (`+0xa6`, `+0xa4`) cohérents
    let resumes: [(u32, u8); 8] = [
        (0x0202_0000, 0),
        (0x0202_0000, 4),
        (0x0204_0000, 0),
        (0x0204_0000, 8),
        (0x0206_0000, 4),
        (0x0206_0000, 10),
        (0x0208_0004, 0),
        (0x0208_0004, 2),
    ];
    let (state, sub) = resumes[rng.next() as usize % resumes.len()];
    wr32(b, obj + 0xa6, state);
    wr8(b, obj + 0xa4, sub);
    for off in [0x6bu32, 0x6e, 0x71, 0x77] {
        wr8(b, obj + off, (rng.next() % 4) as u8);
    }
    wr8(b, obj + 0xa0, (rng.next() % 4) as u8);
    wr8(b, obj + 0x90, 1 + (rng.next() % 3) as u8);
    wr8(b, obj + 0xa1, [0u8, 1, 2, 3, 4, 0xc, 0xff][rng.next() as usize % 7]);
    wr8(b, obj + 0xa2, [0u8, 1, 2, 3, 4, 0xc, 0x7f][rng.next() as usize % 7]);
    wr16(b, obj + 0xaa, [0u16, 1, 2, 0x400][rng.next() as usize % 4]);
    wr8(b, obj + 0xac, (rng.next() % 14) as u8);
    wr8(b, obj + 0xad, (rng.next() % 3 == 0) as u8);
    wr8(b, obj + 0xaf, (rng.next() % 4 == 0) as u8);
    wr8(b, obj + 0xb0, 1 + (rng.next() % 8) as u8);
    wr8(b, obj + 0x33, [0u8, 4, 1, 0x80][rng.next() as usize % 4]);
    wr8(b, obj + 0x32, rng.next() as u8);
    wr8(b, obj + 0x43, (rng.next() % 4) as u8);
    wr16(b, obj + 0x84, (rng.next() % 3) as u16);
    wr8(b, obj + 0x86, (rng.next() % 4 == 0) as u8);
    wr8(b, obj + 0x53, (rng.next() % 4 == 0) as u8);
    wr8(b, obj + 0x56, (rng.next() % 4) as u8);
    wr8(b, obj + 0x29, (rng.next() % 2) as u8);
    // paramètres d'IA lus par 0xa4b4 / 0xa4c2 et les tables de coups
    for off in [0x72u32, 0x73] {
        wr8(b, obj + off, (rng.next() % 4) as u8);
    }
    if rng.next() % 8 == 0 {
        wr16(b, obj + 0x50, 0);
    }
    // victime d'une morsure (+0x50) : vivante et à terre le plus souvent
    let v = 0xff0000 | rd16(b, obj + 0x50) as u32;
    wr8(b, v + 0x188, (rng.next() % 5 != 0) as u8);
    wr16(b, v + 0x62, [0u16, 1, 2, 5, 30][rng.next() as usize % 5]);
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    if rng.next() % 2 == 0 {
        let offsets = [-0xc0i16, -0x40, -0x28, -8, 0, 0x20, 0x40, 0x80, 0x100];
        for j in 0..4u32 {
            let q = 0xff85de + 0x200 * j;
            let off = offsets[rng.next() as usize % offsets.len()] + (rng.next() % 0x12) as i16 - 9;
            wr8(b, q, (rng.next() % 5 != 0) as u8);
            wr16(b, q + 8, ox.wrapping_add(off as u16));
            wr16(b, q + 0x10, oy.wrapping_add((rng.next() % 0x14) as u16).wrapping_sub(8));
            wr8(b, q + 0x2c, 0x20 * (rng.next() % 2) as u8);
        }
    }
    wr8(b, A5 + 0x34, if rng.next() % 3 == 0 { 4 } else { (rng.next() % 12) as u8 });
    wr8(b, A5 + 0x35, (rng.next() % 5) as u8);
    wr8(b, A5 + 0x42, (rng.next() % 4 == 0) as u8);
    if rng.next() % 2 == 0 {
        wr16(b, obj + 0x46, 0);
    }
    wr16(b, A5 + 0x6e, rng.next() as u16);
}

fn ogre(b: &mut Bus, rng: &mut Rng, obj: u32, fresh: bool) {
    if fresh {
        wr8(b, obj + 3, [0u8, 1, 2, 0xff, 0xff][rng.next() as usize % 5]);
    }
    // état à restaurer après un demi-tour
    let valid = [0x0202_0000u32, 0x0202_0400, 0x0202_0600, 0x0202_0800, 0x0204_0000, 0x0204_0200, 0x0204_0400, 0x020a_0000];
    wr32(b, obj + 0xa2, valid[rng.next() as usize % valid.len()]);
    // bras lancé (+0xb0) et ogre accompagné (+0x46) : des objets existants
    let other = 0x85de + 0x200 * (rng.next() % 4) as u16;
    // pendant l'arrivée le bras n'existe pas encore : la fin d'attente (0x75c36) y relit `+0xb1`
    let arrival = rd8(b, obj + 5) == 0;
    wr16(b, obj + 0xb0, if arrival { 0 } else { other });
    wr16(b, obj + 0x46, other);
    let a = 0xff0000 | other as u32;
    wr8(b, a + 0x5a, (rng.next() % 2) as u8);
    wr16(b, a + 0x62, [0u16, 5, 30][rng.next() as usize % 3]);
    if rng.next() % 8 == 0 {
        wr16(b, obj + 0x50, 0);
    }
    wr8(b, obj + 0xaa, (rng.next() % 3) as u8);
    wr8(b, obj + 0xab, 2);
    wr8(b, obj + 0xac, 2 * (rng.next() % 16) as u8);
    wr16(b, obj + 0x62, [0u16, 5, 30][rng.next() as usize % 3]);
    wr16(b, obj + 0x64, 40);
    wr8(b, obj + 0xad, (rng.next() % 3 == 0) as u8);
    wr8(b, obj + 0xb2, (rng.next() % 2) as u8);
    wr8(b, obj + 0xb3, (rng.next() % 2) as u8);
    for off in [0x6bu32, 0x6e, 0x71] {
        wr8(b, obj + off, (rng.next() % 4) as u8);
    }
    for off in [0xa6u32, 0xa8] {
        wr16(b, obj + off, [0u16, 1, 2, 3, 0x14, 0xa, 0x28][rng.next() as usize % 7]);
    }
    wr8(b, obj + 0xa0, [0u8, 1, 2, 3, 4, 5, 0x10, 0x20, 0x31, 0x40][rng.next() as usize % 10]);
    wr8(b, obj + 0xa1, [0u8, 1, 2, 3, 4, 5, 0xff][rng.next() as usize % 7]);
    wr8(b, obj + 0x33, [0u8, 1, 2, 3, 4, 0x80, 0x82, 0x84][rng.next() as usize % 8]);
    wr8(b, obj + 0x32, rng.next() as u8);
    wr8(b, obj + 0x86, (rng.next() % 4 == 0) as u8);
    wr8(b, obj + 0x67, (rng.next() % 2) as u8);
    wr16(b, obj + 0x84, (rng.next() % 3) as u16);
    for off in [0x72u32, 0x73] {
        wr8(b, obj + off, (rng.next() % 4) as u8);
    }
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    wr16(b, obj + 0xbc, ox.wrapping_add((rng.next() % 0x80) as u16));
    if rng.next() % 2 == 0 {
        let offsets = [-0xc0i16, -0x80, -0x60, -0x40, -0x19, 0, 0x20, 0x40, 0x80];
        for j in 0..4u32 {
            let q = 0xff85de + 0x200 * j;
            let off = offsets[rng.next() as usize % offsets.len()] + (rng.next() % 0x12) as i16 - 9;
            wr8(b, q, (rng.next() % 5 != 0) as u8);
            wr16(b, q + 8, ox.wrapping_add(off as u16));
            wr16(b, q + 0x10, oy.wrapping_add((rng.next() % 9) as u16).wrapping_sub(4));
            wr8(b, q + 0x2c, 0x20 * (rng.next() % 2) as u8);
            // joueur à terre ou en position de saisie : 0x7ef4 et 0x7f0c testent son état
            wr8(b, q + 6, [0u8, 0xc, 0xc, 0xe][rng.next() as usize % 4]);
            wr16(b, q + 0xc, 0);
        }
    }
    wr8(b, A5 + 0x30, (rng.next() % 3 == 0) as u8);
    wr8(b, A5 + 0x35, (rng.next() % 5) as u8);
    wr8(b, A5 + 0x42, (rng.next() % 4 == 0) as u8);
    wr8(b, A5 + 0x48, (rng.next() % 9) as u8);
    wr8(b, A5 + 0x2c, if rng.next() % 4 == 0 { 0xc } else { 0 });
    if rng.next() % 2 == 0 {
        wr16(b, obj + 0x46, 0x85de);
    }
    let cam = ox.wrapping_sub(0x80 + (rng.next() % 0x100) as u16);
    wr16(b, A5 + 0x5a6, cam);
    wr16(b, A5 + 0x5ae, cam);
    wr16(b, A5 + 0x5aa, oy.wrapping_sub((rng.next() % 0x100) as u16));
}

fn kobold(b: &mut Bus, rng: &mut Rng, obj: u32, fresh: bool) {
    if fresh {
        wr8(b, obj + 3, (rng.next() % 14) as u8);
        wr8(b, obj + 0x2f, (rng.next() % 4) as u8);
    }
    // état à reprendre après un demi-tour : (`+0xa6`, `+0xa5`) cohérents
    let resumes: [(u32, u8); 8] = [
        (0x0202_0000, 0),
        (0x0202_0000, 4),
        (0x0204_0000, 0),
        (0x0204_0000, 8),
        (0x0206_0000, 4),
        (0x0206_0000, 8),
        (0x0208_0004, 0),
        (0x0208_0004, 2),
    ];
    let (state, sub) = resumes[rng.next() as usize % resumes.len()];
    wr32(b, obj + 0xa6, state);
    wr8(b, obj + 0xa5, sub);
    for off in [0x6bu32, 0x6e, 0x71, 0x77] {
        wr8(b, obj + off, (rng.next() % 4) as u8);
    }
    wr16(b, obj + 0xa0, [0u16, 1, 2, 5, 0x14][rng.next() as usize % 5]);
    wr8(b, obj + 0xa2, [0u8, 1, 2, 3, 4, 0xc, 0x7f][rng.next() as usize % 7]);
    // aspect précédent : souvent celui que le geste va reprendre (le décompte `+0xad` n'a lieu que s'ils sont égaux)
    let resume = [0u8, 1, 2, 3, 4, 0xc, 0x7f, 0x16, 0x1a, 0x1c, 0x1e, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f];
    wr8(b, obj + 0xa3, resume[rng.next() as usize % resume.len()]);
    wr8(b, obj + 0xaa, 1 + (rng.next() % 3) as u8);
    wr8(b, obj + 0xac, 1 + (rng.next() % 3) as u8);
    wr8(b, obj + 0xad, 1 + (rng.next() % 3) as u8);
    wr8(b, obj + 0x33, [0u8, 4, 1, 0x80][rng.next() as usize % 4]);
    wr8(b, obj + 0x32, rng.next() as u8);
    wr16(b, obj + 0x42, (rng.next() % 4) as u16);
    wr8(b, obj + 0x44, (rng.next() % 4) as u8);
    wr16(b, obj + 0x84, (rng.next() % 3) as u16);
    wr8(b, obj + 0x86, (rng.next() % 4 == 0) as u8);
    wr8(b, obj + 0x53, [0u8, 0, 0, 1, 0xff][rng.next() as usize % 5]);
    wr8(b, obj + 0x56, (rng.next() % 4) as u8);
    wr8(b, obj + 0x29, (rng.next() % 2) as u8);
    for off in [0x72u32, 0x73] {
        wr8(b, obj + off, (rng.next() % 5) as u8);
    }
    if rng.next() % 8 == 0 {
        wr16(b, obj + 0x50, 0);
    }
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    if rng.next() % 2 == 0 {
        let offsets = [-0xb0i16, -0x40, -0x30, -0x28, 0, 0x20, 0x40, 0x80, 0x100];
        for j in 0..4u32 {
            let q = 0xff85de + 0x200 * j;
            let off = offsets[rng.next() as usize % offsets.len()] + (rng.next() % 0x12) as i16 - 9;
            wr8(b, q, (rng.next() % 5 != 0) as u8);
            wr16(b, q + 8, ox.wrapping_add(off as u16));
            wr16(b, q + 0x10, oy.wrapping_add((rng.next() % 0x14) as u16).wrapping_sub(8));
            wr8(b, q + 0x2c, 0x20 * (rng.next() % 2) as u8);
        }
    }
    wr8(b, A5 + 0x34, [4u8, 6, 0xc, 1, 2][rng.next() as usize % 5]);
    wr8(b, A5 + 0x35, (rng.next() % 5) as u8);
    wr8(b, A5 + 0x42, (rng.next() % 4 == 0) as u8);
    if rng.next() % 2 == 0 {
        wr16(b, obj + 0x46, 0);
    }
    wr16(b, A5 + 0x6e, rng.next() as u16);
}

fn goblin(b: &mut Bus, rng: &mut Rng, obj: u32, _fresh: bool) {
    // variante paire : 0 marche, 2 et 4 chute, 6 course ; en arrivée, +6 = 6 n'existe que pour les variantes 2 et 4
    // et la variante 6 n'a que trois étapes
    let (s5, s6) = (rd8(b, obj + 5), rd8(b, obj + 6));
    let variant = if s5 == 0 && s6 == 6 { [2u8, 4][rng.next() as usize % 2] } else { 2 * (rng.next() % 4) as u8 };
    wr8(b, obj + 3, variant);
    for off in [0x6bu32, 0x6e, 0x71] {
        wr8(b, obj + off, (rng.next() % 4) as u8);
    }
    wr8(b, obj + 0x77, (rng.next() % 3) as u8);
    wr8(b, obj + 0xa2, [0u8, 1, 2, 5, 0x14][rng.next() as usize % 5]);
    wr8(b, obj + 0xa4, (rng.next() % 3) as u8);
    wr8(b, obj + 0xa1, [0u8, 1, 0x80][rng.next() as usize % 3]);
    // geste mémorisé : un `+6` valide du sous-état 4
    // (le sous-état 6 n'a que sept gestes : l'original sauterait dans des données au-delà)
    let gestures = if s5 == 6 { 7 } else { 11 };
    wr8(b, obj + 0xaa, 2 * (rng.next() % gestures) as u8);
    wr8(b, obj + 0xae, (rng.next() % 6 == 0) as u8);
    wr8(b, obj + 0x33, [0u8, 4, 1, 0x80][rng.next() as usize % 4]);
    wr8(b, obj + 0x32, rng.next() as u8);
    wr8(b, obj + 0x3b, rng.next() as u8);
    wr16(b, obj + 0x84, (rng.next() % 3) as u16);
    wr8(b, obj + 0x86, (rng.next() % 4 == 0) as u8);
    wr8(b, obj + 0x53, [0u8, 0, 0, 1, 0xff][rng.next() as usize % 5]);
    wr8(b, obj + 0x29, (rng.next() % 2) as u8);
    wr8(b, obj + 0x67, (rng.next() % 2) as u8);
    wr8(b, obj + 0x79, 0x1a);
    // paramètres d'IA lus par 0xa4b4 et 0xa4c2
    for off in [0x72u32, 0x73] {
        wr8(b, obj + off, (rng.next() % 5) as u8);
    }
    if rng.next() % 8 == 0 {
        wr16(b, obj + 0x50, 0);
    }
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    if rng.next() % 2 == 0 {
        let offsets = [-0xd0i16, -0xa0, -0x38, -0x20, 0, 0x20, 0x38, 0x70, 0x100];
        for j in 0..4u32 {
            let q = 0xff85de + 0x200 * j;
            let off = offsets[rng.next() as usize % offsets.len()] + (rng.next() % 0x12) as i16 - 9;
            wr8(b, q, (rng.next() % 5 != 0) as u8);
            wr16(b, q + 8, ox.wrapping_add(off as u16));
            wr16(b, q + 0x10, oy.wrapping_add((rng.next() % 0x30) as u16).wrapping_sub(0x18));
            wr8(b, q + 0x2c, 0x20 * (rng.next() % 2) as u8);
            // joueur à terre ou en position de saisie : 0x7f0c teste son état
            wr8(b, q + 6, [0u8, 0xc, 0xc, 0xe][rng.next() as usize % 4]);
            wr16(b, q + 0xc, 0);
        }
    }
    wr8(b, A5 + 0x30, (rng.next() % 3 == 0) as u8);
    wr8(b, A5 + 0x35, (rng.next() % 5) as u8);
    wr8(b, A5 + 0x42, (rng.next() % 4 == 0) as u8);
    if rng.next() % 2 == 0 {
        wr16(b, obj + 0x46, 0);
    }
    let cam = ox.wrapping_sub(0x80 + (rng.next() % 0x100) as u16);
    wr16(b, A5 + 0x506, cam);
    wr16(b, A5 + 0x556, cam);
    wr16(b, A5 + 0x5a6, cam);
    wr16(b, A5 + 0x5ae, cam);
    wr16(b, A5 + 0x7f92, (rng.next() % 32) as u16);
}

fn troll(b: &mut Bus, rng: &mut Rng, obj: u32, _fresh: bool) {
    // variante paire : 0 marche, 2 et 4 chute ; en arrivée, +6 = 6 n'existe que pour les variantes 2 et 4
    let (s5, s6) = (rd8(b, obj + 5), rd8(b, obj + 6));
    let variant = if s5 == 0 && s6 == 6 { [2u8, 4][rng.next() as usize % 2] } else { 2 * (rng.next() % 3) as u8 };
    wr8(b, obj + 3, variant);
    for off in [0x6bu32, 0x6e, 0x71] {
        wr8(b, obj + off, (rng.next() % 4) as u8);
    }
    wr8(b, obj + 0x77, (rng.next() % 3) as u8);
    wr8(b, obj + 0xa2, [0u8, 1, 2, 5, 0x14][rng.next() as usize % 5]);
    wr8(b, obj + 0xa4, (rng.next() % 3) as u8);
    wr8(b, obj + 0xa1, [0u8, 1, 0x80, 4, 8, 0x12][rng.next() as usize % 6]);
    // geste mémorisé : un `+6` valide du sous-état 4
    let gestures = if s5 == 6 { 7 } else { 11 };
    wr8(b, obj + 0xaa, 2 * (rng.next() % gestures) as u8);
    wr8(b, obj + 0xae, (rng.next() % 6 == 0) as u8);
    wr8(b, obj + 0xab, [0u8, 0x10, 0x20, 0xf0][rng.next() as usize % 4]);
    wr8(b, obj + 0xac, [0u8, 0, 1, 0xb4][rng.next() as usize % 4]);
    // objet lancé : un joueur existant
    let launches = 0x85de + 0x200 * (rng.next() % 4);
    wr32(b, obj + 0xa6, 0xff0000 | launches);
    wr8(b, obj + 0x33, [0u8, 4, 1, 0x80][rng.next() as usize % 4]);
    wr8(b, obj + 0x32, rng.next() as u8);
    wr8(b, obj + 0x3b, rng.next() as u8);
    wr16(b, obj + 0x84, (rng.next() % 3) as u16);
    wr8(b, obj + 0x86, (rng.next() % 4 == 0) as u8);
    wr8(b, obj + 0x53, [0u8, 0, 0, 1, 0xff][rng.next() as usize % 5]);
    wr8(b, obj + 0x29, (rng.next() % 2) as u8);
    wr8(b, obj + 0x67, (rng.next() % 2) as u8);
    wr8(b, obj + 0x79, 0x1a);
    for off in [0x72u32, 0x73] {
        wr8(b, obj + off, (rng.next() % 5) as u8);
    }
    if rng.next() % 8 == 0 {
        wr16(b, obj + 0x50, 0);
    }
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    let nobody = rng.next() % 8 == 0;
    if rng.next() % 2 == 0 {
        let offsets = [-0xd0i16, -0xa0, -0x38, -0x20, 0, 0x20, 0x38, 0x70, 0x100];
        for j in 0..4u32 {
            let q = 0xff85de + 0x200 * j;
            let off = offsets[rng.next() as usize % offsets.len()] + (rng.next() % 0x12) as i16 - 9;
            wr8(b, q, (!nobody && rng.next() % 5 != 0) as u8);
            wr16(b, q + 8, ox.wrapping_add(off as u16));
            wr16(b, q + 0x10, oy.wrapping_add((rng.next() % 0x30) as u16).wrapping_sub(0x18));
            wr8(b, q + 0x2c, 0x20 * (rng.next() % 2) as u8);
            wr8(b, q + 6, [0u8, 0xc, 0xc, 0xe][rng.next() as usize % 4]);
            wr16(b, q + 0xc, 0);
            // l'objet lancé : état +4 de 0 à 5
            wr8(b, q + 4, (rng.next() % 6) as u8);
        }
    }
    wr8(b, A5 + 0x30, (rng.next() % 3 == 0) as u8);
    wr8(b, A5 + 0x35, (rng.next() % 5) as u8);
    wr8(b, A5 + 0x42, (rng.next() % 4 == 0) as u8);
    if rng.next() % 2 == 0 {
        wr16(b, obj + 0x46, 0);
    }
    let cam = ox.wrapping_sub(0x80 + (rng.next() % 0x100) as u16);
    for off in [0x506u32, 0x556, 0x50e, 0x55e, 0x5a6, 0x5ae] {
        wr16(b, A5 + off, cam);
    }
    wr16(b, A5 + 0x7f92, (rng.next() % 32) as u16);
}

fn gnoll(b: &mut Bus, rng: &mut Rng, obj: u32, _fresh: bool) {
    // variante paire : 0 marche, 2 et 6 chute, 4 saut de côté, 8 et 10 arrivée armée ; en arrivée, les +6 grands
    // n'existent que pour certaines variantes (6 : toutes sauf 10, 8 : 0 et 4) ; debout, la variante 10 est saisie
    let (s4, s5, s6) = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    let variant = match (s5, s6) {
        (0, 8) => [0u8, 4][rng.next() as usize % 2],
        (0, 6) => [0u8, 2, 4, 6, 8][rng.next() as usize % 5],
        (0, _) => 2 * (rng.next() % 6) as u8,
        _ if s4 == 2 && s5 == 2 && rng.next() % 6 != 0 => 2 * (rng.next() % 5) as u8,
        _ => 2 * (rng.next() % 6) as u8,
    };
    wr8(b, obj + 3, variant);
    for off in [0x6bu32, 0x6e, 0x71] {
        wr8(b, obj + off, (rng.next() % 4) as u8);
    }
    wr8(b, obj + 0x77, (rng.next() % 3) as u8);
    wr8(b, obj + 0xa2, [0u8, 1, 2, 5, 0x14][rng.next() as usize % 5]);
    wr8(b, obj + 0xa4, (rng.next() % 3) as u8);
    wr8(b, obj + 0xa1, [0u8, 1, 0x80, 4, 6, 8, 0x12][rng.next() as usize % 7]);
    // geste mémorisé : un `+6` valide du sous-état 4 (et 6)
    let gestures = if s5 == 6 { 7 } else { 13 };
    wr8(b, obj + 0xa5, 2 * (rng.next() % gestures) as u8);
    wr8(b, obj + 0xa6, (rng.next() % 2) as u8);
    wr8(b, obj + 0xbf, (rng.next() % 2) as u8);
    wr8(b, obj + 0x33, [0u8, 1, 4, 9, 0xb, 0x80][rng.next() as usize % 6]);
    wr8(b, obj + 0x32, rng.next() as u8);
    wr8(b, obj + 0x3b, rng.next() as u8);
    wr16(b, obj + 0x84, (rng.next() % 3) as u16);
    wr8(b, obj + 0x86, (rng.next() % 4 == 0) as u8);
    wr8(b, obj + 0x53, [0u8, 0, 0, 1, 0xff][rng.next() as usize % 5]);
    wr8(b, obj + 0x29, (rng.next() % 2) as u8);
    wr8(b, obj + 0x67, (rng.next() % 2) as u8);
    wr8(b, obj + 0x79, 0x1a);
    for off in [0x72u32, 0x73] {
        wr8(b, obj + off, (rng.next() % 5) as u8);
    }
    if rng.next() % 8 == 0 {
        wr16(b, obj + 0x50, 0);
    }
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    if rng.next() % 2 == 0 {
        let offsets = [-0xd0i16, -0x80, -0x38, -0x20, 0, 0x20, 0x30, 0x70, 0x100];
        for j in 0..4u32 {
            let q = 0xff85de + 0x200 * j;
            let off = offsets[rng.next() as usize % offsets.len()] + (rng.next() % 0x12) as i16 - 9;
            wr8(b, q, (rng.next() % 5 != 0) as u8);
            wr16(b, q + 8, ox.wrapping_add(off as u16));
            wr16(b, q + 0x10, oy.wrapping_add((rng.next() % 0x30) as u16).wrapping_sub(0x18));
            wr8(b, q + 0x2c, 0x20 * (rng.next() % 2) as u8);
            wr8(b, q + 6, [0u8, 0xc, 0xc, 0xe][rng.next() as usize % 4]);
            wr16(b, q + 0xc, 0);
        }
    }
    wr8(b, A5 + 0x30, (rng.next() % 3 == 0) as u8);
    wr8(b, A5 + 0x35, (rng.next() % 5) as u8);
    wr8(b, A5 + 0x42, (rng.next() % 4 == 0) as u8);
    if rng.next() % 2 == 0 {
        wr16(b, obj + 0x46, 0);
    }
    let cam = ox.wrapping_sub(0x80 + (rng.next() % 0x100) as u16);
    for off in [0x506u32, 0x556, 0x5a6, 0x5ae] {
        wr16(b, A5 + off, cam);
    }
    wr16(b, A5 + 0x7f92, (rng.next() % 32) as u16);
}

fn archer(b: &mut Bus, rng: &mut Rng, obj: u32, _fresh: bool) {
    let (s4, s5, s6) = (rd8(b, obj + 4), rd8(b, obj + 5), rd8(b, obj + 6));
    // variante paire : 0 et 2 marche, 4 chute, 6 apparition, 8 course
    wr8(b, obj + 3, 2 * (rng.next() % 5) as u8);
    // créé par un autre objet : seuls les états d'arrivée et de tir en place existent
    let created = s4 == 2 && s5 <= 2 && s6 <= 4 && rng.next() % 3 == 0;
    wr8(b, obj + 0x42, created as u8);
    if created {
        wr8(b, obj + 7, 2 * (rng.next() % 3) as u8);
    }
    wr8(b, obj + 0x43, [0u8, 0, 1, 0x80][rng.next() as usize % 4]);
    wr16(b, obj + 0xa8, [0u16, 1, 2, 0x80, 0xffff][rng.next() as usize % 5]);
    wr8(b, obj + 0xa7, (rng.next() % 8 == 0) as u8);
    wr8(b, obj + 0xac, (rng.next() % 8 == 0) as u8);
    wr8(b, obj + 0xa4, [0u8, 1, 2, 5, 0x20][rng.next() as usize % 5]);
    wr8(b, obj + 0xa5, 1 + (rng.next() % 3) as u8);
    // étape à reprendre après une marche latérale (2 ou 6 dans le jeu)
    wr8(b, obj + 0xa6, [2u8, 6][rng.next() as usize % 2]);
    // état à reprendre après un demi-tour
    let valid = [0x0202_0000u32, 0x0202_0200, 0x0204_0000, 0x0204_0202, 0x0208_0000, 0x020a_0000];
    wr32(b, obj + 0xa0, valid[rng.next() as usize % valid.len()]);
    for off in [0x6bu32, 0x6e, 0x71] {
        wr8(b, obj + off, (rng.next() % 4) as u8);
    }
    wr8(b, obj + 0x2f, (rng.next() % 2) as u8);
    wr8(b, obj + 0x33, [0u8, 1, 4, 0x80][rng.next() as usize % 4]);
    wr8(b, obj + 0x32, rng.next() as u8);
    wr16(b, obj + 0x84, (rng.next() % 3) as u16);
    wr8(b, obj + 0x86, (rng.next() % 4 == 0) as u8);
    wr8(b, obj + 0x53, [0u8, 0, 0, 1, 0xff][rng.next() as usize % 5]);
    wr8(b, obj + 0x79, 3);
    if rng.next() % 6 == 0 {
        wr16(b, obj + 0x50, 0);
    }
    // créateur éventuel (le bit 7 de `+0x43` fait agir sur lui à la sortie de l'écran)
    let host = 0x85de + 0x200 * (rng.next() % 4) as u16;
    wr16(b, obj + 0x46, if rng.next() % 3 == 0 { 0 } else { host });
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    let nobody = rng.next() % 8 == 0;
    if rng.next() % 2 == 0 {
        let offsets = [-0xd0i16, -0x80, -0x38, -0x28, 0, 0x20, 0x38, 0x60, 0x100];
        for j in 0..4u32 {
            let q = 0xff85de + 0x200 * j;
            let off = offsets[rng.next() as usize % offsets.len()] + (rng.next() % 0x12) as i16 - 9;
            wr8(b, q, (!nobody && rng.next() % 5 != 0) as u8);
            wr16(b, q + 8, ox.wrapping_add(off as u16));
            wr16(b, q + 0x10, oy.wrapping_add((rng.next() % 0x30) as u16).wrapping_sub(0x18));
            wr8(b, q + 0x2c, 0x20 * (rng.next() % 2) as u8);
            wr8(b, q + 6, [0u8, 0xc, 0xc, 0xe][rng.next() as usize % 4]);
            wr16(b, q + 0xc, 0);
        }
    }
    wr8(b, A5 + 0x34, [4u8, 6, 0xc, 1][rng.next() as usize % 4]);
    wr8(b, A5 + 0x35, (rng.next() % 5) as u8);
    wr8(b, A5 + 0x42, (rng.next() % 4 == 0) as u8);
    let cam = ox.wrapping_sub(0x80 + (rng.next() % 0x100) as u16);
    for off in [0x506u32, 0x556, 0x50e, 0x55e, 0x5a6, 0x5ae] {
        wr16(b, A5 + off, cam);
    }
    wr16(b, A5 + 0x7f92, (rng.next() % 32) as u16);
}

fn golem(b: &mut Bus, rng: &mut Rng, obj: u32, _fresh: bool) {
    wr8(b, obj + 0x2f, (rng.next() % 2) as u8);
    wr8(b, obj + 0x77, (rng.next() % 4) as u8);
    // drapeau de coup reçu : presque toujours nul
    wr8(b, obj + 0x8f, [0u8, 0, 0, 0, 1, 2, 3][rng.next() as usize % 7]);
    wr8(b, obj + 0xa6, [0u8, 1, 2, 3][rng.next() as usize % 4]);
    wr16(b, obj + 0xa2, [0u16, 1, 2, 5, 0x10][rng.next() as usize % 5]);
    wr16(b, obj + 0xa4, rng.next() as u16);
    wr8(b, obj + 0xa7, (rng.next() % 2) as u8);
    wr8(b, obj + 0x29, (rng.next() % 2) as u8);
    wr8(b, obj + 0x33, [0u8, 1, 4, 0x80][rng.next() as usize % 4]);
    wr16(b, obj + 0x30, [1u16, 2, 5][rng.next() as usize % 3]);
    wr16(b, obj + 0x32, [0u16, 0, 1, 0xffff][rng.next() as usize % 4]);
    wr16(b, obj + 0x84, (rng.next() % 3) as u16);
    wr8(b, obj + 0x86, (rng.next() % 4 == 0) as u8);
    wr8(b, obj + 0x53, [0u8, 0, 0, 1, 0xff][rng.next() as usize % 5]);
    for off in [0x6bu32, 0x6e, 0x71] {
        wr8(b, obj + off, (rng.next() % 4) as u8);
    }
    for off in [0x72u32, 0x73] {
        wr8(b, obj + off, (rng.next() % 5) as u8);
    }
    if rng.next() % 8 == 0 {
        wr16(b, obj + 0x50, 0);
    }
    // l'objet qui l'a touché (+0x54) : un joueur existant, dont l'octet +0x57 donne le gel
    let key = 0x85de + 0x200 * (rng.next() % 4) as u16;
    wr16(b, obj + 0x54, key);
    wr8(b, 0xff0000 | key as u32 + 0x57, [0u8, 4, 8, 0x20][rng.next() as usize % 4]);
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    if rng.next() % 2 == 0 {
        let offsets = [-0xd0i16, -0xa0, -0x68, -0x20, -8, 0, 0x30, 0x68, 0x100];
        for j in 0..4u32 {
            let q = 0xff85de + 0x200 * j;
            let off = offsets[rng.next() as usize % offsets.len()] + (rng.next() % 0x12) as i16 - 9;
            wr8(b, q, (rng.next() % 5 != 0) as u8);
            wr16(b, q + 8, ox.wrapping_add(off as u16));
            wr16(b, q + 0x10, oy.wrapping_add((rng.next() % 0x20) as u16).wrapping_sub(0x10));
            wr8(b, q + 0x2c, 0x20 * (rng.next() % 2) as u8);
            wr16(b, q + 0xc, 0);
        }
    }
    // touches du quatrième joueur
    wr8(b, A5 + 3, rng.next() as u8);
    wr8(b, A5 + 0xf, rng.next() as u8);
    wr8(b, A5 + 0x35, (rng.next() % 5) as u8);
    wr8(b, A5 + 0x36, (rng.next() % 4 == 0) as u8);
    let cam = ox.wrapping_sub(0x80 + (rng.next() % 0x180) as u16);
    for off in [0x506u32, 0x556, 0x5a6, 0x5ae] {
        wr16(b, A5 + off, cam);
    }
    wr16(b, A5 + 0x50a, oy);
    wr16(b, A5 + 0x55a, oy);
    wr16(b, A5 + 0x7f92, (rng.next() % 32) as u16);
}

fn machine(b: &mut Bus, rng: &mut Rng, obj: u32, fresh: bool) {
    if fresh {
        wr8(b, obj + 3, (rng.next() % 3) as u8);
    }
    wr8(b, obj + 0x2f, (rng.next() % 3 != 0) as u8);
    wr8(b, obj, 1);
    wr8(b, obj + 0xaa, (rng.next() % 3) as u8);
    wr8(b, obj + 0xab, 2);
    wr8(b, obj + 0xac, 2 * (rng.next() % 16) as u8);
    for off in [0x6bu32, 0x6e, 0x71] {
        wr8(b, obj + off, (rng.next() % 4) as u8);
    }
    wr8(b, obj + 0x6d, (rng.next() % 9) as u8);
    // aspect courant et précédent (valeurs prises par la machine)
    let aspects = [0u8, 1, 2, 3, 4, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x20, 0x30, 0x31, 0x32, 0x40, 0xff];
    wr8(b, obj + 0xa0, aspects[rng.next() as usize % aspects.len()]);
    wr8(b, obj + 0xa1, aspects[rng.next() as usize % aspects.len()]);
    // état à reprendre après un demi-tour
    let valid = [0x0202_0000u32, 0x0202_0400, 0x0202_0600, 0x0202_0800, 0x0204_0000, 0x020a_0000];
    wr32(b, obj + 0xa2, valid[rng.next() as usize % valid.len()]);
    for off in [0xa6u32, 0xa8] {
        wr16(b, obj + off, [0u16, 1, 2, 3, 0x14, 0x1e][rng.next() as usize % 6]);
    }
    wr8(b, obj + 0xad, (rng.next() % 4 == 0) as u8);
    wr8(b, obj + 0xae, [0u8, 1, 2, 5][rng.next() as usize % 4]);
    wr8(b, obj + 0xb2, (rng.next() % 2) as u8);
    wr8(b, obj + 0x33, [0u8, 0, 1, 2, 0x80][rng.next() as usize % 5]);
    wr8(b, obj + 0x32, rng.next() as u8);
    wr16(b, obj + 0x62, [0u16, 5, 30][rng.next() as usize % 3]);
    wr16(b, obj + 0x84, (rng.next() % 3) as u16);
    wr8(b, obj + 0x86, (rng.next() % 4 == 0) as u8);
    wr8(b, obj + 0x53, [0u8, 0, 0, 1, 0xff][rng.next() as usize % 5]);
    wr16(b, obj + 0x14, [0u16, 0x200, 0xfe00, 0x300, 0xfd00][rng.next() as usize % 5]);
    wr16(b, obj + 0x16, [0u16, 0x20, 0xffe0][rng.next() as usize % 3]);
    wr16(b, obj + 0x18, [0u16, 0x400, 0xff00, 0x80, 0x500][rng.next() as usize % 5]);
    wr16(b, obj + 0x1a, 0xffc0);
    wr8(b, obj + 0x67, (rng.next() % 2) as u8);
    for off in [0x72u32, 0x73] {
        wr8(b, obj + off, (rng.next() % 3) as u8);
    }
    if rng.next() % 8 == 0 {
        wr16(b, obj + 0x50, 0);
    }
    let (ox, oy) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
    if rng.next() % 2 == 0 {
        let offsets = [-0xe0i16, -0x80, -0x50, -0x30, -0x10, 0, 0x30, 0x60, 0x100];
        for j in 0..4u32 {
            let q = 0xff85de + 0x200 * j;
            let off = offsets[rng.next() as usize % offsets.len()] + (rng.next() % 0x12) as i16 - 9;
            wr8(b, q, (rng.next() % 5 != 0) as u8);
            wr16(b, q + 8, ox.wrapping_add(off as u16));
            wr16(b, q + 0x10, oy.wrapping_add((rng.next() % 0x30) as u16).wrapping_sub(0x18));
            wr8(b, q + 0x2c, 0x20 * (rng.next() % 2) as u8);
            wr8(b, q + 6, [0u8, 0xc, 0xc, 0xe][rng.next() as usize % 4]);
            wr16(b, q + 0xc, 0);
        }
    }
    wr8(b, A5 + 0x30, (rng.next() % 3 == 0) as u8);
    // saisie : `A5+0x42` et le lien `+0x46` (2810)
    wr8(b, A5 + 0x42, (rng.next() % 8 == 0) as u8);
    wr16(b, obj + 0x46, if rng.next() % 4 == 0 { 0x85de } else { 0 });
    wr8(b, A5 + 0x35, (rng.next() % 5) as u8);
    wr8(b, A5 + 0x36, (rng.next() % 4 == 0) as u8);
    wr8(b, A5 + 0x48, (rng.next() % 9) as u8);
    let cam = if rng.next() % 4 == 0 {
        ox.wrapping_add((rng.next() % 0x100) as u16)
    } else {
        ox.wrapping_sub(0x80 + (rng.next() % 0x180) as u16)
    };
    for off in [0x5a6u32, 0x5ae, 0x506, 0x556] {
        wr16(b, A5 + off, cam);
    }
    wr16(b, A5 + 0x7f92, (rng.next() % 32) as u16);
    wr8(b, A5 + 0x7f93, (rng.next() % 32) as u8);
}
