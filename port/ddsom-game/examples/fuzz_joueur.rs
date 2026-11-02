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
//! Variables : `DDSOM_ENTREE=<address>` (un seul gestionnaire), `DDSOM_GRAINE`, `DDSOM_PAS` (images par
//! objet, 40 par défaut), `DDSOM_VARIANTES` (variantes tirées, 3 par défaut), `DDSOM_VERBOSE`.

use std::path::Path;

use ddsom_game::mem::{rd16, rd8, wr16, wr32, wr8, A5};
use ddsom_game::objects::pools::{Pool, PEFF, PSHL};
use m68k::decode::Fetch;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn among<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.below(v.len() as u32) as usize]
    }
}

/// Octets du joueur propriétaire que les gestionnaires testent : (décalage, valeurs plausibles).
const PLAYER_BYTES: &[(u32, &[u8])] = &[
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

const PLAYER_WORDS: &[(u32, &[u16])] = &[
    (0x62, &[0, 1, 0x30, 0x70]),
    (0x168, &[0, 1, 0x10, 0x1e, 0x20, 0x21, 0x40, 0x100]),
    (0x0c, &[0, 0x10]),
];

/// Octets de l'objet testés par les gestionnaires.
const OBJECT_BYTES: &[(u32, &[u8])] = &[
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

fn random(b: &mut cps2::Cps2Bus, rng: &mut Rng, base: u32, bytes: &[(u32, &[u8])], words: &[(u32, &[u16])]) {
    for &(off, values) in bytes {
        if rng.below(2) == 0 {
            wr8(b, base + off, rng.among(values));
        }
    }
    for &(off, values) in words {
        if rng.below(2) == 0 {
            wr16(b, base + off, rng.among(values));
        }
    }
}

/// Fabrique un objet frère actif dans un autre emplacement du même pool.
fn sibling(b: &mut cps2::Cps2Bus, rng: &mut Rng, pool: &Pool, objet: u32, kind: u8) -> u32 {
    let mut o = pool.slot(rng.below(pool.slots)).0;
    if o == objet {
        o = pool.slot((rng.below(pool.slots - 1) + 1 + (objet - pool.slot(0).0) / pool.stride) % pool.slots).0;
    }
    for off in (0..pool.stride).step_by(2) {
        wr16(b, o + off, 0);
    }
    wr8(b, o, 1);
    wr8(b, o + 2, kind);
    wr8(b, o + 3, rng.below(2) as u8);
    wr8(b, o + 4, 2);
    wr8(b, o + 0x28, pool.class);
    o
}

/// Nombre de variantes (`+3`) que l'original sait traiter pour un type (les autres sautent dans des données).
fn valid_variants(class: u8, kind: u8, default: u32) -> u32 {
    match (class, kind) {
        (0x12, 4) | (0x12, 13) | (0x12, 22) | (0x12, 30) => 3,
        (0x12, 12) => 6,
        (0x12, 0) | (0x12, 1) => 1,
        (0x12, 38) | (0x12, 3) => 2,
        (0x12, 41) | (0x12, 42) => 8,
        (6, 18) => 5,
        (6, 6) => 6,
        (6, 20) => 6,
        (_, _) => default.min(2),
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 6 {
        eprintln!("usage : fuzz_joueur <roms> <eeprom> <scénario> <images de chauffe> <tours> [PSHL|PEFF]");
        std::process::exit(2);
    }
    let (roms, eeprom, scenario) = (&a[1], &a[2], &a[3]);
    let warmup: u32 = a[4].parse().unwrap();
    let tours: u32 = a[5].parse().unwrap();
    let choice = a.get(6).map(String::as_str).unwrap_or("");
    let filter = std::env::var("DDSOM_ENTREE").ok().and_then(|v| u32::from_str_radix(&v, 16).ok());
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    ddsom_game::install(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    for f in 0..warmup as usize {
        script.apply(f, &mut m);
        m.run_frame();
    }
    ddsom_game::set_check(true);
    ddsom_game::set_check_all(std::env::var("DDSOM_CHECK_ALL").is_ok());
    std::panic::set_hook(Box::new(|_| {}));
    let seed: u64 = std::env::var("DDSOM_GRAINE").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut rng = Rng(0x1234_5678_9abc_def1 ^ seed.wrapping_mul(0x9e37_79b9_7f4a_7c15));
    let variants: u32 = std::env::var("DDSOM_VARIANTES").ok().and_then(|v| v.parse().ok()).unwrap_or(3);
    let duration: u32 = std::env::var("DDSOM_PAS").ok().and_then(|v| v.parse().ok()).unwrap_or(40);

    let tables: [(&Pool, u32, u32); 2] = [(&PSHL, 0x39908, 25), (&PEFF, 0x39992, 48)];
    let mut inputs: Vec<(u32, &Pool, u8)> = Vec::new();
    for (pool, table, n) in tables {
        if !choice.is_empty() && pool.name != choice {
            continue;
        }
        for k in 0..n {
            let e = (m.bus.fetch16(table + 4 * k) as u32) << 16 | m.bus.fetch16(table + 4 * k + 2) as u32;
            if ddsom_game::all_natives().any(|nat| nat.entry == e) && filter.map_or(true, |f| f == e) {
                inputs.push((e, pool, k as u8));
            }
        }
    }
    let (c0, b0) = (m.cpu.clone(), m.bus.clone());
    let (mut tests, mut shown) = (0u64, 0);
    for _ in 0..tours {
        for &(entry, pool, kind) in &inputs {
            let (mut c, mut b) = (c0.clone(), b0.clone());
            b.irq_horizon = u64::MAX;
            let obj = pool.slot(rng.below(pool.slots));
            for off in (0..pool.stride).step_by(2) {
                wr16(&mut b, obj.0 + off, 0);
            }
            let player = 0xff85de + 0x200 * rng.below(4);
            wr8(&mut b, player, 1);
            wr8(&mut b, obj.0, 1);
            wr8(&mut b, obj.0 + 2, kind);
            let nv = valid_variants(pool.class, kind, variants);
            wr8(&mut b, obj.0 + 3, rng.below(nv) as u8);
            if pool.class == PEFF.class && kind <= 1 && rng.below(2) == 0 {
                // anneau parent (variante négative)
                wr8(&mut b, obj.0 + 3, 0x80);
            }
            wr8(&mut b, obj.0 + 0x28, pool.class);
            wr8(&mut b, obj.0 + 0x2c, rng.among(&[0, 0x20]));
            wr16(&mut b, obj.0 + 0x50, player as u16);
            // lanceur et frères factices : liens +0x52, +0x5c, +0x42, +0x46
            let parent = sibling(&mut b, &mut rng, pool, obj.0, kind);
            for off in [0x52u32, 0x5c, 0x42, 0x46] {
                wr16(&mut b, obj.0 + off, parent as u16);
            }
            wr16(&mut b, parent + 0x50, player as u16);
            let child = [sibling(&mut b, &mut rng, pool, obj.0, kind), sibling(&mut b, &mut rng, pool, obj.0, kind)];
            wr16(&mut b, obj.0 + 0x30, child[0] as u16);
            wr16(&mut b, obj.0 + 0x32, child[1] as u16);
            if rng.below(2) == 0 {
                wr16(&mut b, obj.0 + 0x30, parent as u16);
            }
            // états de l'automate
            // trajectoire naturelle (état 0, sous-états nuls) ; DDSOM_ETATS=1 tire aussi des états quelconques
            // (beaucoup tombent hors des tables de l'original : écarts sans signification)
            if std::env::var("DDSOM_ETATS").is_ok() {
                wr8(&mut b, obj.0 + 4, rng.among(&[0, 0, 2, 2, 2, 4, 6]));
                wr8(&mut b, obj.0 + 5, rng.among(&[0, 0, 2, 4, 6, 8, 0xa, 0xc]));
                wr8(&mut b, obj.0 + 6, rng.among(&[0, 0, 2]));
                wr8(&mut b, obj.0 + 7, rng.among(&[0, 2, 4]));
            } else {
                wr8(&mut b, obj.0 + 4, 0);
            }
            // position proche de l'écran
            let (cx, cy) = (rd16(&mut b, A5 + 0x506), rd16(&mut b, A5 + 0x50a));
            for (o, j) in [(obj.0, 0u32), (player, 0), (parent, 0)] {
                let _ = j;
                wr16(&mut b, o + 8, cx.wrapping_add(0x40 + rng.below(0x100) as u16));
                wr16(&mut b, o + 0x10, cy.wrapping_add(0x60 + rng.below(0x40) as u16));
            }
            wr16(&mut b, obj.0 + 0xc, rng.among(&[0, 0, 0x10, 0x40, 0xfff0]));
            random(&mut b, &mut rng, player, PLAYER_BYTES, PLAYER_WORDS);
            random(&mut b, &mut rng, obj.0, OBJECT_BYTES, &[]);
            if pool.class == PSHL.class && kind == 8 {
                wr8(&mut b, obj.0 + 0x2f, rng.among(&[0, 2, 4]));
            }
            if pool.class == PEFF.class && kind == 24 {
                // le tas d'or : lien vers un joueur qui lui-même se désigne
                wr16(&mut b, obj.0 + 0x5c, player as u16);
                wr16(&mut b, player + 0x5c, rng.among(&[0, player as u16]));
                wr16(&mut b, player + 0x50, player as u16);
                wr16(&mut b, obj.0 + 0x1a, rng.among(&[0, player as u16]));
                wr8(&mut b, obj.0 + 0x17, rng.below(2) as u8);
                let near = rd16(&mut b, obj.0 + 8).wrapping_add(rng.below(0x30) as u16);
                wr16(&mut b, player + 8, near);
                for i in 0..6u32 {
                    wr8(&mut b, player + 0x140 + i, rng.among(&[0, 1, 2, 0x56, 0x10, 0x20]));
                    wr8(&mut b, player + 0x148 + i, rng.below(4) as u8);
                }
            }
            if pool.class == PSHL.class && kind == 19 {
                let target = ddsom_game::objects::pools::ENEMY.slot(rng.below(22)).0;
                wr8(&mut b, target, 1);
                wr16(&mut b, target + 0x62, 20);
                wr16(&mut b, obj.0 + 0x5c, target as u16);
            }
            if pool.class == PEFF.class && kind == 39 {
                // le monstre touché (+0x42) est un ennemi
                let enemy = ddsom_game::objects::pools::ENEMY.slot(rng.below(22)).0;
                wr8(&mut b, enemy, 1);
                wr16(&mut b, obj.0 + 0x42, enemy as u16);
            }
            // génération de la partie : fixe pendant la vie de l'objet (un changement le fait mourir)
            wr8(&mut b, A5 + 0x34, rng.below(4) as u8);
            if pool.class == PEFF.class {
                // octet de génération : +0x5a (type 0), +0x5b (types 1, 34, 41)
                let g = rd8(&mut b, A5 + 0x34);
                match kind {
                    0 => wr8(&mut b, obj.0 + 0x5a, g),
                    1 | 34 | 41 => wr8(&mut b, obj.0 + 0x5b, g),
                    _ => {}
                }
            }
            for step in 0..duration {
                if rd8(&mut b, obj.0) == 0 {
                    break;
                }
                if step > 0 {
                    if rng.below(3) == 0 {
                        random(&mut b, &mut rng, player, PLAYER_BYTES, PLAYER_WORDS);
                    }
                    if rng.below(4) == 0 {
                        wr8(&mut b, obj.0 + 0x57, rng.among(&[0, 2, 0x20, 0x08]));
                    }
                    if rng.below(3) == 0 {
                        wr8(&mut b, obj.0 + 0x33, rng.among(&[0, 1, 0x80]));
                    }
                    if rng.below(8) == 0 {
                        // +0x2f (rang) et +0x5c/+0x5d (pointeur de lien) gardent leur valeur
                        let guards: Vec<u8> = [0x2fu32, 0x5c, 0x5d].iter().map(|&o| rd8(&mut b, obj.0 + o)).collect();
                        random(&mut b, &mut rng, obj.0, OBJECT_BYTES, &[]);
                        for (&o, v) in [0x2fu32, 0x5c, 0x5d].iter().zip(guards) {
                            wr8(&mut b, obj.0 + o, v);
                        }
                    }
                }
                for q in ddsom_game::objects::pools::POOLS {
                    let free = (0..q.slots).filter(|&i| rd8(&mut b, q.slot(i).0) == 0).count();
                    wr16(&mut b, A5 + q.free, free as u16);
                }
                wr8(&mut b, A5 + 0x3d, (rng.below(8) == 0) as u8);
                wr8(&mut b, A5 + 0x42, (rng.below(10) == 0) as u8);
                wr8(&mut b, A5 + 0x36, rng.below(2) as u8);
                wr8(&mut b, A5 + 0x35, 1 + rng.below(4) as u8);
                wr8(&mut b, A5 + 0x6f, step as u8);
                wr16(&mut b, A5 + 0x4ba, rng.among(&[0, 0x100]));
                wr32(&mut b, 0xff2044 + 4 * rng.below(4), rng.below(3) << 16 | rng.below(3));
                wr8(&mut b, obj.0 + 0x40, (rng.below(40) == 0) as u8);
                c.a[0] = obj.0;
                c.a[5] = A5;
                c.d[7] = step;
                c.d[0] = 4 * kind as u32;
                c.a[1] = entry;
                tests += 1;
                let native_name = ddsom_game::all_natives().find(|n| n.entry == entry).map(|n| n.name).unwrap_or("");
                let before = ddsom_game::report().get(native_name).map_or(0, |s| s.mismatches);
                let prev_obj: Vec<u8> = (0..pool.stride).map(|i| rd8(&mut b, obj.0 + i)).collect();
                let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ddsom_game::call(&mut c, &mut b, entry)));
                if r.is_err() {
                    break;
                }
                let after = ddsom_game::report().get(native_name).map_or(0, |s| s.mismatches);
                if after > before && shown < 40 {
                    shown += 1;
                    let last = ddsom_game::report().get(native_name).and_then(|s| s.last.clone());
                    println!("écart en {entry:06x} {} {kind} pas {step} : {}", pool.name, last.unwrap_or_default());
                    println!("  objet avant : {:02x?}", prev_obj);
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
    println!("{tests} appels de gestionnaires");
    println!("{}", if total == 0 { "VALIDE : aucun écart" } else { "ÉCHEC" });
}
