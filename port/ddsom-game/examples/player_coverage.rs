//! Couverture des fonctions Rust du noyau joueur sur un scénario : appels de chaque fonction
//! instrumentée (`cov!()`), y compris celles jamais appelées. Mode natif sans contrôle.
//!   player_coverage <roms> <eeprom> <scénario> <images>

use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, frames] = &a[..] else {
        eprintln!("usage : player_coverage <roms> <eeprom> <scénario> <images>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    ddsom_game::install(&mut m);
    ddsom_game::player::cov::enable();
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    for f in 0..frames.parse().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    let mut hits = std::collections::BTreeMap::new();
    for (name, n) in ddsom_game::player::cov::report() {
        let short = name.trim_start_matches("ddsom_game::player::").trim_end_matches("::here").to_string();
        *hits.entry(short).or_insert(0u64) += n;
    }
    for name in ddsom_game::player::cov::instrumented() {
        hits.entry(name).or_insert(0);
    }
    for (name, n) in hits {
        println!("{n:>9} {name}");
    }
    for (addr, n) in ddsom_game::player::cov::rom_report() {
        println!("{n:>9} ROM {addr:06x}");
    }
}
