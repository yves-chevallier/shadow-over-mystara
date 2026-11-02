//! The game data extracted in memory from the ROMs (mode with ROMs) equal those loaded from the assets (mode without
//! ROM): the two modes of the game read the same tables. Needs the ROMs (`DDSOM_ROMS`, otherwise `rom/`) and assets
//! extracted by the same version of `extract` (`DDSOM_ASSETS`, otherwise `assets/`).

use std::path::{Path, PathBuf};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn rom_and_assets_give_the_same_data() {
    let roms = std::env::var_os("DDSOM_ROMS").map(PathBuf::from).unwrap_or_else(|| root().join("rom"));
    let assets = std::env::var_os("DDSOM_ASSETS").map(PathBuf::from).unwrap_or_else(|| root().join("assets"));
    if !roms.join("dd2.13m").exists() || !ddsom_data::GameData::FILES.iter().all(|f| assets.join(f).exists()) {
        eprintln!("ROMs or assets missing: test skipped");
        return;
    }
    let from_roms = extract::to_data(&roms).unwrap();
    let from_assets = ddsom_data::GameData::load(&assets).unwrap();
    assert!(from_roms.frames == from_assets.frames, "frames differ");
    assert!(from_roms.anims == from_assets.anims, "animations differ");
    assert!(from_roms.speeds == from_assets.speeds, "speeds differ");
    let tables = |d: &ddsom_data::DataFile| -> Vec<_> {
        d.tables.iter().map(|t| (t.name.clone(), t.rom, t.program, t.bytes().unwrap())).collect()
    };
    assert!(tables(&from_roms.set) == tables(&from_assets.set), "SET tables differ");
    assert!(tables(&from_roms.dialogs) == tables(&from_assets.dialogs), "dialogs differ");
}
