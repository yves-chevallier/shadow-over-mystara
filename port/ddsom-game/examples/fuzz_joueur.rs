//! Fuzz des gestionnaires des tirs (PSHL) et des effets (PEFF) des joueurs, réécrits dans
//! `families/pshl*.rs` et `families/peff*.rs`.
//!
//! Chaque gestionnaire natif est appelé en mode contrôle (l'original s'exécute à côté sur une copie, les
//! deux sont comparés : mémoire, registres du contrat, avec `DDSOM_CHECK_ALL=1` tous les registres) sur
//! un objet fabriqué dans un état tiré au sort : joueur propriétaire et lanceur aux champs variés, parent
//! et frères factices, états de l'automate, compteurs et drapeaux de la scène. L'objet vit ensuite
//! quelques dizaines d'images (le propriétaire change de geste, le moteur de collisions pose ses drapeaux...).
//!
//!   fuzz_joueur <roms> <eeprom> <scénario> <images de chauffe> <tours> [PSHL|PEFF]
//!
//! Variables : `DDSOM_ENTREE=<adresse>` (un seul gestionnaire), `DDSOM_GRAINE`, `DDSOM_PAS` (images par
//! objet, 40 par défaut), `DDSOM_VARIANTES` (variantes tirées, 3 par défaut), `DDSOM_VERBOSE`.

use std::path::Path;

use ddsom_game::mem::{rd16, rd8, wr16, wr32, wr8, A5};
use ddsom_game::objects::pools::{Pool, PEFF, PSHL};
use m68k::decode::Fetch;

struct Alea(u64);

impl Alea {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn dans(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn parmi<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.dans(v.len() as u32) as usize]
    }
}

/// Octets du joueur propriétaire que les gestionnaires testent : (décalage, valeurs plausibles).
const JOUEUR_OCTETS: &[(u32, &[u8])] = &[
    (0x03, &[0, 2, 4, 6, 8, 0x0a, 0x0b, 0x0e]),
    (0x05, &[0, 2, 4]),
    (0x07, &[0, 1, 2, 3, 4, 6, 0x0e, 0x10, 0x12]),
    (0x2c, &[0, 0x20]),
    (0x33, &[0, 1, 0xff]),
    (0x53, &[0, 1]),
    (0x6a, &[0, 0x16, 0x17, 0x18, 0x25]),
    (0x6c, &[0, 0x40]),
    (0x6e, &[0, 0x10, 0x20, 0x40, 0x80]),
    (0x78, &[0, 1]),
    (0x10c, &[0, 1, 2, 3, 4, 5, 10, 14, 15, 16, 19, 20, 21]),
    (0x10d, &[0, 0x10, 0x11, 0x12, 0x13]),
    (0x10e, &[0, 0x20, 0x21]),
    (0x10f, &[0, 1]),
    (0x15d, &[0, 1, 2]),
    (0x15e, &[0, 0x15, 0x16, 0x17, 0x18, 0x1a, 0x24, 0x25, 0x27]),
    (0x166, &[0, 4]),
    (0x1e9, &[0, 1, 2, 3, 4, 5]),
    (0x1ec, &[0, 1]),
    (0x1ee, &[0, 1]),
    (0x140, &[0, 1, 2, 0x56, 0x20]),
    (0x141, &[0, 1, 2, 0x56, 0x20]),
    (0x142, &[0, 1, 2, 0x56, 0x20]),
    (0x148, &[0, 1, 2, 5, 9]),
    (0x149, &[0, 1, 2, 5, 9]),
    (0x150, &[0, 1, 2, 5]),
    (0x151, &[0, 1, 2, 5]),
];

const JOUEUR_MOTS: &[(u32, &[u16])] = &[
    (0x62, &[0, 1, 0x30, 0x70]),
    (0x168, &[0, 1, 0x10, 0x1e, 0x20, 0x21, 0x40, 0x100]),
    (0x0c, &[0, 0x10]),
];

/// Octets de l'objet testés par les gestionnaires.
const OBJET_OCTETS: &[(u32, &[u8])] = &[
    (0x2f, &[0, 1, 2, 3, 4]),
    (0x33, &[0, 1, 0x80]),
    (0x2d, &[0, 4, 8, 0x10, 0x18, 0x1f]),
    (0x53, &[0, 1]),
    (0x57, &[0, 2, 0x20, 0x40]),
    (0x58, &[0, 1, 4, 8, 0x10, 0x20]),
    (0x59, &[0, 1, 2, 3]),
    (0x5a, &[0, 1, 2, 4]),
    (0x5b, &[0, 1, 2]),
    (0x5c, &[0, 1, 2, 4, 8]),
    (0x5d, &[0, 1, 2, 3, 0x10]),
    (0x5e, &[0, 1, 2]),
    (0x5f, &[0, 1, 2, 7]),
];

