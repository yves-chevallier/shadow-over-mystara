//! Validation différentielle sans le jeu : le Z80 d'origine seul et le pilote Rust reçoivent les
//! mêmes commandes synthétiques dans la RAM partagée, et on compare leurs écritures. Cela exerce
//! toute la ROM (toutes les musiques et tous les effets, toutes les commandes système) et pas
//! seulement ce que le scénario du jeu envoie.
//!
//! son_fuzz <roms> songs <premier> <dernier> <interruptions>  un essai par numéro de musique/effet
//! son_fuzz <roms> random <graine> <essais> <interruptions>   suites aléatoires de commandes
//! son_fuzz <roms> synth <graine> <essais> <interruptions>    sons synthétiques ajoutés à la ROM
//! son_fuzz <roms> short <graine> <essais> <interruptions>    idem avec des pistes courtes
//! son_fuzz <roms> one songs <numéro> <interruptions> [de a]            un essai, avec le détail
//! son_fuzz <roms> one random|synth|short <graine> <essai> <interruptions> [de a]
//!
//! Variables d'environnement : FUZZ_STATE=1 compare aussi la RAM du Z80 à l'état du pilote (quand
//! le Z80 est au repos) ; FUZZ_COVERAGE=fichier écrit la couverture du programme du Z80 ;
//! FUZZ_SHOW, FUZZ_PROFILE, FUZZ_BREAK=adresse, FUZZ_WATCH=adresse : aides au débogage de `one`.

#[path = "support/mod.rs"]
mod support;

use ddsom_sound::Rom;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use support::session::*;
use support::Report;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 4 {
        eprintln!("usage : son_fuzz <roms> songs|random|synth|short|one ... (voir l'en-tête du fichier)");
        std::process::exit(2);
    }
    let rom = Rom::load(Path::new(&a[1])).expect("rom son");
    let num = |i: usize| a[i].parse::<usize>().expect("nombre");
    match a[2].as_str() {
        "songs" => {
            let (first, last, irqs) = (num(3), num(4), num(5));
            run_many(&rom, first..=last, |n| song_session(&rom, n, irqs));
        }
        "random" => {
            let (seed, count, irqs) = (num(3), num(4), num(5));
            run_many(&rom, 0..=count.saturating_sub(1), |i| random_session(&rom, seed, i, irqs));
        }
        "synth" => {
            let (seed, count, irqs) = (num(3), num(4), num(5));
            run_many(&rom, 0..=count.saturating_sub(1), |i| synth_session(&rom, seed, i, irqs));
        }
        "short" => {
            let (seed, count, irqs) = (num(3), num(4), num(5));
            run_many(&rom, 0..=count.saturating_sub(1), |i| synth_short_session(&rom, seed, i, irqs));
        }
        "one" => {
            // one songs <numéro> <interruptions> [de a] | one random <graine> <essai> <interruptions> [de a]
            let dump_at = |i| (a.len() > i + 1).then(|| (num(i), num(i + 1)));
            let (s, dump) = match a[3].as_str() {
                "songs" => (song_session(&rom, num(4), num(5)), dump_at(6)),
                "synth" => (synth_session(&rom, num(4), num(5), num(6)), dump_at(7)),
                "short" => (synth_short_session(&rom, num(4), num(5), num(6)), dump_at(7)),
                _ => (random_session(&rom, num(4), num(5), num(6)), dump_at(7)),
            };
            for (at, c) in &s.commands {
                println!("interruption {at} : commande {c:02x?}");
            }
            if std::env::var("FUZZ_SHOW").is_ok() {
                for (at, bytes) in s.patches.iter().filter(|p| p.0 >= 0x48000) {
                    println!("données à la région {at:#x} (adresse linéaire {:#x}) : {}", at - 0x8000, bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" "));
                }
            }
            summary(&run_session(&rom, &s, dump, None));
        }
        _ => eprintln!("mode inconnu"),
    }
}

fn summary(r: &Report) {
    println!(
        "Z80 {} écritures, Rust {}, identiques {} (+{} acquittements), débordements {}",
        r.real.total, r.drv.total, r.matched_main, r.matched_acks, r.spill_intervals
    );
    match &r.first_divergence {
        None if r.identical() => println!("IDENTIQUE"),
        None => println!("préfixe identique, longueurs différentes"),
        Some(d) => println!("DIVERGENCE : {d}"),
    }
}

/// Exécute les essais `range` en parallèle et résume.
fn run_many(rom: &Rom, range: std::ops::RangeInclusive<usize>, make: impl Fn(usize) -> Session + Sync) {
    let next = AtomicUsize::new(*range.start());
    let last = *range.end();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let results = std::sync::Mutex::new(Vec::new());
    let coverage = std::sync::Mutex::new(vec![false; 0x10000]);
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i > last {
                    break;
                }
                let s = make(i);
                let mut cov = vec![false; 0x10000];
                let r = run_session(rom, &s, None, Some(&mut cov));
                for (c, x) in coverage.lock().unwrap().iter_mut().zip(&cov) {
                    *c |= x;
                }
                results.lock().unwrap().push((i, r.identical(), r.matched_main + r.matched_acks, r.first_divergence, r.slow));
            });
        }
    });
    if let Ok(path) = std::env::var("FUZZ_COVERAGE") {
        let bytes: Vec<u8> = coverage.into_inner().unwrap().iter().map(|&c| c as u8).collect();
        std::fs::write(path, bytes).unwrap();
    }
    let mut results = results.into_inner().unwrap();
    results.sort_by_key(|r| r.0);
    let slow = results.iter().filter(|r| r.4 && r.3.is_none()).count();
    let ok = results.iter().filter(|r| r.1 || (r.4 && r.3.is_none())).count() - slow;
    let writes: usize = results.iter().map(|r| r.2).sum();
    for (i, identical, _, div, slow) in &results {
        if !identical && !(*slow && div.is_none()) {
            println!("essai {i} : {}", div.as_deref().unwrap_or("longueurs différentes"));
        }
    }
    println!(
        "{ok} essais identiques sur {} ({writes} écritures comparées) ; {slow} abandonnés : le Z80 est resté occupé plus de {MAX_BUSY} interruptions (durée d'exécution non modélisée)",
        results.len()
    );
}
