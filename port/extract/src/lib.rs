//! Extraction of the Shadow over Mystara resources from the user's ROMs: the `extract` program (all the assets,
//! docs/assets.md) and, in memory, the game data of the mode with ROMs ([`to_data`]).
//!
//! The library has no 68000: only the `remainder` category (feature `full`, the program) decodes the original code
//! to tell its bytes from the data.

pub mod bosses;
pub mod combat;
pub mod data;
pub mod dialogs;
pub mod effects;
pub mod enemies;
pub mod engine;
pub mod gfx;
pub mod levels;
pub mod misc;
pub mod music;
pub mod objects;
pub mod overreads;
pub mod natives;
pub mod palettes;
pub mod player_tables;
pub mod rebuild;
pub mod players;
pub mod rest;
#[cfg(feature = "full")]
pub mod remainder;
pub mod rom;
pub mod scene_palettes;
pub mod scenes;
pub mod screens;
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

/// The game of the mode with ROMs: installs the game data extracted in memory and the residue (the program), returns
/// what the game machine holds (`cps2::Machine::game`). No ROM image is kept on the machine.
pub fn install_game(roms: &Path) -> io::Result<cps2::GameContents> {
    let program = Program::load(roms)?;
    ddsom_data::install(to_data_from(&program)?);
    ddsom_data::residue::install(ddsom_data::residue::Source::Ready(program.residue()));
    cps2::GameContents::from_roms(roms)
}

/// The game of the mode without ROM: installs the game data and the residue (read at its first consultation) of the
/// assets folder `dir`, returns what the game machine holds. No ROM file is opened.
pub fn install_game_assets(dir: &Path) -> io::Result<cps2::GameContents> {
    ddsom_data::install(GameData::load(dir)?);
    ddsom_data::residue::install(ddsom_data::residue::Source::Assets(dir.to_path_buf()));
    ddsom_assets::game_contents(dir)
}

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
    let (regions, specs) = scenes::specs(rom);
    let scenes = data::build(rom, regions, &specs, Vec::new());
    let (regions, specs, texts) = misc::specs(rom);
    let misc = data::build(rom, regions, &specs, texts);
    let (regions, specs) = players::specs(rom);
    let players = data::build(rom, regions, &specs, Vec::new());
    let (regions, specs) = combat::combat();
    let combat = data::build(rom, regions, &specs, Vec::new());
    let (regions, specs) = rest::specs(rom);
    let rest = data::build(rom, regions, &specs, Vec::new());
    let (regions, specs) = combat::boxes(rom);
    let boxes = data::build(rom, regions, &specs, Vec::new());
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
        scenes,
        misc,
        texts: texts::texts(rom)?,
        screens: screens::screens(rom),
        player_tables: player_tables::player_tables(rom),
        players,
        combat,
        levels: levels::levels(rom)?,
        terrain: terrain::terrain(rom),
        scene_palettes: scene_palettes::scene_palettes(rom)?,
        palettes: palettes::palettes(rom),
        engine: engine::engine(rom),
        rest,
        boxes,
        overreads: overreads::overreads(rom),
    })
}
