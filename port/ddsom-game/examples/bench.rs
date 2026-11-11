//! Speed of the game alone (without the reference machine): frames per second on a scenario.
//!
//! bench <roms> <eeprom> <scenario> <frames>
//!
//! The game without the 68000 (`install_no_68k`) and with the Rust sound driver, like machine A of `validate` and the
//! application.
//! `DDSOM_ASSETS=<dir>`: machine built from the assets only.
//! Only `Machine::run_frame` is measured (game, sound driver, QSound), without rendering the frame
//! (`DDSOM_RENDU=1`: with it), nor the application's audio and video output.
//! Basis of the optimisation phase: docs/refactorisation.md, "Performance".
//! Also prints a hash of the final state (work RAM, GFX RAM, object RAM, palette): built without the tools
//! (`--no-default-features`, machine without 68000) it must equal that of the build with the tools.

use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, frames] = &a[..] else {
        eprintln!("usage: bench <roms> <eeprom> <scenario> <frames>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    ddsom_game::refuse_hybrid();
    // the game machine (no ROM) and its data, as in `ddsom`
    let contents = match std::env::var_os("DDSOM_ASSETS") {
        Some(dir) => extract::install_game_assets(Path::new(&dir)).expect("assets"),
        None => extract::install_game(Path::new(roms)).expect("roms"),
    };
    let m = cps2::Machine::game(contents, ee.as_deref());
    let mut m = ddsom_game::install_no_68k(m);
    // DDSOM_RENDU=1: the frame is computed at every frame, as in the application
    m.rendering = std::env::var_os("DDSOM_RENDU").is_some();
    let script = ddsom_game::core::scenario::Scenario::load(Path::new(scenario)).expect("scenario");
    let frames: usize = frames.parse().unwrap();
    let t0 = std::time::Instant::now();
    for f in 0..frames {
        script.apply(f, &mut m);
        m.run_frame();
    }
    let dt = t0.elapsed();
    let residue: u64 = ddsom_game::world::data::residue().iter().map(|r| r.1).sum();
    if residue > 0 {
        println!("residue: {residue} consultations {:?}", ddsom_game::world::data::residue());
    }
    // FNV-1a of the final state
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let hw = &m.bus.hw;
    let words = m.bus.ram.iter().chain(&hw.gfxram).chain(&hw.objram[0]).chain(&hw.objram[1]).chain(&hw.palette);
    for &w in words {
        hash = (hash ^ w as u64).wrapping_mul(0x100_0000_01b3);
    }
    println!("final state hash {hash:016x}");
    println!(
        "{}: {frames} frames in {:.2?}, {:.0} frames/s ({:.1} µs per frame)",
        Path::new(scenario).file_name().unwrap().to_string_lossy(),
        dt,
        frames as f64 / dt.as_secs_f64(),
        dt.as_secs_f64() * 1e6 / frames as f64
    );
}
