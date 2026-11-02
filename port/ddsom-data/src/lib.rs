//! Typed game data of ddsom: the tables of the original program, loaded once and read by the game through
//! [`data()`] (docs/fin.md, section 2 "Données").
//!
//! The data come from the assets produced by `extract` ([`GameData::load`]) or, in the mode with ROMs, from the ROMs
//! themselves through the same extraction in memory (`extract::to_data`): the game reads them the same way in both
//! modes. Formats: docs/assets.md.

pub mod data;
pub mod effects;
pub mod json;
pub mod levels;
pub mod pointers;
pub mod scene_palettes;
pub mod spawns;
pub mod speeds;
pub mod sprites;
pub mod tables;
pub mod terrain;
pub mod texts;

pub use data::DataFile;
pub use effects::EffectTables;
pub use levels::Levels;
pub use scene_palettes::ScenePalettes;
pub use spawns::Spawns;
pub use speeds::Speeds;
pub use sprites::{Anims, Frames};
pub use tables::SpriteTables;
pub use terrain::Terrain;
pub use texts::Texts;

use std::io;
use std::path::Path;
use std::sync::OnceLock;

/// Address of a table or an entry in the program ROM. The original keeps such addresses in its state (next
/// animation entry, speed table, sprite frame...) and `validate` compares that state with the original: they stay in
/// the state, but as keys of the typed tables, never as places to read bytes from.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct RomAddr(pub u32);

impl RomAddr {
    /// The address `n` bytes further (wrapping like the 68000 address arithmetic).
    pub fn offset(self, n: i32) -> RomAddr {
        RomAddr(self.0.wrapping_add(n as u32))
    }
}

/// All the data the game reads (one field per family of tables).
pub struct GameData {
    pub frames: Frames,
    pub anims: Anims,
    pub speeds: Speeds,
    pub effects: EffectTables,
}

impl GameData {
    /// Files of the assets folder that make up the game data.
    pub const FILES: [&'static str; 4] = [
        "sprites/frames.json",
        "sprites/anims.json",
        "data/speeds.json",
        "data/effects.json",
    ];

    /// Loads the game data from an assets folder (written by `extract`).
    pub fn load(dir: &Path) -> io::Result<Self> {
        Ok(GameData {
            frames: Frames::load(&dir.join(Self::FILES[0]))?,
            anims: Anims::load(&dir.join(Self::FILES[1]))?,
            speeds: Speeds::load(&dir.join(Self::FILES[2]))?,
            effects: EffectTables::load(&dir.join(Self::FILES[3]))?,
        })
    }
}

static DATA: OnceLock<GameData> = OnceLock::new();

/// Makes `d` the game data of the process, unless data are already installed (the same game data serve every
/// machine of a process; the first installed stay). Returns the installed data.
pub fn install(d: GameData) -> &'static GameData {
    DATA.get_or_init(|| d)
}

/// Installs the data that `make` builds, if none are installed yet.
pub fn install_with(make: impl FnOnce() -> GameData) -> &'static GameData {
    DATA.get_or_init(make)
}

/// The game data. Panics if none are installed: the application (`ddsom`) and the tools install them before the
/// game starts.
pub fn data() -> &'static GameData {
    DATA.get().expect("game data not loaded: install `GameData::load(assets)` or `extract::to_data(roms)` first")
}
