//! Fuzzing des gestionnaires de familles ITEM / BOX / EFFCT sur des états réels perturbés.
//!
//! Le scénario déroule la partie avec le code d'origine seul (la ligne du temps est celle du jeu). Aux
//! appels d'un gestionnaire réécrit (un sur `période`), on prend l'état réel, on en tire `essais`
//! copies dont quelques octets sont perturbés (champs de l'objet, de son joueur, drapeaux de scène),
//! puis on exécute sur chacune le code d'origine et la routine native, et on compare (RAM, registres,
//! cycles). Cela exerce des branches que les scénarios n'atteignent pas. Les états absurdes sont
//! sans danger : les parties non réécrites s'exécutent en code d'origine dans les deux versions.
//!
//! Les types que les scénarios n'exercent jamais se testent en les *forçant* : le dernier argument
//! (`ITEM:2,ITEM:3,EFFCT:7`) liste des types dont la routine native est essayée sur des copies d'objets
//! réels de la même famille dont l'octet de type (`+2`) est remplacé, avec variante et états tirés au
//! hasard.
//!
//!   fuzz_familles <roms> <eeprom> <scénario> <images> [période [essais [graine [FAMILLE:type,...]]]]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

use cps2::Cps2Bus;
use m68k::Bus;
use m68k::Cpu;

use ddsom_game::mem::{rd16, rd32, rd8, wr16, wr8};

#[derive(Default)]
struct Bilan {
    essais: u64,
    ecarts: u64,
    paniques: u64,
    abandons: u64,
    exemples: Vec<String>,
}

/// (nom de la famille, table de sauts, nombre de types)
const FAMILLES: &[(&str, u32, u32)] = &[("ITEM", 0x39f5a, 10), ("BOX", 0x39e76, 4), ("EFFCT", 0x39ce0, 92)];

thread_local! {
    /// adresse d'entrée d'un gestionnaire -> famille
    static ENTREES: RefCell<BTreeMap<u32, usize>> = RefCell::new(BTreeMap::new());
    /// types forcés : (famille, type, adresse d'entrée)
    static FORCES: RefCell<Vec<(usize, u32, u32)>> = const { RefCell::new(Vec::new()) };
    static APPELS_FAMILLE: RefCell<BTreeMap<usize, u64>> = RefCell::new(BTreeMap::new());
    static BILAN: RefCell<BTreeMap<&'static str, Bilan>> = RefCell::new(BTreeMap::new());
    static APPELS: RefCell<BTreeMap<u32, u64>> = RefCell::new(BTreeMap::new());
    static ALEA: RefCell<u64> = const { RefCell::new(0x9e37_79b9_7f4a_7c15) };
    static PARAMETRES: RefCell<(u64, u32)> = const { RefCell::new((16, 8)) };
}

fn alea() -> u64 {
    ALEA.with(|a| {
        let mut x = *a.borrow();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *a.borrow_mut() = x;
        x
    })
}

fn choisit<T: Copy>(v: &[T]) -> T {
    v[(alea() % v.len() as u64) as usize]
}

/// Octets de l'objet que les gestionnaires testent (décalage, valeurs plausibles).
const OBJET_OCTETS: &[(u32, &[u8])] = &[
    (0x01, &[0, 1, 2, 3]),
    (0x03, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0x0a, 0x10, 0x11]),
    (0x04, &[0, 2, 4, 6]),
    (0x05, &[0, 2, 4, 6, 8, 0x0a]),
    (0x06, &[0, 2, 4]),
    (0x14, &[0, 1]),
    (0x15, &[0, 1, 2, 0x0c]),
    (0x16, &[0, 1]),
    (0x22, &[0, 1]),
    (0x2c, &[0, 0x20]),
    (0x2d, &[0, 1, 2]),
    (0x2f, &[0, 1, 2, 3, 4]),
    (0x33, &[0, 1]),
    (0x39, &[0, 1, 2, 3, 4, 5, 6, 7]),
    (0x40, &[0, 1]),
    (0x43, &[0, 1]),
    (0x53, &[0, 1, 0x80]),
    (0x57, &[0, 1, 2, 3]),
    (0x58, &[0, 1, 2]),
    (0x59, &[0, 1, 2]),
    (0x5a, &[0, 1, 2, 9]),
    (0x5b, &[0, 1]),
    (0x60, &[0, 1, 2, 3, 4]),
    (0x61, &[0, 1]),
    (0x62, &[0, 1, 2, 3]),
    (0x6b, &[0, 1, 2, 3]),
    (0x6e, &[0, 1]),
    (0x6f, &[0, 1, 2, 8]),
    (0x70, &[0, 1]),
    (0x71, &[0, 1]),
    (0x76, &[0, 1]),
    (0x77, &[0, 1]),
    (0x7c, &[0, 1]),
    (0x7d, &[0, 1]),
    (0x7e, &[0, 1]),
    (0x7f, &[0, 1]),
    (0x30, &[0, 1, 2]),
    (0x31, &[0, 1]),
];

