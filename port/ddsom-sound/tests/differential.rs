//! The Rust driver must make the same writes as the original Z80 program.
//!
//! The tests read the ROMs from the folder given by `DDSOM_ROMS` (the same one used to build
//! `ddsom-rc`) and skip themselves without it. Short version of `examples/sound_fuzz.rs`; to be
//! run with `--release`.

#[path = "../examples/support/mod.rs"]
mod support;

use ddsom_sound::Rom;
use std::path::PathBuf;
use support::session::{random_session, run_session, song_session, synth_short_session, Session};

fn rom() -> Option<Rom> {
    let dir = PathBuf::from(std::env::var_os("DDSOM_ROMS")?);
    Some(Rom::load(&dir).expect("dd2.01 and dd2.02 in DDSOM_ROMS"))
}

/// Plays each run and requires identity (runs where the Z80 stays busy for too long to be
/// comparable are ignored, but enough of them must be comparable).
fn check(rom: &Rom, sessions: impl Iterator<Item = Session>) {
    let mut compared = 0;
    for (i, s) in sessions.enumerate() {
        let r = run_session(rom, &s, None, None);
        assert!(r.first_divergence.is_none(), "run {i}: {:?}", r.first_divergence);
        assert!(r.slow || r.identical(), "run {i}: different lengths");
        compared += usize::from(!r.slow);
    }
    assert!(compared > 0);
}

#[test]
fn rom_music_and_effects() {
    let Some(rom) = rom() else { return };
    check(&rom, (0..40).map(|n| song_session(&rom, n * 29, 500)));
}

#[test]
fn random_command_sequences() {
    let Some(rom) = rom() else { return };
    check(&rom, (0..25).map(|i| random_session(&rom, 1, i, 600)));
}

#[test]
fn synthetic_tracks() {
    let Some(rom) = rom() else { return };
    check(&rom, (0..60).map(|i| synth_short_session(&rom, 1, i, 250)));
}
