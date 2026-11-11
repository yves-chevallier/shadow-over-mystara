//! Named ROM tables of the typed data files, and the engine's data (docs/fin.md, L12).
//!
//! A [`Table`] is one table of a `data/*.json` file: its name, its address and space in the original program (data,
//! or program for the tables that the original reads PC-relative) and its bytes. A [`Tables`] indexes the tables of a
//! file by name and by address: the game reads a table by name at a byte offset, like the original (`table + 4 ×
//! code`), and the data that a table or the state designates by an address through [`Tables::byte`],
//! [`Tables::word`], [`Tables::long`] (data space) and their program-space twins. An address of the RAM goes to the
//! bus: some routines read either, as the original does.
//!
//! [`engine`] gathers the data of the engine (AI helpers, cameras, video, palettes, sprites, creators): the tables of
//! `data/engine.json` and of the files whose tables the engine also reads (`enemies.json`: hit points, immunities,
//! approach points; `scenes.json`: per-scene camera tables, player markers; `rest.json`: aura palettes) by name, and,
//! by address, those tables plus the scenery maps (`levels.json`), the fixed terrain maps and terrain types, the scene
//! palette tables, the palette library, the collision boxes (`sprites/tables.json`) and the texts (`texts.json`, the
//! dialogs and the texts of the other files). A read outside every table is a read of the residue (`Missing`):
//! counted, fatal with `DDSOM_STRICT=1`.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::world::World;

use crate::world::data::{data, Missing};
use ddsom_data::DataFile;

/// One table: its bytes as the ROM stores them.
pub struct Table {
    pub name: &'static str,
    pub rom: u32,
    pub program: bool,
    bytes: Vec<u8>,
}

impl Table {
    fn missing(&self) -> Missing {
        Missing { table: self.name, key: self.rom }
    }

    /// Byte at offset `off` from the start of the table.
    pub fn at8(&self, b: &mut World, off: u32) -> u8 {
        match self.bytes.get(off as usize) {
            Some(&v) => v,
            None => self.missing().byte_in(b, self.rom.wrapping_add(off), self.program),
        }
    }

    /// Word (big-endian) at offset `off`.
    pub fn at16(&self, b: &mut World, off: u32) -> u16 {
        match self.slice(off, 2) {
            Some(s) => u16::from_be_bytes([s[0], s[1]]),
            None => self.missing().word_in(b, self.rom.wrapping_add(off), self.program),
        }
    }

    /// Long (big-endian) at offset `off`.
    pub fn at32(&self, b: &mut World, off: u32) -> u32 {
        match self.slice(off, 4) {
            Some(s) => u32::from_be_bytes([s[0], s[1], s[2], s[3]]),
            None => self.missing().long_in(b, self.rom.wrapping_add(off), self.program),
        }
    }

    /// Long `i` (offset `4 i`).
    pub fn nth32(&self, b: &mut World, i: u32) -> u32 {
        self.at32(b, i.wrapping_mul(4))
    }

    /// Word `i` (offset `2 i`).
    pub fn nth16(&self, b: &mut World, i: u32) -> u16 {
        self.at16(b, i.wrapping_mul(2))
    }

    fn slice(&self, off: u32, n: usize) -> Option<&[u8]> {
        let start = off as usize;
        self.bytes.get(start..start.checked_add(n)?)
    }

    /// The table holds the address `a` (of its space).
    pub fn holds(&self, a: u32) -> bool {
        (a.wrapping_sub(self.rom) as usize) < self.bytes.len()
    }
}

/// Runs of contiguous bytes (start address, bytes) sorted by address, for one space.
#[derive(Default)]
struct Runs(Vec<(u32, Vec<u8>)>);

impl Runs {
    /// Merges the zones that touch or overlap into runs (their common bytes are the same ROM bytes).
    fn new(mut zones: Vec<(u32, Vec<u8>)>) -> Runs {
        zones.sort_by_key(|&(rom, _)| rom);
        let mut out: Vec<(u32, Vec<u8>)> = Vec::new();
        for (rom, bytes) in zones {
            match out.last_mut() {
                Some((start, run)) if rom <= *start + run.len() as u32 => {
                    let skip = (*start + run.len() as u32 - rom) as usize;
                    run.extend(bytes.iter().skip(skip));
                }
                _ => out.push((rom, bytes)),
            }
        }
        Runs(out)
    }

