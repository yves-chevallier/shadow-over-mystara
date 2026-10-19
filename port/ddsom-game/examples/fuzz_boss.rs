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
        _ => &[(&[0, 2, 4, 6], &[0, 2, 4], &[0, 2, 4])],
    }
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
fn prep(kind: u8, b: &mut Bus, alea: &mut Alea, obj: u32) {
    if kind == 17 {
        prep_17(b, alea, obj);
    }
    if kind == 6 {
        prep_6(b, alea, obj);
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
                c.a[0] = obj.0;
                c.a[5] = A5;
                c.d[7] = pas;
                c.d[0] = 4 * kind as u32;
                c.a[1] = entree;
                essais += 1;
                let avant = ddsom_game::report().values().map(|s| s.mismatches).sum::<u64>();
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
                if apres > avant && affiches < 12 {
                    affiches += 1;
                    let dernier = ddsom_game::report().values().filter_map(|s| s.last.clone()).next();
                    println!("écart en {entree:06x} type {kind} pas {pas} : {}", dernier.unwrap_or_default());
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
                            println!("    {:06x} {:?}", c2.pc, ins.op);
                            c2.step(&mut b2);
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