/// Mots de l'objet (décalage, valeurs plausibles).
const OBJET_MOTS: &[(u32, &[u16])] = &[
    (0x0c, &[0, 0x10, 0xfff0]),
    (0x0e, &[0, 0x8000]),
    (0x42, &[0, 1, 2, 0x3c, 0x3d, 0x100, 0x4b0, 0xffff]),
    (0x50, &[0x85de, 0x87de, 0x89de, 0x8bde]),
    (0x54, &[0x85de, 0x87de, 0x89de, 0x8bde]),
    (0x46, &[0x85de, 0x87de]),
    (0x1e, &[0x85de, 0x87de]),
    (0x56, &[0, 1, 2, 0x3c]),
    (0x58, &[0, 1, 2, 0x30]),
    (0x5c, &[0, 1, 2, 0x85de, 0x87de]),
    (0x14, &[0, 0x100, 0xff00, 0x380]),
    (0x18, &[0, 0x300, 0xfd00]),
    (0x1a, &[0, 0xffd8, 0xffc0]),
    (0x1c, &[0, 0x40]),
    (0x30, &[1, 2, 3]),
    (0x32, &[0, 0xfe00]),
    (0x8, &[0, 0x40, 0x100, 0x200, 0x300]),
    (0x10, &[0, 0x40, 0x80, 0xc0]),
];

/// Octets du joueur propriétaire.
const JOUEUR_OCTETS: &[(u32, &[u8])] = &[
    (0x00, &[0, 1, 1, 1, 2]),
    (0x02, &[0, 1, 2, 3, 4, 5]),
    (0x38, &[0, 1, 2, 3, 4, 5]),
    (0x182, &[0, 1]),
    (0x03, &[0, 2, 4, 6, 8, 0x0a, 0x0b]),
    (0x04, &[0, 2, 4, 6]),
    (0x22, &[0, 1]),
    (0x23, &[0, 1, 2]),
    (0x2c, &[0, 0x20]),
    (0x33, &[0, 1]),
    (0x7d, &[0, 1, 2, 3]),
    (0x140, &[0x56, 0x23, 0x20, 0x29, 1, 2, 3, 4, 5]),
    (0x141, &[0x56, 0x23, 0x29]),
    (0x142, &[0x56, 0x2c]),
    (0x143, &[0x56, 0x30]),
    (0x144, &[0x56]),
    (0x145, &[0x56]),
    (0x148, &[0, 1, 5, 8, 9]),
    (0x149, &[0, 1, 9]),
    (0x14a, &[0, 9]),
    (0x180, &[0, 1]),
];

/// Variables de scène (décalage depuis A5, valeurs plausibles).
const SCENE_OCTETS: &[(i32, &[u8])] = &[
    (0x2c, &[0, 1, 2, 3, 4, 7, 8, 9]),
    (0x36, &[0, 1]),
    (0x3d, &[0, 1]),
    (0x42, &[0, 1]),
    (0x44, &[0, 1]),
    (0x45, &[0, 1, 8, 9]),
    (0x4f, &[0, 1]),
    (0x6f, &[0, 1, 2, 3]),
    (-0x5ac4, &[0, 1]),
    (-0x5ac6, &[0, 1, 2, 3]),
    (-0x55c8, &[0, 4, 0x0c, 0x10]),
];

