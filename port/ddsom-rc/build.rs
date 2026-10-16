//! Génère le code recompilé à partir des ROMs locales (jamais versionné : dérivé du jeu).
//! ROMs : $DDSOM_ROMS ou ../../rom. Points d'entrée supplémentaires : entries.txt (adresses hex,
//! appris à l'exécution). Sans ROMs, génère un module vide (tout est interprété).

use std::path::PathBuf;

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let roms = std::env::var_os("DDSOM_ROMS").map(PathBuf::from).unwrap_or_else(|| manifest.join("../../rom"));
    println!("cargo:rerun-if-env-changed=DDSOM_ROMS");
    println!("cargo:rerun-if-changed=entries.txt");
    println!("cargo:rerun-if-changed={}", roms.join("ddsom.key").display());

    let modules: usize = std::env::var("DDSOM_RC_MODULES").ok().and_then(|v| v.parse().ok()).unwrap_or(48);
    let files = match cps2::load_program(&roms) {
        Ok((data, prog)) => {
            let extra: Vec<u32> = std::fs::read_to_string(manifest.join("entries.txt"))
                .unwrap_or_default()
                .lines()
                .filter_map(|l| u32::from_str_radix(l.split('#').next().unwrap().trim(), 16).ok())
                .collect();
            let mut img = recomp::Image { prog: &prog, data: &data };
            let p = recomp::discover_with_extra(&mut img, &extra);
            recomp::generate(&p, &recomp::Options { bus_type: "cps2::Cps2Bus", modules, max_block: 64 })
        }
        Err(e) => {
            println!("cargo:warning=ROMs introuvables ({e}) : aucun code recompilé, tout sera interprété");
            vec![(
                "mod.rs".to_string(),
                "pub const BLOCKS: usize = 0;\n\
                 pub fn run_block(_: u32, _: &mut m68k::Cpu, _: &mut cps2::Cps2Bus, _: u64) -> Option<u32> { None }\n"
                    .to_string(),
            )]
        }
    };
    for (name, content) in files {
        std::fs::write(out.join(name), content).unwrap();
    }
}
