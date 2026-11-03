//! Tables of the players: `data/player_tables.json` (`ddsom_data::player_tables`, docs/assets.md).
//!
//! The tables are declared in `ddsom_data::player_tables`; they are read like those of the effects (`effects.rs`): up
//! to their declared end, otherwise up to the next declared table or the next code executed by the original.

use std::io;
use std::path::Path;

use ddsom_data::json;
use ddsom_data::player_tables::PlayerTables;

use crate::effects::{code_starts, read_table};
use crate::rom::Program;

pub fn player_tables(rom: &Program) -> PlayerTables {
    let code = code_starts();
    PlayerTables::build(|spec| read_table(rom, spec, PlayerTables::SPECS, &code))
}

pub fn extract(roms: &Path, path: &Path) -> io::Result<()> {
    let rom = Program::load(roms)?;
    std::fs::create_dir_all(path.parent().unwrap())?;
    json::save(path, &player_tables(&rom).to_map(), 2)
}