/// Perturbe l'état copié ; renvoie la description des changements.
fn perturbe(b: &mut Cps2Bus, obj: u32) -> String {
    let mut notes = Vec::new();
    let n = 1 + alea() % 4;
    let proprietaire = {
        let p = rd16(b, obj + 0x50);
        0xff_0000 | p as u32
    };
    for _ in 0..n {
        match alea() % 10 {
            0..=4 => {
                let (off, vals) = choisit(OBJET_OCTETS);
                let v = choisit(vals);
                wr8(b, obj + off, v);
                notes.push(format!("obj+{off:02x}={v:02x}"));
            }
            5 | 6 => {
                let (off, vals) = choisit(OBJET_MOTS);
                let v = choisit(vals);
                wr16(b, obj + off, v);
                notes.push(format!("obj+{off:02x}.w={v:04x}"));
            }
            7 | 8 => {
                if (0xff_8000..0x100_0000).contains(&proprietaire) {
                    let (off, vals) = choisit(JOUEUR_OCTETS);
                    let v = choisit(vals);
                    wr8(b, proprietaire + off, v);
                    notes.push(format!("joueur+{off:02x}={v:02x}"));
                }
            }
            9 if alea() % 2 == 0 => {
                // l'objet se place sur un joueur (recouvrement de boîtes, proximité...)
                let joueur = 0xff_85de + 0x200 * (alea() % 4) as u32;
                let delta = choisit(&[0u16, 0, 4, 0xfffc, 0x10, 0xfff0, 0x40, 0xffc0]);
                for off in [0x08u32, 0x0c, 0x10] {
                    let v = rd16(b, joueur + off).wrapping_add(if alea() % 3 == 0 { delta } else { 0 });
                    wr16(b, obj + off, v);
                }
                notes.push(format!("pos=joueur@{joueur:06x}{delta:+#x}"));
            }
            _ => {
                let (off, vals) = choisit(SCENE_OCTETS);
                let v = choisit(vals);
                wr8(b, 0xff_8000u32.wrapping_add(off as u32), v);
                notes.push(format!("A5{off:+#x}={v:02x}"));
            }
        }
    }
    notes.join(" ")
}

/// Exécute le code d'origine jusqu'au retour ; faux si la routine ne revient pas assez vite.
fn execute_origine(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    let ret = rd32(b, c.a[7]);
    let sp_apres = c.a[7].wrapping_add(4);
    let mut garde = 0;
    while !(c.pc == ret && c.a[7] == sp_apres) {
        if ddsom_rc::run_block(c.pc, c, b, u64::MAX).is_none() {
            c.step(b);
        }
        garde += 1;
        if garde > 200_000 || c.cycles > 5_000_000_000 {
            return false;
        }
    }
    true
}

