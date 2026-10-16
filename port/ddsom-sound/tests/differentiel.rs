//! Le pilote Rust doit faire les mêmes écritures que le programme du Z80 d'origine.
//!
//! Les tests lisent les ROMs dans le dossier désigné par `DDSOM_ROMS` (le même que pour compiler
//! `ddsom-rc`) et s'ignorent sans lui. Version courte de `examples/son_fuzz.rs` ; à lancer en
//! `--release`.

#[path = "../examples/support/mod.rs"]
mod support;

use ddsom_sound::Rom;
use std::path::PathBuf;
use support::session::{random_session, run_session, song_session, synth_short_session, Session};

fn rom() -> Option<Rom> {
    let dir = PathBuf::from(std::env::var_os("DDSOM_ROMS")?);
    Some(Rom::load(&dir).expect("dd2.01 et dd2.02 dans DDSOM_ROMS"))
}

/// Joue chaque essai et exige l'identité (les essais où le Z80 reste occupé trop longtemps pour
/// être comparable sont ignorés, mais il en faut assez qui le soient).
fn check(rom: &Rom, sessions: impl Iterator<Item = Session>) {
    let mut compared = 0;
    for (i, s) in sessions.enumerate() {
        let r = run_session(rom, &s, None, None);
        assert!(r.first_divergence.is_none(), "essai {i} : {:?}", r.first_divergence);
        assert!(r.slow || r.identical(), "essai {i} : longueurs différentes");
        compared += usize::from(!r.slow);
    }
    assert!(compared > 0);
}

#[test]
fn musiques_et_effets_de_la_rom() {
    let Some(rom) = rom() else { return };
    check(&rom, (0..40).map(|n| song_session(&rom, n * 29, 500)));
}

#[test]
fn suites_aleatoires_de_commandes() {
    let Some(rom) = rom() else { return };
    check(&rom, (0..25).map(|i| random_session(&rom, 1, i, 600)));
}

#[test]
fn pistes_synthetiques() {
    let Some(rom) = rom() else { return };
    check(&rom, (0..60).map(|i| synth_short_session(&rom, 1, i, 250)));
}