    /// `n` bytes from `a` (big-endian), if one run holds them all.
    fn get(&self, a: u32, n: usize) -> Option<u32> {
        let i = self.0.partition_point(|&(rom, _)| rom <= a).checked_sub(1)?;
        let (rom, run) = &self.0[i];
        let off = (a - rom) as usize;
        let s = run.get(off..off + n)?;
        Some(s.iter().fold(0u32, |v, &x| v << 8 | x as u32))
    }
}

/// The program ROM ends here: above are the RAM and the hardware.
const ROM_END: u32 = 0x40_0000;

/// Tables by name and bytes by address (see the module).
pub struct Tables {
    /// Name of the residue key of a read by address outside the tables.
    label: &'static str,
    by_name: HashMap<&'static str, Table>,
    /// Bytes by address: data space, program space.
    runs: [Runs; 2],
}

impl Tables {
    /// The tables of `files`; `label` names the residue key of a read outside them. A name declared by two files is
    /// an error of the extraction.
    pub fn new(label: &'static str, files: &[&DataFile]) -> Tables {
        let mut by_name = HashMap::new();
        for t in files.iter().flat_map(|f| &f.tables) {
            let name: &'static str = Box::leak(t.name.clone().into_boxed_str());
            let bytes = t.bytes().unwrap_or_else(|e| panic!("{label} tables: {e}"));
            let old = by_name.insert(name, Table { name, rom: t.rom, program: t.program, bytes });
            assert!(old.is_none(), "{label} tables: `{name}` declared twice");
        }
        let mut zones: [Vec<(u32, Vec<u8>)>; 2] = [Vec::new(), Vec::new()];
        for t in by_name.values() {
            zones[t.program as usize].push((t.rom, t.bytes.clone()));
        }
        Tables { label, by_name, runs: zones.map(Runs::new) }
    }

    /// Adds bytes readable by address only (data of another file that the readers of these tables reach).
    fn with_zones(mut self, extra: [Vec<(u32, Vec<u8>)>; 2]) -> Tables {
        for (space, zones) in extra.into_iter().enumerate() {
            let mut all = std::mem::take(&mut self.runs[space].0);
            all.extend(zones);
            self.runs[space] = Runs::new(all);
        }
        self
    }

    /// The table named `name`. Panics if it does not exist: the names are those of the extraction.
    pub fn get(&self, name: &str) -> &Table {
        self.by_name.get(name).unwrap_or_else(|| panic!("{}: no table `{name}`", self.label))
    }

    /// The tables, by name.
    pub fn iter(&self) -> impl Iterator<Item = &Table> {
        self.by_name.values()
    }

    fn missing(&self, a: u32) -> Missing {
        Missing { table: self.label, key: a }
    }

    /// Byte of the data space at `a` (ROM table, or RAM).
    pub fn byte(&self, b: &mut World, a: u32) -> u8 {
        if a & 0xff_ffff >= ROM_END {
            return crate::world::mem::rd8(b, a);
        }
        self.runs[0].get(a, 1).map_or_else(|| self.missing(a).read8(b, a), |v| v as u8)
    }

    /// Word of the data space at `a`.
    pub fn word(&self, b: &mut World, a: u32) -> u16 {
        if a & 0xff_ffff >= ROM_END {
            return crate::world::mem::rd16(b, a);
        }
        self.runs[0].get(a, 2).map_or_else(|| self.missing(a).read16(b, a), |v| v as u16)
    }

    /// Long of the data space at `a`.
    pub fn long(&self, b: &mut World, a: u32) -> u32 {
        if a & 0xff_ffff >= ROM_END {
            return crate::world::mem::rd32(b, a);
        }
        self.runs[0].get(a, 4).unwrap_or_else(|| self.missing(a).read32(b, a))
    }

    /// Byte of the program space at `a` (table read PC-relative).
    pub fn pc_byte(&self, b: &mut World, a: u32) -> u8 {
        self.runs[1].get(a, 1).map_or_else(|| self.missing(a).pc_byte(b, a), |v| v as u8)
    }

    /// Word of the program space at `a`.
    pub fn pc_word(&self, b: &mut World, a: u32) -> u16 {
        self.runs[1].get(a, 2).map_or_else(|| self.missing(a).pc_word(b, a), |v| v as u16)
    }

    /// Long of the program space at `a`.
    pub fn pc_long(&self, b: &mut World, a: u32) -> u32 {
        self.runs[1].get(a, 4).unwrap_or_else(|| self.missing(a).pc_long(b, a))
    }
}

