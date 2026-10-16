//! Fuzzing des gestionnaires de familles ITEM / BOX / EFFCT sur des états réels perturbés.
//!
//! Le scénario déroule la partie avec le code d'origine seul (la ligne du temps est celle du jeu). Aux
//! appels d'un gestionnaire réécrit (un sur `période`), on prend l'état réel, on en tire `essais`
//! copies dont quelques octets sont perturbés (champs de l'objet, de son joueur, drapeaux de scène),
//! puis on exécute sur chacune le code d'origine et la routine native, et on compare (RAM, registres,
//! cycles). Cela exerce des branches que les scénarios n'atteignent pas. Les états absurdes sont
//! sans danger : les parties non réécrites s'exécutent en code d'origine dans les deux versions.
//!
//!   fuzz_familles <roms> <eeprom> <scénario> <images> [période [essais [graine]]]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

use cps2::Cps2Bus;
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

thread_local! {
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
    (0x56, &[0, 1, 2, 0x3c]),
    (0x58, &[0, 1, 2, 0x30]),
    (0x5c, &[0, 1, 2]),
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
    (0x02, &[0, 1, 2, 3, 4, 5]),
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

fn essai(n: &'static ddsom_game::Native, c: &Cpu, b: &Cps2Bus, perturbation: bool) {
    let mut c1 = c.clone();
    let mut b1 = b.clone();
    let notes = if perturbation { perturbe(&mut b1, c.a[0]) } else { String::new() };
    let (mut c2, mut b2) = (c1.clone(), b1.clone());
    let obj = c.a[0];
    let avant: Vec<u8> = (0..0x60).map(|i| rd8(&mut b2, obj + i)).collect();
    if !execute_origine(&mut c2, &mut b2) {
        BILAN.with(|m| m.borrow_mut().entry(n.name).or_default().abandons += 1);
        return;
    }
    // routine native sur la copie perturbée (même état de départ que l'original)
    let resultat = catch_unwind(AssertUnwindSafe(|| {
        let cout = (n.cost)(&c1, &mut b1);
        (n.run)(&mut c1, &mut b1);
        c1.cycles += cout;
    }));
    let mut diffs = Vec::new();
    match resultat {
        Err(e) => {
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
                bilan.exemples.push(format!("{} ; {notes} ; objet {obj:06x} {}", diffs.join(", "), dump(&avant)));
            }
        }
    });
}

fn dump(octets: &[u8]) -> String {
    octets.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join("")
}

fn run(pc: u32, c: &mut Cpu, b: &mut Cps2Bus, dl: u64) -> Option<u32> {
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
            essai(n, c, b, false);
            for _ in 0..essais {
                essai(n, c, b, true);
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
