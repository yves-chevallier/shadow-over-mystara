//! Tables of the ESHL, SOBJ, ITEM and BOX families: `data/objects.json` (`ddsom_data::objects`, docs/assets.md).
//!
//! The tables are declared in `ddsom_data::objects`; they are read like those of the effects (`effects.rs`): up to
//! their declared end, otherwise up to the next declared table or the next code executed by the original.

use std::io;
use std::path::Path;

use ddsom_data::json;
use ddsom_data::objects::ObjectTables;

use crate::effects::{code_starts, read_table};
use crate::rom::Program;

pub fn objects(rom: &Program) -> ObjectTables {
    let code = code_starts();
    ObjectTables::build(|spec| read_table(rom, spec, ObjectTables::SPECS, &code))
}

pub fn extract(roms: &Path, path: &Path) -> io::Result<()> {
    let rom = Program::load(roms)?;
    std::fs::create_dir_all(path.parent().unwrap())?;
    json::save(path, &objects(&rom).to_map(), 2)
}
