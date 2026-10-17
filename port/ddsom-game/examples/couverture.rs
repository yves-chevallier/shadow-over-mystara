//! Couverture des routines natives : combien de fois chaque point d'entrée natif est atteint sur un
//! scénario (un « adresse nom appels » par ligne, y compris les appels à zéro).
//!   couverture <roms> <eeprom> <scénario> <images> [index.tsv]
//! Avec l'index des fonctions (decomp/index.tsv) : code d'origine seul, appels de chaque fonction.
//! Sert à signaler les routines non exercées (donc non validées) par les scénarios.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;

thread_local! {
    static APPELS: RefCell<BTreeMap<u32, u64>> = RefCell::new(BTreeMap::new());
}

fn run(pc: u32, c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus, dl: u64) -> Option<u32> {
    APPELS.with(|m| {
        if let Some(n) = m.borrow_mut().get_mut(&pc) {
            *n += 1;
        }
    });
    ddsom_game::run(pc, c, b, dl)
}

/// Code d'origine seul (pour compter les appels des fonctions d'origine, natives ou non).
fn run_origine(pc: u32, c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus, dl: u64) -> Option<u32> {
    APPELS.with(|m| {
        if let Some(n) = m.borrow_mut().get_mut(&pc) {
            *n += 1;
        }
    });
    ddsom_rc::run_block(pc, c, b, dl)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (Some(roms), Some(eeprom), Some(scenario), Some(frames)) = (a.get(1), a.get(2), a.get(3), a.get(4)) else {
        eprintln!("usage : couverture <roms> <eeprom> <scénario> <images> [index.tsv]");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    APPELS.with(|c| {
        for n in ddsom_game::all_natives() {
            c.borrow_mut().insert(n.entry, 0);
        }
    });
    if let Some(index) = a.get(5) {
        for l in std::fs::read_to_string(index).expect("index").lines().skip(1) {
            if let Some(addr) = l.split('\t').next().and_then(|x| u32::from_str_radix(x, 16).ok()) {
                APPELS.with(|c| {
                    c.borrow_mut().entry(addr).or_insert(0);
                });
            }
        }
    }
    m.executor = Some(if a.get(5).is_some() { run_origine } else { run });
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    for f in 0..frames.parse().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    for (entry, n) in APPELS.with(|c| c.borrow().clone()) {
        println!("{entry:06x} {n}");
    }
}