/// Words from `rom` as big-endian bytes.
fn words(rom: u32, words: impl IntoIterator<Item = u16>) -> (u32, Vec<u8>) {
    (rom, words.into_iter().flat_map(u16::to_be_bytes).collect())
}

/// The bytes by address of the data that the engine reads besides `data/engine.json` (see the module).
fn engine_zones() -> [Vec<(u32, Vec<u8>)>; 2] {
    let d = data();
    let mut zones: [Vec<(u32, Vec<u8>)>; 2] = [Vec::new(), Vec::new()];
    for l in &d.levels.layers {
        zones[0].push((l.banks_rom, l.banks.iter().flat_map(|b| b.rom.to_be_bytes()).collect()));
        for bank in &l.banks {
            zones[0].push(words(bank.rom, bank.blocks.iter().flatten().copied()));
        }
        zones[0].push(words(l.strips_rom, l.strips.iter().flatten().copied()));
        for t in &l.tables {
            zones[0].push(words(t.rom, std::iter::once(t.width() as u16).chain(t.rows.iter().flatten().copied())));
        }
        let pointers = l.scenes.iter().flat_map(|&t| l.tables[t].pointer().to_be_bytes()).collect();
        zones[1].push((l.scenes_rom, pointers));
    }
    let s1 = &d.levels.scroll1;
    let mut grids: Vec<u16> = s1.grids.iter().flatten().copied().collect();
    grids.resize((s1.blocks_rom - s1.grids_rom) as usize / 2, 0);
    zones[0].push(words(s1.grids_rom, grids));
    zones[0].push(words(s1.blocks_rom, s1.blocks.iter().flatten().copied()));
    for m in &d.terrain.maps {
        zones[0].push(words(m.rom, m.columns.iter().flatten().copied()));
    }
    for k in &d.terrain.kinds {
        zones[0].push(words(k.rom, k.kinds.iter().flat_map(|e| e.words())));
    }
    let p = &d.scene_palettes;
    let stage_words = p.stages.iter().flatten().map(|&v| ddsom_data::ScenePalettes::stage_word(v));
    zones[0].push(words(p.stages_rom, stage_words));
    for t in [&p.extra, &p.sets].into_iter().chain(p.pages.iter()) {
        zones[t.program as usize].push((t.rom, t.entries.iter().flat_map(|a| a.to_be_bytes()).collect()));
    }
    zones[0].push((ddsom_data::Palettes::ROM, d.palettes.bytes()));
    for (rom, program, bytes) in d.texts.regions() {
        zones[program as usize].push((rom, bytes));
    }
    for f in [&d.dialogs, &d.scenes, &d.misc, &d.screens, &d.set] {
        for text in &f.texts {
            zones[0].push((text.rom, text.bytes()));
        }
    }
    for bt in &d.sprite_tables.box_tables {
        zones[0].push((bt.rom, bt.sets.iter().flat_map(|a| a.to_be_bytes()).collect()));
    }
    for blk in &d.sprite_tables.box_blocks {
        let mut bytes: Vec<u8> = blk.boxes.iter().flat_map(|x| x.bytes()).collect();
        bytes.extend(&blk.tail);
        zones[0].push((blk.rom, bytes));
    }
    zones
}

/// The tables of the crates (`data/boxes.json`): sheets per kind (`fiches`, the sheets by address), loot (`loot`),
/// wear of the equipment (`wear_chances`).
pub fn boxes() -> &'static Tables {
    static BOXES: OnceLock<Tables> = OnceLock::new();
    BOXES.get_or_init(|| Tables::new("boxes", &[&data().boxes]))
}

/// The table `name` of the engine's data ([`engine`]), looked up once per call site.
macro_rules! engine_table {
    ($name:expr) => {{
        static TABLE: std::sync::OnceLock<&'static $crate::world::tables::Table> = std::sync::OnceLock::new();
        *TABLE.get_or_init(|| $crate::world::tables::engine().get($name))
    }};
}
pub(crate) use engine_table;

/// The engine's data (see the module).
pub fn engine() -> &'static Tables {
    static ENGINE: OnceLock<Tables> = OnceLock::new();
    ENGINE.get_or_init(|| {
        let d = data();
        Tables::new("engine", &[&d.engine, &d.enemies, &d.scenes, &d.rest]).with_zones(engine_zones())
    })
}
