//! Speed of the game alone (without the reference machine): frames per second on a scenario.
//!
//! bench <roms> <eeprom> <scenario> <frames>
//!
//! The game without the 68000 (`install_no_68k`), like machine A of `validate`. `DDSOM_SON_RUST=1`: Rust sound driver
//! instead of the emulated Z80 (like the application).
//! `DDSOM_ASSETS=<dir>`: machine built from the assets only.
//! Only `Machine::run_frame` is measured (game, Z80 or sound driver, QSound), without rendering the frame
//! (`DDSOM_RENDU=1`: with it), nor the application's audio and video output.
//! Basis of the optimisation phase: docs/refactorisation.md, « Performance ».

use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, frames] = &a[..] else {
        eprintln!("usage: bench <roms> <eeprom> <scenario> <frames>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    ddsom_game::refuse_hybrid();
    let mut m = match std::env::var_os("DDSOM_ASSETS") {
        Some(dir) => {
            let contents = ddsom_assets::rebuild::contents(Path::new(&dir)).expect("assets");
            let mut m = cps2::Machine::ddsom(contents, ee.as_deref());
            m.sound.use_rust_driver();
            m
        }
        None => {
            let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
            m.set_overclock(16.0);
            if std::env::var_os("DDSOM_SON_RUST").is_some() {
                m.sound.use_rust_driver();
            }
            m
        }
    };
    ddsom_game::install_no_68k(&mut m);
    // DDSOM_RENDU=1: the frame is computed at every frame, as in the application
    m.rendering = std::env::var_os("DDSOM_RENDU").is_some();
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scenario");
    let frames: usize = frames.parse().unwrap();
    let t0 = std::time::Instant::now();
    for f in 0..frames {
        script.apply(f, &mut m);
        m.run_frame();
    }
    let dt = t0.elapsed();
    println!(
        "{}: {frames} frames in {:.2?}, {:.0} frames/s ({:.1} µs per frame)",
        Path::new(scenario).file_name().unwrap().to_string_lossy(),
        dt,
        frames as f64 / dt.as_secs_f64(),
        dt.as_secs_f64() * 1e6 / frames as f64
    );
}