fn essai(n: &'static ddsom_game::Native, c: &Cpu, b: &Cps2Bus, perturbation: bool, force: Option<u32>) {
    let mut c1 = c.clone();
    let mut b1 = b.clone();
    let mut notes = String::new();
    if let Some(t) = force {
        // objet synthétique : le même objet réel, d'un autre type
        let obj = c.a[0];
        wr8(&mut b1, obj + 2, t as u8);
        wr8(&mut b1, obj + 3, choisit(&[0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0x0a, 0x0c, 0x10, 0x11, 0x14]));
        wr8(&mut b1, obj + 4, choisit(&[0, 0, 2, 2, 4, 6]));
        wr8(&mut b1, obj + 5, choisit(&[0, 0, 2, 4, 6, 8, 0x0a]));
        wr8(&mut b1, obj + 6, choisit(&[0, 0, 2, 4]));
        c1.pc = n.entry;
        wr16(&mut b1, obj + 0x50, choisit(&[0x85de, 0x87de, 0x89de, 0x8bde]));
        wr16(&mut b1, obj + 0x54, choisit(&[0x85de, 0x87de, 0x89de, 0x8bde, obj as u16]));
        wr16(&mut b1, obj + 0x5c, choisit(&[0x85de, 0x87de, obj as u16]));
        notes = format!(
            "type {t} variante {:02x} états {:02x} {:02x} {:02x} ; ",
            rd8(&mut b1, obj + 3),
            rd8(&mut b1, obj + 4),
            rd8(&mut b1, obj + 5),
            rd8(&mut b1, obj + 6)
        );
    }
    if perturbation {
        notes += &perturbe(&mut b1, c.a[0]);
    }
    let (mut c2, mut b2) = (c1.clone(), b1.clone());
    let (c_depart, b_depart) =
        if std::env::var_os("FUZZ_TRACE").is_some() { (Some(c1.clone()), Some(b1.clone())) } else { (None, None) };
    let obj = c.a[0];
    let avant: Vec<u8> = (0..0x60).map(|i| rd8(&mut b2, obj + i)).collect();
    if !execute_origine(&mut c2, &mut b2) {
        BILAN.with(|m| m.borrow_mut().entry(n.name).or_default().abandons += 1);
        return;
    }
    // routine native sur la copie perturbée (même état de départ que l'original)
    let resultat = catch_unwind(AssertUnwindSafe(|| {
        let cout = (n.cost)(&c1, &mut b1);
        if cout < ddsom_game::families::COUT_HORS_CONTRAT {
            (n.run)(&mut c1, &mut b1);
            c1.cycles += cout;
        } else {
            // chemin hors contrat : l'original s'exécute seul, rien à comparer
            c1 = c2.clone();
            b1 = b2.clone();
        }
    }));
    let mut diffs = Vec::new();
    match resultat {
        Err(e) => {
            if let (true, Some(cd), Some(mut bd)) = (std::env::var_os("FUZZ_TRACE").is_some(), c_depart.clone(), b_depart.clone()) {
                ddsom_game::families::activer_diagnostic(true);
                let _ = catch_unwind(AssertUnwindSafe(|| (n.cost)(&cd, &mut bd)));
                ddsom_game::families::activer_diagnostic(false);
            }
            let msg = e
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_default();
            BILAN.with(|m| {
                let mut m = m.borrow_mut();
                let bilan = m.entry(n.name).or_default();
                bilan.essais += 1;
                bilan.paniques += 1;
                if bilan.exemples.len() < 3 {
                    bilan.exemples.push(format!("PANIQUE {msg} ; {notes} ; objet {obj:06x} {}", dump(&avant)));
                }
            });
            return;
        }
        Ok(()) => {}
    }
    if c1.cycles != c2.cycles {
        diffs.push(format!("cycles {} au lieu de {}", c1.cycles - c.cycles, c2.cycles - c.cycles));
    }
    if c1.d[7] != c2.d[7] {
        diffs.push(format!("d7={:08x} au lieu de {:08x}", c1.d[7], c2.d[7]));
    }
    if c1.a[0] != c2.a[0] {
        diffs.push(format!("a0={:08x} au lieu de {:08x}", c1.a[0], c2.a[0]));
    }
    let sp = c2.a[7] & 0xff_ffff;
    ddsom_game::mem::diff_bus(&b1, &b2, sp.saturating_sub(0x400)..sp, &mut diffs);
    BILAN.with(|m| {
        let mut m = m.borrow_mut();
        let bilan = m.entry(n.name).or_default();
        bilan.essais += 1;
        if !diffs.is_empty() {
            bilan.ecarts += 1;
            if bilan.exemples.len() < 3 {
                if let (Some(mut ct), Some(mut bt)) = (c_depart, b_depart) {
                    // trace de l'original (pc et durée de chaque instruction) pour le diagnostic
                    let ret = rd32(&mut bt, ct.a[7]);
                    let sp_apres = ct.a[7].wrapping_add(4);
                    let mut trace = String::new();
                    let mut n = 0;
                    while !(ct.pc == ret && ct.a[7] == sp_apres) && n < 3000 {
                        let (pc, avant) = (ct.pc, ct.cycles);
                        ct.step(&mut bt);
                        trace += &format!(" {pc:06x}+{}", ct.cycles - avant);
                        n += 1;
                    }
                    bilan.exemples.push(format!("TRACE{trace}"));
                }
                bilan.exemples.push(format!("{} ; {notes} ; objet {obj:06x} {}", diffs.join(", "), dump(&avant)));
            }
        }
    });
}

fn dump(octets: &[u8]) -> String {
    octets.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join("")
}

