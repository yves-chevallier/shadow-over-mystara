//! Tables de données du jeu (joueurs, combat, ennemis, caisses, scènes) : `assets/data/*.json`
//! (`ddsom_assets::data`, docs/assets.md « Tables de données »).
//!
//! Chaque table est décrite par une `Spec` (nom, adresse, espace, format, nombre de lignes). Les tailles
//! viennent de l'écart entre tables quand elles se suivent (régions), sinon du code qui les lit.

use std::io;
use std::path::Path;

use ddsom_assets::data::{parse_format, DataFile, Region, Table};
use ddsom_assets::json;

use crate::rom::Program;

/// Description d'une table à extraire.
pub struct Spec {
    pub name: String,
    pub rom: u32,
    pub program: bool,
    pub format: &'static str,
    pub fields: &'static [&'static str],
    pub rows: usize,
}

/// Table de l'espace données.
pub fn data(name: &str, rom: u32, format: &'static str, fields: &'static [&'static str], rows: usize) -> Spec {
    Spec { name: name.into(), rom, program: false, format, fields, rows }
}

/// Table de l'espace programme (lue en relatif au PC).
pub fn prog(name: &str, rom: u32, format: &'static str, fields: &'static [&'static str], rows: usize) -> Spec {
    Spec { name: name.into(), rom, program: true, format, fields, rows }
}

pub fn row_size(format: &str) -> usize {
    parse_format(format).unwrap().iter().map(|k| k.size()).sum()
}

/// Table de `rows` lignes qui va de `rom` à `end`.
pub fn span(name: &str, rom: u32, end: u32, program: bool, format: &'static str, fields: &'static [&'static str]) -> Spec {
    let size = row_size(format);
    let len = (end - rom) as usize;
    assert!(len % size == 0, "{name} : {len:#x} octets pour des lignes de {size}");
    Spec { name: name.into(), rom, program, format, fields, rows: len / size }
}

pub fn read(rom: &Program, s: &Spec) -> Table {
    let n = s.rows * row_size(s.format);
    let bytes: Vec<u8> =
        (0..n as u32).map(|i| if s.program { rom.prog8(s.rom + i) } else { rom.rd8(s.rom + i) }).collect();
    Table::read(&s.name, s.rom, s.program, s.format, s.fields, &bytes, s.rows)
}

pub fn region(name: &str, start: u32, end: u32) -> Region {
    Region { name: name.into(), start, end, program: false }
}

pub fn save(rom: &Program, path: &Path, regions: Vec<Region>, specs: &[Spec]) -> io::Result<()> {
    let tables: Vec<Table> = specs.iter().map(|s| read(rom, s)).collect();
    for r in &regions {
        let mut inside: Vec<&Table> =
            tables.iter().filter(|t| t.program == r.program && t.rom >= r.start && t.rom < r.end).collect();
        inside.sort_by_key(|t| t.rom);
        let mut at = r.start;
        for t in inside {
            assert_eq!(t.rom, at, "région {} : trou ou chevauchement avant {}", r.name, t.name);
            at = t.end();
        }
        assert_eq!(at, r.end, "région {} : fin", r.name);
    }
    eprintln!("{} : {} tables", path.display(), tables.len());
    std::fs::create_dir_all(path.parent().unwrap())?;
    json::save(path, &DataFile { regions, tables }, 4)
}

pub fn extract(roms: &Path, dir: &Path) -> io::Result<()> {
    let rom = Program::load(roms)?;
    let (regions, specs) = crate::players::specs(&rom);
    save(&rom, &dir.join("players.json"), regions, &specs)?;
    let (regions, specs) = crate::combat::combat();
    save(&rom, &dir.join("combat.json"), regions, &specs)?;
    let (regions, specs) = crate::combat::enemies(&rom);
    save(&rom, &dir.join("enemies.json"), regions, &specs)?;
    let (regions, specs) = crate::combat::boxes(&rom);
    save(&rom, &dir.join("boxes.json"), regions, &specs)?;
    let (regions, specs) = crate::scenes::specs(&rom);
    save(&rom, &dir.join("scenes.json"), regions, &specs)?;
    Ok(())
}
