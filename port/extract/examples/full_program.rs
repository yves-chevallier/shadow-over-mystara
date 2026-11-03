//! Check of the program region rebuilt from the assets (docs/sans-rom.md "full data coverage"
//! section): every rebuilt byte equals the ROM, in both spaces, and the only missing bytes are the 68000
//! code that `data/reste.json` declares (`code`). Lists the missing ranges and their total.
//!
//!   full_program <assets> <roms> [--liste]
//!
//! Without `--liste`, prints only the totals. Exits with an error if a byte differs or if a missing byte is not
//! declared code.

use std::path::Path;

use extract::rebuild::{self, Program, PROGRAM_SIZE};
use ddsom_assets::remainder::{ranges, Remainder};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 3 {
        eprintln!("usage: full_program <assets> <roms> [--liste]");
        std::process::exit(2);
    }
    let (assets, roms) = (Path::new(&a[1]), Path::new(&a[2]));
    let list = a.iter().any(|x| x == "--liste");
    let p = Program::load(assets).expect("assets");
    let remainder = Remainder::load(&assets.join("data/reste.json")).expect("data/reste.json");
    let code = remainder.code_ranges().expect("code ranges");
    let mut card = vec![false; PROGRAM_SIZE];
    for &(d, f) in &code {
        card[d as usize..f as usize].fill(true);
    }
    let is_code = |x: u32| card[x as usize];
    let (data, prog) = cps2::load_program(roms).expect("roms");
    let bytes = |w: Vec<u16>| -> Vec<u8> { w.iter().flat_map(|v| v.to_be_bytes()).collect() };
    let mut ok = p.missing.is_empty();
    if !ok {
        println!("missing files: {:?}", p.missing);
    }
    for (name, img, rom) in [("data", &p.data, bytes(data)), ("program", &p.prog, bytes(prog))] {
        if !img.conflicts.is_empty() {
            println!("{name} space: contradictory writes at {:#x}", img.conflicts[0]);
            ok = false;
        }
        if let Some(x) = rebuild::first_difference(img, &rom) {
            println!("{name} space: byte {x:#x} rebuilt {:#04x}, ROM {:#04x}", img.bytes[x as usize], rom[x as usize]);
            ok = false;
        }
        let absent = ranges(PROGRAM_SIZE as u32, |x| !img.is_covered(x));
        let total: u32 = absent.iter().map(|(d, f)| f - d).sum();
        let outside_code: Vec<&(u32, u32)> = absent.iter().filter(|&&(d, f)| (d..f).any(|x| !is_code(x))).collect();
        println!(
            "{name} space: {} bytes rebuilt (equal to the ROM), {total} missing in {} ranges, of which {} outside the declared code",
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
            println!("  missing outside the code: {d:06x}-{f:06x}");
        }
        ok &= outside_code.is_empty();
    }
    println!("declared code (data/reste.json): {} bytes in {} ranges", remainder.code_bytes, code.len());
    if !ok {
        std::process::exit(1);
    }
    println!("program region complete outside the code");
}
