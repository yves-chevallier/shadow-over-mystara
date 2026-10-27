//! Contrôle de la région programme reconstruite depuis les assets (docs/sans-rom.md « Couverture complète des
//! données ») : tout octet reconstruit égale la ROM, dans les deux espaces, et les seuls octets absents sont le code
//! 68000 que `data/reste.json` déclare (`code`). Liste les plages absentes et leur total.
//!
//!   programme_complet <assets> <roms> [--liste]
//!
//! Sans `--liste`, n'affiche que les totaux. Sort en erreur si un octet diffère ou si un octet absent n'est pas du
//! code déclaré.

use std::path::Path;

use ddsom_assets::rebuild::{self, Program, PROGRAM_SIZE};
use ddsom_assets::reste::{plages, Reste};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 {
        eprintln!("usage : programme_complet <assets> <roms> [--liste]");
        std::process::exit(2);
    }
    let (assets, roms) = (Path::new(&a[1]), Path::new(&a[2]));
    let liste = a.iter().any(|x| x == "--liste");
    let p = Program::load(assets).expect("assets");
    let reste = Reste::load(&assets.join("data/reste.json")).expect("data/reste.json");
    let code = reste.plages_de_code().expect("plages de code");
    let mut carte = vec![false; PROGRAM_SIZE];
    for &(d, f) in &code {
        carte[d as usize..f as usize].fill(true);
    }
    let est_code = |x: u32| carte[x as usize];
    let (data, prog) = cps2::load_program(roms).expect("roms");
    let octets = |w: Vec<u16>| -> Vec<u8> { w.iter().flat_map(|v| v.to_be_bytes()).collect() };
    let mut ok = p.missing.is_empty();
    if !ok {
        println!("fichiers absents : {:?}", p.missing);
    }
    for (nom, img, rom) in [("données", &p.data, octets(data)), ("programme", &p.prog, octets(prog))] {
        if !img.conflicts.is_empty() {
            println!("espace {nom} : écritures contradictoires en {:#x}", img.conflicts[0]);
            ok = false;
        }
        if let Some(x) = rebuild::first_difference(img, &rom) {
            println!("espace {nom} : octet {x:#x} reconstruit {:#04x}, ROM {:#04x}", img.bytes[x as usize], rom[x as usize]);
            ok = false;
        }
        let absents = plages(PROGRAM_SIZE as u32, |x| !img.is_covered(x));
        let total: u32 = absents.iter().map(|(d, f)| f - d).sum();
        let hors_code: Vec<&(u32, u32)> = absents.iter().filter(|&&(d, f)| (d..f).any(|x| !est_code(x))).collect();
        println!(
            "espace {nom} : {} octets reconstruits (égaux à la ROM), {total} absents en {} plages, dont {} hors du code déclaré",
            img.covered_len(),
            absents.len(),
            hors_code.len()
        );
        if liste {
            for (d, f) in &absents {
                println!("  {d:06x}-{f:06x} {}", f - d);
            }
        }
        for (d, f) in hors_code.iter().take(20) {
            println!("  absent hors du code : {d:06x}-{f:06x}");
        }
        ok &= hors_code.is_empty();
    }
    println!("code déclaré (data/reste.json) : {} octets en {} plages", reste.octets_de_code, code.len());
    if !ok {
        std::process::exit(1);
    }
    println!("région programme complète hors code");
}
