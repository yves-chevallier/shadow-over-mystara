//! Contrôle de la région programme reconstruite depuis les assets (docs/sans-rom.md « Couverture complète des
//! données ») : tout octet reconstruit égale la ROM, dans les deux espaces, et les seuls octets absents sont le code
//! 68000 que `data/remainder.json` déclare (`code`). Liste les plages absentes et leur total.
//!
//!   full_program <assets> <roms> [--liste]
//!
//! Sans `--list`, n'affiche que les totaux. Sort en erreur si un octet diffère ou si un octet absent n'est pas du
//! code déclaré.

use std::path::Path;

use ddsom_assets::rebuild::{self, Program, PROGRAM_SIZE};
use ddsom_assets::remainder::{ranges, Remainder};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 {
        eprintln!("usage : full_program <assets> <roms> [--liste]");
        std::process::exit(2);
    }
    let (assets, roms) = (Path::new(&a[1]), Path::new(&a[2]));
    let list = a.iter().any(|x| x == "--liste");
    let p = Program::load(assets).expect("assets");
    let remainder = Remainder::load(&assets.join("data/reste.json")).expect("data/reste.json");
    let code = remainder.code_ranges().expect("plages de code");
    let mut card = vec![false; PROGRAM_SIZE];
    for &(d, f) in &code {
        card[d as usize..f as usize].fill(true);
    }
    let is_code = |x: u32| card[x as usize];
    let (data, prog) = cps2::load_program(roms).expect("roms");
    let bytes = |w: Vec<u16>| -> Vec<u8> { w.iter().flat_map(|v| v.to_be_bytes()).collect() };
    let mut ok = p.missing.is_empty();
    if !ok {
        println!("fichiers absents : {:?}", p.missing);
    }
    for (name, img, rom) in [("données", &p.data, bytes(data)), ("programme", &p.prog, bytes(prog))] {
        if !img.conflicts.is_empty() {
            println!("espace {name} : écritures contradictoires en {:#x}", img.conflicts[0]);
            ok = false;
        }
        if let Some(x) = rebuild::first_difference(img, &rom) {
            println!("espace {name} : octet {x:#x} reconstruit {:#04x}, ROM {:#04x}", img.bytes[x as usize], rom[x as usize]);
            ok = false;
        }
        let absent = ranges(PROGRAM_SIZE as u32, |x| !img.is_covered(x));
        let total: u32 = absent.iter().map(|(d, f)| f - d).sum();
        let outside_code: Vec<&(u32, u32)> = absent.iter().filter(|&&(d, f)| (d..f).any(|x| !is_code(x))).collect();
        println!(
            "espace {name} : {} octets reconstruits (égaux à la ROM), {total} absents en {} plages, dont {} hors du code déclaré",
            img.covered_len(),
            absent.len(),
            outside_code.len()
        );
        if list {
            for (d, f) in &absent {
                println!("  {d:06x}-{f:06x} {}", f - d);
            }
        }
        for (d, f) in outside_code.iter().take(20) {
            println!("  absent hors du code : {d:06x}-{f:06x}");
        }
        ok &= outside_code.is_empty();
    }
    println!("code déclaré (data/reste.json) : {} octets en {} plages", remainder.code_bytes, code.len());
    if !ok {
        std::process::exit(1);
    }
    println!("région programme complète hors code");
}