fn run(pc: u32, c: &mut Cpu, b: &mut Cps2Bus, dl: u64) -> Option<u32> {
    // types forcés : essais sur des objets synthétiques à chaque appel (périodique) d'un gestionnaire
    // de la même famille
    if let Some(famille) = ENTREES.with(|e| e.borrow().get(&pc).copied()) {
        let (periode, essais) = PARAMETRES.with(|p| *p.borrow());
        let k = APPELS_FAMILLE.with(|m| {
            let mut m = m.borrow_mut();
            let e = m.entry(famille).or_insert(0);
            *e += 1;
            *e
        });
        if k % periode == 1 {
            let forces: Vec<_> = FORCES.with(|f| f.borrow().iter().filter(|x| x.0 == famille).copied().collect());
            for (_, type_force, entree) in forces {
                if let Some(n) = ddsom_game::families::NATIVES.iter().find(|n| n.entry == entree) {
                    for _ in 0..essais * 2 {
                        essai(n, c, b, true, Some(type_force));
                    }
                }
            }
        }
    }
    if let Some(n) = ddsom_game::families::NATIVES.iter().find(|n| n.entry == pc) {
        let (periode, essais) = PARAMETRES.with(|p| *p.borrow());
        let k = APPELS.with(|m| {
            let mut m = m.borrow_mut();
            let e = m.entry(pc).or_insert(0);
            *e += 1;
            *e
        });
        if k % periode == 1 {
            // l'état réel d'abord (sans perturbation), puis des variantes perturbées
            essai(n, c, b, false, None);
            for _ in 0..essais {
                essai(n, c, b, true, None);
            }
        }
    }
    ddsom_rc::run_block(pc, c, b, dl)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (Some(roms), Some(eeprom), Some(scenario), Some(frames)) = (a.get(1), a.get(2), a.get(3), a.get(4)) else {
        eprintln!("usage : fuzz_familles <roms> <eeprom> <scénario> <images> [période [essais [graine]]]");
        std::process::exit(2);
    };
    let periode: u64 = a.get(5).map_or(16, |s| s.parse().unwrap());
    let essais: u32 = a.get(6).map_or(8, |s| s.parse().unwrap());
    if let Some(g) = a.get(7) {
        ALEA.with(|x| *x.borrow_mut() = 0x9e37_79b9_7f4a_7c15 ^ g.parse::<u64>().unwrap().wrapping_mul(0x1000_0000_01b3));
    }
    PARAMETRES.with(|p| *p.borrow_mut() = (periode, essais));
    std::panic::set_hook(Box::new(|_| {})); // les paniques sont comptées, pas affichées
    ddsom_game::families::activer_couverture();
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    let voulus = a.get(8).cloned().unwrap_or_default();
    for (i, &(nom, table, n)) in FAMILLES.iter().enumerate() {
        for t in 0..n {
            let adresse = table + 4 * t;
            let entree = (m.bus.read_prog16(adresse) as u32) << 16 | m.bus.read_prog16(adresse + 2) as u32;
            ENTREES.with(|e| e.borrow_mut().insert(entree, i));
            if voulus.split(',').any(|x| x == format!("{nom}:{t}")) {
                FORCES.with(|f| f.borrow_mut().push((i, t, entree)));
            }
        }
    }
    m.executor = Some(run);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    for f in 0..frames.parse().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    for (fichier, ligne, pris) in ddsom_game::families::couverture() {
        println!("COUV {} {ligne} {pris}", fichier.rsplit('/').next().unwrap());
    }
    let mut total_ecarts = 0;
    for (nom, bilan) in BILAN.with(|m| std::mem::take(&mut *m.borrow_mut())) {
        println!(
            "  {nom:24} {:>7} essais {:>5} écarts {:>4} paniques {:>5} abandons",
            bilan.essais, bilan.ecarts, bilan.paniques, bilan.abandons
        );
        total_ecarts += bilan.ecarts + bilan.paniques;
        for e in bilan.exemples {
            println!("    {e}");
        }
    }
    println!("{}", if total_ecarts == 0 { "FUZZ OK" } else { "FUZZ ÉCARTS" });
    let _ = rd16;
}
