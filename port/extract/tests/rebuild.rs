//! The extracted resources rebuild the ROMs byte for byte on the zones they describe
//! (graphics region, QSound samples, program ROM tables, at the original addresses).
//! Skipped if the resources (`DDSOM_ASSETS`, otherwise ../../assets) or the ROMs (`DDSOM_ROMS`, otherwise ../../rom)
//! are missing.

use std::path::{Path, PathBuf};

use extract::rebuild::{self, Image, Program};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn roms() -> PathBuf {
    std::env::var_os("DDSOM_ROMS").map(PathBuf::from).unwrap_or_else(|| root().join("rom"))
}

fn assets() -> PathBuf {
    std::env::var_os("DDSOM_ASSETS").map(PathBuf::from).unwrap_or_else(|| root().join("assets"))
}

fn available() -> bool {
    let ok = assets().join("palettes.png").exists() && roms().join("dd2.13m").exists();
    if !ok {
        eprintln!("resources or ROMs missing: test skipped");
    }
    ok
}

fn same(name: &str, img: &Image, rom: &[u8]) {
    assert!(img.conflicts.is_empty(), "{name}: contradictory writes at {:#x}", img.conflicts[0]);
    if let Some(a) = rebuild::first_difference(img, rom) {
        panic!("{name}: byte {a:#x} differs ({:#04x} instead of {:#04x})", img.bytes[a as usize], rom[a as usize]);
    }
}

#[test]
fn rebuilt_program() {
    if !available() {
        return;
    }
    let p = Program::load(&assets()).unwrap();
    assert!(p.missing.is_empty(), "missing files: {:?}", p.missing);
    let (data, prog) = cps2::load_program(&roms()).unwrap();
    let bytes = |w: Vec<u16>| -> Vec<u8> { w.iter().flat_map(|v| v.to_be_bytes()).collect() };
    same("data space", &p.data, &bytes(data));
    same("program space", &p.prog, &bytes(prog));
    // most of the tables are there: palettes (256 KB), frames (over 2 MB with the animations)
    assert!(p.data.covered_len() > 0x200000, "{}", p.data.covered_len());
}

/// With `data/reste.json`, the program region is complete: the only missing bytes (in both spaces)
/// are 68000 code declared by that file (docs/sans-rom.md "full data coverage").
#[test]
fn full_program_outside_code() {
    if !available() {
        return;
    }
    let p = Program::load(&assets()).unwrap();
    let remainder = ddsom_assets::remainder::Remainder::load(&assets().join("data/reste.json")).unwrap();
    let mut code = vec![false; rebuild::PROGRAM_SIZE];
    for (d, f) in remainder.code_ranges().unwrap() {
        code[d as usize..f as usize].fill(true);
    }
    for (name, img) in [("data space", &p.data), ("program space", &p.prog)] {
        let absent = (0..rebuild::PROGRAM_SIZE as u32).find(|&a| !img.is_covered(a) && !code[a as usize]);
        assert!(absent.is_none(), "{name}: byte {:#x} missing from the assets and outside the code", absent.unwrap());
    }
}

#[test]
fn rebuilt_graphics() {
    if !available() {
        return;
    }
    let region = rebuild::gfx_region(&assets().join("gfx")).unwrap();
    let rom = cps2_gfx::rom::load_gfx(&roms(), cps2_gfx::rom::DDSOM_GFX, cps2_gfx::rom::DDSOM_GFX_SIZE).unwrap();
    assert!(region == rom, "graphics region differs");
}

#[test]
fn rebuilt_samples() {
    if !available() {
        return;
    }
    let img = ddsom_assets::samples::bank(&assets()).unwrap();
    let mut pcm = Vec::new();
    for name in ["dd2.11m", "dd2.12m"] {
        pcm.extend(std::fs::read(roms().join(name)).unwrap().as_chunks::<2>().0.iter().flat_map(|w| [w[1], w[0]]));
    }
    assert_eq!(img.bytes.len(), pcm.len());
    same("samples", &img, &pcm);
    assert!(img.covered_len() > 0x100000, "{}", img.covered_len());
}
