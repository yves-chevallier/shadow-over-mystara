//! Ce qu'une tâche laisse sur sa pile 68000 quand elle cède la main : cessions <roms> <eeprom> <images> <scénario>...
//!
//! À chaque image, pour chaque tâche qui a cédé, la table des tâches du jeu donne son adresse de reprise et sa pile ;
//! la profondeur (octets entre le haut de la pile donnée à la tâche et sa pile à la reprise) dit ce qui reste à mettre
//! dans l'état Rust de la tâche pour que l'ordonnanceur appelle ses pas directement : 0, la reprise et les registres
//! suffisent ; sinon des adresses de retour (appels en cours) ou des registres sauvés par la tâche elle-même. Les
//! adresses de retour sur la pile sont listées.
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ddsom_game::taches::table::{lire, Etat, Reprise};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, images, scenarios @ ..] = &a[..] else {
        eprintln!("usage : cessions <roms> <eeprom> <images> <scénario>...");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let images: usize = images.parse().unwrap();
    // (tâche, reprise, profondeur) -> (images, contenu de la pile au premier passage)
    let mut vus: BTreeMap<(String, u32, u32), (u64, Vec<u32>)> = BTreeMap::new();
    let mut taches: BTreeSet<String> = BTreeSet::new();
    for scenario in scenarios {
        let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
        m.set_overclock(16.0);
        ddsom_game::install(&mut m);
        let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
        for f in 0..images {
            script.apply(f, &mut m);
            m.run_frame();
            let Some(t) = lire(&m.bus) else { continue };
            for e in &t.emplacements {
                let Reprise::Cession(c) = e.reprise else { continue };
                if e.etat == Etat::Libre {
                    continue;
                }
                let nom = format!("{:?}", e.tache);
                taches.insert(nom.clone());
                let profondeur = e.pile.wrapping_sub(c.usp);
                let entree = vus.entry((nom, c.pc, profondeur)).or_insert_with(|| {
                    let long = |a: u32| (m.bus.ram[(a as usize & 0xffff) / 2] as u32) << 16 | m.bus.ram[(a as usize & 0xffff) / 2 + 1] as u32;
                    let pile = (0..profondeur.min(0x80) / 4).map(|i| long(c.usp + 4 * i)).collect();
                    (0, pile)
                });
                entree.0 += 1;
            }
        }
    }
    for nom in &taches {
        let lignes: Vec<_> = vus.iter().filter(|((n, _, _), _)| n == nom).collect();
        let profonde = lignes.iter().filter(|((_, _, p), _)| *p > 0).count();
        println!("{nom} : {} reprises, dont {profonde} avec une pile non vide", lignes.len());
        for ((_, pc, p), (n, pile)) in lignes {
            let pile: Vec<String> = pile.iter().map(|v| format!("{v:x}")).collect();
            println!("  {pc:6x} profondeur {p:3} ({n} images) {}", pile.join(" "));
        }
    }
}
