//! Tables de données du jeu (joueurs, combat, ennemis, caisses, scènes) : `assets/data/*.json`
//! (`ddsom_assets::data`, docs/assets.md « Tables de données »).
//!
//! Chaque table est décrite par une `Spec` (nom, adresse, espace, format, nombre de lignes). Les tailles
//! viennent de l'écart entre tables quand elles se suivent (régions), sinon du code qui les lit.

use std::io;
use std::path::Path;

use ddsom_assets::data::{parse_format, DataFile, Region, Table, TextBlock};
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

/// Texte d'octets de `start` à `end`, coupé en lignes après chaque octet `cut`.
pub fn text(rom: &Program, name: &str, start: u32, end: u32, cut: u8) -> TextBlock {
    let bytes: Vec<u8> = (start..end).map(|a| rom.rd8(a)).collect();
    let lines = bytes.split_inclusive(|&b| b == cut).map(ddsom_assets::texts::encode).collect();
    TextBlock { name: name.into(), rom: start, encoding: None, lines }
}

pub fn save(rom: &Program, path: &Path, regions: Vec<Region>, specs: &[Spec], texts: Vec<TextBlock>) -> io::Result<()> {
    let tables: Vec<Table> = specs.iter().map(|s| read(rom, s)).collect();
    for r in &regions {
        let mut inside: Vec<(u32, u32, &str)> = tables
            .iter()
            .filter(|t| t.program == r.program)
            .map(|t| (t.rom, t.end(), t.name.as_str()))
            .chain(texts.iter().filter(|_| !r.program).map(|t| (t.rom, t.end(), t.name.as_str())))
            .filter(|&(start, _, _)| start >= r.start && start < r.end)
            .collect();
        inside.sort();
        let mut at = r.start;
        for (start, end, name) in inside {
            assert_eq!(start, at, "région {} : trou ou chevauchement avant {name}", r.name);
            at = end;
        }
        assert_eq!(at, r.end, "région {} : fin", r.name);
    }
    eprintln!("{} : {} tables, {} textes", path.display(), tables.len(), texts.len());
    std::fs::create_dir_all(path.parent().unwrap())?;
    json::save(path, &DataFile { regions, tables, texts }, 4)
}

pub fn extract(roms: &Path, dir: &Path) -> io::Result<()> {
    let rom = Program::load(roms)?;
    let (regions, specs) = crate::players::specs(&rom);
    save(&rom, &dir.join("players.json"), regions, &specs, Vec::new())?;
    let (regions, specs) = crate::combat::combat();
    save(&rom, &dir.join("combat.json"), regions, &specs, Vec::new())?;
    let (regions, specs) = crate::combat::enemies(&rom);
    save(&rom, &dir.join("enemies.json"), regions, &specs, Vec::new())?;
    let (regions, specs) = crate::combat::boxes(&rom);
    save(&rom, &dir.join("boxes.json"), regions, &specs, Vec::new())?;
    let (regions, specs) = crate::scenes::specs(&rom);
    save(&rom, &dir.join("scenes.json"), regions, &specs, Vec::new())?;
    let (regions, specs, texts) = crate::misc::specs(&rom);
    save(&rom, &dir.join("misc.json"), regions, &specs, texts)?;
    let (regions, specs, texts) = crate::dialogs::specs(&rom);
    save(&rom, &dir.join("dialogs.json"), regions, &specs, texts)?;
    Ok(())
}
