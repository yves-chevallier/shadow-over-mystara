//! Validation des routines natives sur un scénario : deux machines en parallèle.
//!   A : routines natives en mode contrôle (chaque appel comparé à l'original, timing de l'original)
//!   B : code d'origine seul (recompilé)
//! Toute différence de RAM (hors piles des tâches) entre A et B est une erreur de réécriture.
//!
//! validate <roms> <eeprom> <scenario> <images>

use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, frames] = &a[..] else {
        eprintln!("usage : validate <roms> <eeprom> <scenario> <images>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let load = || cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    let (mut ma, mut mb) = (load(), load());
    ddsom_game::install(&mut ma);
    ddsom_game::set_check(true);
    ddsom_rc::install(&mut mb);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    let frames: usize = frames.parse().unwrap();
    // Zones de sauvegarde de registres : piles des tâches (FF0A00-FF0FFF) et pile logicielle de
    // l'interruption VBLANK (sous FF2A1A). Un registre laissé libre par une routine native n'y
    // est qu'une copie ; s'il était réellement utilisé, l'écart apparaîtrait ailleurs.
    // + le SR sauvegardé de chaque tâche (mot +2 des entrées de 16 octets en FF0064).
    let saved = |a: usize| {
        (0x0a00..0x1000).contains(&a)
            || (0x2900..0x2a1a).contains(&a)
            || ((0x0064..0x0164).contains(&a) && (a - 0x64) % 16 == 2)
    };
    let t0 = std::time::Instant::now();
    let mut first = None;
    for f in 0..frames {
        script.apply(f, &mut ma);
        script.apply(f, &mut mb);
        ma.run_frame();
        mb.run_frame();
        let diff: Vec<usize> = (0..0x8000)
            .filter(|&i| ma.bus.ram[i] != mb.bus.ram[i] && !saved(2 * i))
            .collect();
        if !diff.is_empty() && first.is_none() {
            first = Some(f + 1);
            println!("image {} : {} mots différents", f + 1, diff.len());
            for &i in diff.iter().take(10) {
                println!("  {:06x}: natif {:04x}  origine {:04x}", 0xff0000 + 2 * i, ma.bus.ram[i], mb.bus.ram[i]);
            }
        }
    }
    println!("{frames} images en {:.1?}", t0.elapsed());
    for (name, st) in ddsom_game::report() {
        println!("  {name:28} {:>7} appels {:>5} écarts{}", st.calls, st.mismatches,
            st.first.map(|f| format!(" — {f}")).unwrap_or_default());
    }
    let errors: u64 = ddsom_game::report().values().map(|s| s.mismatches).sum();
    match (first, errors) {
        (None, 0) => println!("VALIDE : RAM identique sur {frames} images, aucun écart"),
        (None, n) => println!("ÉCHEC : {n} appels natifs en écart (état rétabli depuis l'original à chaque fois)"),
        (Some(f), _) => println!("ÉCHEC : première différence de RAM à l'image {f}"),
    }
}
