//! Extraction of the Shadow over Mystara resources from the user's ROMs: the `extract` program (all the assets,
//! docs/assets.md) and, in memory, the game data of the mode with ROMs ([`to_data`]).
//!
//! The library has no 68000: only the `remainder` category (feature `full`, the program) decodes the original code
//! to tell its bytes from the data.

pub mod combat;
pub mod data;
pub mod dialogs;
pub mod effects;
pub mod enemies;
pub mod gfx;
pub mod levels;
pub mod misc;
pub mod music;
pub mod objects;
pub mod natives;
pub mod palettes;
pub mod player_tables;
pub mod players;
pub mod rest;
#[cfg(feature = "full")]
pub mod remainder;
pub mod rom;
pub mod scene_palettes;
pub mod scenes;
pub mod set;
pub mod sounds;
pub mod spawns;
pub mod speeds;
pub mod sprites;
pub mod tables;
pub mod terrain;
pub mod texts;

use std::io;
use std::path::Path;

use ddsom_data::GameData;

pub use rom::Program;

/// The game data, extracted in memory from the ROMs of `roms` (the same tables as the assets folder).
pub fn to_data(roms: &Path) -> io::Result<GameData> {
    to_data_from(&Program::load(roms)?)
}

/// The game data of a program ROM already loaded (data and decrypted program views).
pub fn to_data_from(rom: &Program) -> io::Result<GameData> {
    let frames = sprites::frames(rom)?;
    let anims = sprites::anims(rom, &frames)?;
    let (regions, specs) = combat::enemies(rom);
    let enemies = data::build(rom, regions, &specs, Vec::new());
    let (regions, specs, texts) = dialogs::specs(rom);
    let dialogs = data::build(rom, regions, &specs, texts);
    let (regions, specs) = players::specs(rom);
    let players = data::build(rom, regions, &specs, Vec::new());
    let (regions, specs) = combat::combat();
    let combat = data::build(rom, regions, &specs, Vec::new());
    Ok(GameData {
        frames,
        anims,
        speeds: speeds::speeds(rom),
        effects: effects::effects(rom),
        enemies,
        objects: objects::objects(rom),
        sprite_tables: tables::sprite_tables(rom)?,
        set: set::set(rom),
        dialogs,
        player_tables: player_tables::player_tables(rom),
        players,
        combat,
    })
}