fn aleatoire(b: &mut cps2::Cps2Bus, alea: &mut Alea, base: u32, octets: &[(u32, &[u8])], mots: &[(u32, &[u16])]) {
    for &(off, valeurs) in octets {
        if alea.dans(2) == 0 {
            wr8(b, base + off, alea.parmi(valeurs));
        }
    }
    for &(off, valeurs) in mots {
        if alea.dans(2) == 0 {
            wr16(b, base + off, alea.parmi(valeurs));
        }
    }
}

/// Fabrique un objet frère actif dans un autre emplacement du même pool.
fn frere(b: &mut cps2::Cps2Bus, alea: &mut Alea, pool: &Pool, objet: u32, kind: u8) -> u32 {
    let mut o = pool.slot(alea.dans(pool.slots)).0;
    if o == objet {
        o = pool.slot((alea.dans(pool.slots - 1) + 1 + (objet - pool.slot(0).0) / pool.stride) % pool.slots).0;
    }
    for off in (0..pool.stride).step_by(2) {
        wr16(b, o + off, 0);
    }
    wr8(b, o, 1);
    wr8(b, o + 2, kind);
    wr8(b, o + 3, alea.dans(2) as u8);
    wr8(b, o + 4, 2);
    wr8(b, o + 0x28, pool.class);
    o
}

