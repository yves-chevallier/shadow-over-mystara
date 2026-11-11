//! Coverage of the Rust functions of the player core over a scenario: calls of each
//! instrumented function (`cov!()`), including those never called. Native mode without checking.
//!   player_coverage <roms> <eeprom> <scenario> <frames>

use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, frames] = &a[..] else {
        eprintln!("usage: player_coverage <roms> <eeprom> <scenario> <frames>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    let mut m = ddsom_game::install(m);
    ddsom_game::actors::player::cov::enable();
    let script = ddsom_game::core::scenario::Scenario::load(Path::new(scenario)).expect("scenario");
    for f in 0..frames.parse().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    let mut hits = std::collections::BTreeMap::new();
    for (name, n) in ddsom_game::actors::player::cov::report() {
        let short = name.trim_start_matches("ddsom_game::actors::player::").trim_end_matches("::here").to_string();
        *hits.entry(short).or_insert(0u64) += n;
    }
    for name in ddsom_game::actors::player::cov::instrumented() {
        hits.entry(name).or_insert(0);
    }
    for (name, n) in hits {
        println!("{n:>9} {name}");
    }
}
