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
    for (a, b) in from_roms.effects.tables().into_iter().zip(from_assets.effects.tables()) {
        assert!(a == b, "effect table {} differs", a.name);
    }
    assert!(from_roms.sprite_tables == from_assets.sprite_tables, "sprite tables differ");
    for (a, b) in from_roms.objects.tables().into_iter().zip(from_assets.objects.tables()) {
        assert!(a == b, "object table {} differs", a.name);
    }
    let tables = |d: &ddsom_data::DataFile| -> Vec<_> {
        d.tables.iter().map(|t| (t.name.clone(), t.rom, t.program, t.bytes().unwrap())).collect()
    };
    let (a, b) = (tables(&from_roms.enemies), tables(&from_assets.enemies));
    if let Some((x, y)) = a.iter().zip(&b).find(|(x, y)| x != y) {
        panic!("enemy tables differ: {} {:#x} / {} {:#x}", x.0, x.1, y.0, y.1);
    }
    assert!(a.len() == b.len(), "enemy tables differ in number");
    let (a, b) = (tables(&from_roms.set), tables(&from_assets.set));
    if let Some((x, y)) = a.iter().zip(&b).find(|(x, y)| x != y) {
        panic!("SET tables differ: {} {:#x} / {} {:#x}", x.0, x.1, y.0, y.1);
    }
    assert!(a.len() == b.len(), "SET tables differ in number");
    assert!(tables(&from_roms.dialogs) == tables(&from_assets.dialogs), "dialogs differ");
    assert!(tables(&from_roms.scenes) == tables(&from_assets.scenes), "scene tables differ");
    assert!(tables(&from_roms.misc) == tables(&from_assets.misc), "misc tables differ");
    assert!(tables(&from_roms.screens) == tables(&from_assets.screens), "screen tables differ");
    assert!(from_roms.texts == from_assets.texts, "texts differ");
    for (a, b) in from_roms.player_tables.tables().into_iter().zip(from_assets.player_tables.tables()) {
        assert!(a == b, "player table {} differs", a.name);
    }
    assert!(tables(&from_roms.players) == tables(&from_assets.players), "player tables (players.json) differ");
    assert!(tables(&from_roms.combat) == tables(&from_assets.combat), "combat tables differ");
    assert!(from_roms.levels == from_assets.levels, "levels differ");
    assert!(from_roms.terrain == from_assets.terrain, "terrain differs");
    assert!(from_roms.scene_palettes == from_assets.scene_palettes, "scene palettes differ");
    assert!(from_roms.palettes == from_assets.palettes, "palette library differs");
    assert!(tables(&from_roms.engine) == tables(&from_assets.engine), "engine tables differ");
    assert!(tables(&from_roms.rest) == tables(&from_assets.rest), "rest tables differ");
    assert!(tables(&from_roms.boxes) == tables(&from_assets.boxes), "crate tables differ");
}
