//! Vitesse du jeu seul (sans machine de référence) : images par seconde sur un scénario.
//!
//! bench <roms> <eeprom> <scenario> <images>
//!
//! Le jeu sans 68000 (`install_sans_68k`), comme la machine A de `validate`. `DDSOM_SON_RUST=1` : pilote son Rust au lieu du Z80 émulé (comme l'application).
//! `DDSOM_ASSETS=<dossier>` : machine faite des seuls assets.
//! Seul `Machine::run_frame` est mesuré (jeu, Z80 ou pilote son, QSound), sans le rendu de l'image
//! (`DDSOM_RENDU=1` : avec), ni la sortie audio et vidéo de l'application. Base de la phase d'optimisation : docs/refactorisation.md, « Performance ».

use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, frames] = &a[..] else {
        eprintln!("usage : bench <roms> <eeprom> <scenario> <images>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    ddsom_game::refuser_hybride();
    let mut m = match std::env::var_os("DDSOM_ASSETS") {
        Some(dir) => {
            let contenus = ddsom_assets::rebuild::contenus(Path::new(&dir)).expect("assets");
            let mut m = cps2::Machine::ddsom(contenus, ee.as_deref());
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
    ddsom_game::install_sans_68k(&mut m);
    // DDSOM_RENDU=1 : l'image est calculée à chaque image, comme dans l'application
    m.rendu = std::env::var_os("DDSOM_RENDU").is_some();
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    let frames: usize = frames.parse().unwrap();
    let t0 = std::time::Instant::now();
    for f in 0..frames {
        script.apply(f, &mut m);
        m.run_frame();
    }
    let dt = t0.elapsed();
    println!(
        "{} : {frames} images en {:.2?}, {:.0} images/s ({:.1} µs par image)",
        Path::new(scenario).file_name().unwrap().to_string_lossy(),
        dt,
        frames as f64 / dt.as_secs_f64(),
        dt.as_secs_f64() * 1e6 / frames as f64
    );
}
