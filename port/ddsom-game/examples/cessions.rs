//! Ce qu'une tâche laisse sur sa pile 68000 quand elle cède la main : cessions <roms> <eeprom> <images> <scénario>...
//!
//! À chaque image, pour chaque tâche qui a cédé, la table des tâches du jeu donne son adresse de reprise et sa pile ;
//! la profondeur (octets entre le haut de la pile donnée à la tâche et sa pile à la reprise) dit ce qui reste à mettre
//! dans l'état Rust de la tâche pour que l'ordonnanceur appelle ses pas directement : 0, la reprise et les registres
//! suffisent ; sinon des adresses de retour (appels en cours) ou des registres sauvés par la tâche elle-même. Les
//! adresses de retour sur la pile sont listées, et les appels en cours tenus en Rust (`taches::table::Cadre`) :
//! `>suite` pour un appel, `R` pour des registres sauvés, `=v` pour une valeur gardée.
//!
//! Le 68000 restant s'exécute pas à pas, vérifié : une instruction qui dépilerait un appel en cours tenu en Rust
//! arrête l'outil (`taches::table::verifier_68000`).
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ddsom_game::taches::table::{lire, Contenu, Etat};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, images, scenarios @ ..] = &a[..] else {
        eprintln!("usage : cessions <roms> <eeprom> <images> <scénario>...");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let images: usize = images.parse().unwrap();
    // (tâche, reprise, profondeur, appels en cours en Rust) -> (images, contenu de la pile au premier passage)
    let mut vus: BTreeMap<(String, u32, u32, String), (u64, Vec<u32>)> = BTreeMap::new();
    ddsom_game::set_verifier_appels(true);
    let mut taches: BTreeSet<String> = BTreeSet::new();
    let mut raster = false;
    for scenario in scenarios {
        let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
        m.set_overclock(16.0);
        ddsom_game::install(&mut m);
        let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
        for f in 0..images {
            script.apply(f, &mut m);
            m.run_frame();
            // compteurs raster du CPS-B armés (une IRQ de niveau 4 pourrait tomber en cours d'image)
            raster |= m.bus.raster_reload[..2].iter().any(|&r| (r as u32) < cps2::LINES);
            let Some(t) = lire(&m.bus) else { continue };
            for e in &t.emplacements {
                let Some(c) = e.reprise.contexte() else { continue };
                if e.etat == Etat::Libre {
                    continue;
                }
                let nom = format!("{:?}", e.tache);
                taches.insert(nom.clone());
                let profondeur = e.pile.wrapping_sub(c.usp);
                let cadres: Vec<String> = e
                    .cadres
                    .iter()
                    .map(|k| match k.contenu {
                        Contenu::Appel(suite) => format!(">{:x}", suite.adresse()),
                        Contenu::Registres { .. } => "R".into(),
                        Contenu::Valeur(v) => format!("={v:x}"),
                    })
                    .collect();
                let entree = vus.entry((nom, c.pc, profondeur, cadres.join(" "))).or_insert_with(|| {
                    let long = |a: u32| (m.bus.ram[(a as usize & 0xffff) / 2] as u32) << 16 | m.bus.ram[(a as usize & 0xffff) / 2 + 1] as u32;
                    let pile = (0..profondeur.min(0x80) / 4).map(|i| long(c.usp + 4 * i)).collect();
                    (0, pile)
                });
                entree.0 += 1;
            }
        }
    }
    println!("IRQ raster armée : {}", if raster { "oui" } else { "jamais" });
    for nom in &taches {
        let lignes: Vec<_> = vus.iter().filter(|((n, ..), _)| n == nom).collect();
        let profonde = lignes.iter().filter(|((_, _, p, _), _)| *p > 0).count();
        let en_rust = lignes.iter().filter(|((.., k), _)| !k.is_empty()).count();
        println!("{nom} : {} reprises, dont {profonde} avec une pile 68000 non vide ({en_rust} avec des appels en cours en Rust)", lignes.len());
        for ((_, pc, p, cadres), (n, pile)) in lignes {
            let pile: Vec<String> = pile.iter().map(|v| format!("{v:x}")).collect();
            let cadres = if cadres.is_empty() { String::new() } else { format!(" [Rust : {cadres}]") };
            println!("  {pc:6x} profondeur {p:3} ({n} images) {}{cadres}", pile.join(" "));
        }
    }
}