/// Nombre de variantes (`+3`) que l'original sait traiter pour un type (les autres sautent dans des données).
fn variantes_valides(classe: u8, kind: u8, defaut: u32) -> u32 {
    match (classe, kind) {
        (0x12, 4) | (0x12, 13) | (0x12, 22) | (0x12, 30) => 3,
        (0x12, 12) => 6,
        (0x12, 0) | (0x12, 1) => 1,
        (0x12, 38) | (0x12, 3) => 2,
        (0x12, 41) | (0x12, 42) => 8,
        (6, 18) => 5,
        (6, 6) => 6,
        (6, 20) => 6,
        (_, _) => defaut.min(2),
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 6 {
        eprintln!("usage : fuzz_joueur <roms> <eeprom> <scénario> <images de chauffe> <tours> [PSHL|PEFF]");
        std::process::exit(2);
    }
    let (roms, eeprom, scenario) = (&a[1], &a[2], &a[3]);
    let chauffe: u32 = a[4].parse().unwrap();
    let tours: u32 = a[5].parse().unwrap();
    let choix = a.get(6).map(String::as_str).unwrap_or("");
    let filtre = std::env::var("DDSOM_ENTREE").ok().and_then(|v| u32::from_str_radix(&v, 16).ok());
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    ddsom_game::install(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    for f in 0..chauffe as usize {
        script.apply(f, &mut m);
        m.run_frame();
    }
    ddsom_game::set_check(true);
    ddsom_game::set_check_all(std::env::var("DDSOM_CHECK_ALL").is_ok());
    std::panic::set_hook(Box::new(|_| {}));
    let graine: u64 = std::env::var("DDSOM_GRAINE").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut alea = Alea(0x1234_5678_9abc_def1 ^ graine.wrapping_mul(0x9e37_79b9_7f4a_7c15));
    let variantes: u32 = std::env::var("DDSOM_VARIANTES").ok().and_then(|v| v.parse().ok()).unwrap_or(3);
    let duree: u32 = std::env::var("DDSOM_PAS").ok().and_then(|v| v.parse().ok()).unwrap_or(40);

    let tables: [(&Pool, u32, u32); 2] = [(&PSHL, 0x39908, 25), (&PEFF, 0x39992, 48)];
    let mut entrees: Vec<(u32, &Pool, u8)> = Vec::new();
    for (pool, table, n) in tables {
        if !choix.is_empty() && pool.name != choix {
            continue;
        }
        for k in 0..n {
            let e = (m.bus.fetch16(table + 4 * k) as u32) << 16 | m.bus.fetch16(table + 4 * k + 2) as u32;
            if ddsom_game::all_natives().any(|nat| nat.entry == e) && filtre.map_or(true, |f| f == e) {
                entrees.push((e, pool, k as u8));
            }
        }
    }
    let (c0, b0) = (m.cpu.clone(), m.bus.clone());
    let (mut essais, mut affiches) = (0u64, 0);
    for _ in 0..tours {
        for &(entree, pool, kind) in &entrees {
            let (mut c, mut b) = (c0.clone(), b0.clone());
            b.irq_horizon = u64::MAX;
            let obj = pool.slot(alea.dans(pool.slots));
            for off in (0..pool.stride).step_by(2) {
                wr16(&mut b, obj.0 + off, 0);
            }
            let joueur = 0xff85de + 0x200 * alea.dans(4);
            wr8(&mut b, joueur, 1);
            wr8(&mut b, obj.0, 1);
            wr8(&mut b, obj.0 + 2, kind);
            let nv = variantes_valides(pool.class, kind, variantes);
            wr8(&mut b, obj.0 + 3, alea.dans(nv) as u8);
            if pool.class == PEFF.class && kind <= 1 && alea.dans(2) == 0 {
                // anneau parent (variante négative)
                wr8(&mut b, obj.0 + 3, 0x80);
            }
            wr8(&mut b, obj.0 + 0x28, pool.class);
            wr8(&mut b, obj.0 + 0x2c, alea.parmi(&[0, 0x20]));
            wr16(&mut b, obj.0 + 0x50, joueur as u16);
            // lanceur et frères factices : liens +0x52, +0x5c, +0x42, +0x46
            let pere = frere(&mut b, &mut alea, pool, obj.0, kind);
            for off in [0x52u32, 0x5c, 0x42, 0x46] {
                wr16(&mut b, obj.0 + off, pere as u16);
            }
            wr16(&mut b, pere + 0x50, joueur as u16);
            let fils = [frere(&mut b, &mut alea, pool, obj.0, kind), frere(&mut b, &mut alea, pool, obj.0, kind)];
            wr16(&mut b, obj.0 + 0x30, fils[0] as u16);
            wr16(&mut b, obj.0 + 0x32, fils[1] as u16);
            if alea.dans(2) == 0 {
                wr16(&mut b, obj.0 + 0x30, pere as u16);
            }
            // états de l'automate
            // trajectoire naturelle (état 0, sous-états nuls) ; DDSOM_ETATS=1 tire aussi des états quelconques
            // (beaucoup tombent hors des tables de l'original : écarts sans signification)
            if std::env::var("DDSOM_ETATS").is_ok() {
                wr8(&mut b, obj.0 + 4, alea.parmi(&[0, 0, 2, 2, 2, 4, 6]));
                wr8(&mut b, obj.0 + 5, alea.parmi(&[0, 0, 2, 4, 6, 8, 0xa, 0xc]));
                wr8(&mut b, obj.0 + 6, alea.parmi(&[0, 0, 2]));
                wr8(&mut b, obj.0 + 7, alea.parmi(&[0, 2, 4]));
            } else {
                wr8(&mut b, obj.0 + 4, 0);
            }
            // position proche de l'écran
            let (cx, cy) = (rd16(&mut b, A5 + 0x506), rd16(&mut b, A5 + 0x50a));
            for (o, j) in [(obj.0, 0u32), (joueur, 0), (pere, 0)] {
                let _ = j;
                wr16(&mut b, o + 8, cx.wrapping_add(0x40 + alea.dans(0x100) as u16));
                wr16(&mut b, o + 0x10, cy.wrapping_add(0x60 + alea.dans(0x40) as u16));
            }
            wr16(&mut b, obj.0 + 0xc, alea.parmi(&[0, 0, 0x10, 0x40, 0xfff0]));
            aleatoire(&mut b, &mut alea, joueur, JOUEUR_OCTETS, JOUEUR_MOTS);
            aleatoire(&mut b, &mut alea, obj.0, OBJET_OCTETS, &[]);
            if pool.class == PSHL.class && kind == 8 {
                wr8(&mut b, obj.0 + 0x2f, alea.parmi(&[0, 2, 4]));
            }
            if pool.class == PEFF.class && kind == 24 {
                // le tas d'or : lien vers un joueur qui lui-même se désigne
                wr16(&mut b, obj.0 + 0x5c, joueur as u16);
                wr16(&mut b, joueur + 0x5c, alea.parmi(&[0, joueur as u16]));
                wr16(&mut b, joueur + 0x50, joueur as u16);
                wr16(&mut b, obj.0 + 0x1a, alea.parmi(&[0, joueur as u16]));
                wr8(&mut b, obj.0 + 0x17, alea.dans(2) as u8);
                let proche = rd16(&mut b, obj.0 + 8).wrapping_add(alea.dans(0x30) as u16);
                wr16(&mut b, joueur + 8, proche);
                for i in 0..6u32 {
                    wr8(&mut b, joueur + 0x140 + i, alea.parmi(&[0, 1, 2, 0x56, 0x10, 0x20]));
                    wr8(&mut b, joueur + 0x148 + i, alea.dans(4) as u8);
                }
            }
            if pool.class == PSHL.class && kind == 19 {
                let cible = ddsom_game::objects::pools::ENEMY.slot(alea.dans(22)).0;
                wr8(&mut b, cible, 1);
                wr16(&mut b, cible + 0x62, 20);
                wr16(&mut b, obj.0 + 0x5c, cible as u16);
            }
            if pool.class == PEFF.class && kind == 39 {
                // le monstre touché (+0x42) est un ennemi
                let ennemi = ddsom_game::objects::pools::ENEMY.slot(alea.dans(22)).0;
                wr8(&mut b, ennemi, 1);
                wr16(&mut b, obj.0 + 0x42, ennemi as u16);
            }
            // génération de la partie : fixe pendant la vie de l'objet (un changement le fait mourir)
            wr8(&mut b, A5 + 0x34, alea.dans(4) as u8);
            if pool.class == PEFF.class {
                // octet de génération : +0x5a (type 0), +0x5b (types 1, 34, 41)
                let g = rd8(&mut b, A5 + 0x34);
                match kind {
                    0 => wr8(&mut b, obj.0 + 0x5a, g),
                    1 | 34 | 41 => wr8(&mut b, obj.0 + 0x5b, g),
                    _ => {}
                }
            }
            for pas in 0..duree {
                if rd8(&mut b, obj.0) == 0 {
                    break;
                }
                if pas > 0 {
                    if alea.dans(3) == 0 {
                        aleatoire(&mut b, &mut alea, joueur, JOUEUR_OCTETS, JOUEUR_MOTS);
                    }
                    if alea.dans(4) == 0 {
                        wr8(&mut b, obj.0 + 0x57, alea.parmi(&[0, 2, 0x20, 0x08]));
                    }
                    if alea.dans(3) == 0 {
                        wr8(&mut b, obj.0 + 0x33, alea.parmi(&[0, 1, 0x80]));
                    }
                    if alea.dans(8) == 0 {
                        // +0x2f (rang) et +0x5c/+0x5d (pointeur de lien) gardent leur valeur
                        let gardes: Vec<u8> = [0x2fu32, 0x5c, 0x5d].iter().map(|&o| rd8(&mut b, obj.0 + o)).collect();
                        aleatoire(&mut b, &mut alea, obj.0, OBJET_OCTETS, &[]);
                        for (&o, v) in [0x2fu32, 0x5c, 0x5d].iter().zip(gardes) {
                            wr8(&mut b, obj.0 + o, v);
                        }
                    }
                }
                for q in ddsom_game::objects::pools::POOLS {
                    let libres = (0..q.slots).filter(|&i| rd8(&mut b, q.slot(i).0) == 0).count();
                    wr16(&mut b, A5 + q.free, libres as u16);
                }
                wr8(&mut b, A5 + 0x3d, (alea.dans(8) == 0) as u8);
                wr8(&mut b, A5 + 0x42, (alea.dans(10) == 0) as u8);
                wr8(&mut b, A5 + 0x36, alea.dans(2) as u8);
                wr8(&mut b, A5 + 0x35, 1 + alea.dans(4) as u8);
                wr8(&mut b, A5 + 0x6f, pas as u8);
                wr16(&mut b, A5 + 0x4ba, alea.parmi(&[0, 0x100]));
                wr32(&mut b, 0xff2044 + 4 * alea.dans(4), alea.dans(3) << 16 | alea.dans(3));
                wr8(&mut b, obj.0 + 0x40, (alea.dans(40) == 0) as u8);
                c.a[0] = obj.0;
                c.a[5] = A5;
                c.d[7] = pas;
                c.d[0] = 4 * kind as u32;
                c.a[1] = entree;
                essais += 1;
                let nom = ddsom_game::all_natives().find(|n| n.entry == entree).map(|n| n.name).unwrap_or("");
                let avant = ddsom_game::report().get(nom).map_or(0, |s| s.mismatches);
                let avant_obj: Vec<u8> = (0..pool.stride).map(|i| rd8(&mut b, obj.0 + i)).collect();
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ddsom_game::call(&mut c, &mut b, entree)));
                if r.is_err() {
                    break;
                }
                let apres = ddsom_game::report().get(nom).map_or(0, |s| s.mismatches);
                if apres > avant && affiches < 40 {
                    affiches += 1;
                    let dernier = ddsom_game::report().get(nom).and_then(|s| s.last.clone());
                    println!("écart en {entree:06x} {} {kind} pas {pas} : {}", pool.name, dernier.unwrap_or_default());
                    println!("  objet avant : {:02x?}", avant_obj);
                }
            }
        }
    }
    let mut total = 0;
    for (name, st) in ddsom_game::report() {
        if name.starts_with("peff_") || name.starts_with("pshl_") {
            total += st.mismatches;
            println!("  {name:12} {:>7} appels {:>5} écarts{}", st.calls, st.mismatches, st.first.map(|f| format!(" — {f}")).unwrap_or_default());
        }
    }
    println!("{essais} appels de gestionnaires");
    println!("{}", if total == 0 { "VALIDE : aucun écart" } else { "ÉCHEC" });
}
