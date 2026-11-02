//! Generates the recompiled code from the local ROMs (never versioned: derived from the game).
//! ROMs: $DDSOM_ROMS or ../../rom. Extra entry points: entries.txt (hex addresses,
//! learned at run time). Without ROMs, generates an empty module (everything is interpreted).

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
            println!("cargo:warning=ROMs not found ({e}): no recompiled code, everything will be interpreted");
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
