//! The tables of the players (docs/fin.md, L10), read like the original: at a byte offset from the start of a table
//! ([`Read`], shared with the effects).
//!
//! - [`player_data`]: the tables of `data/player_tables.json` (`ddsom_data::player_tables`), by name;
//! - [`players`], [`combat`]: the tables of `data/players.json` and `data/combat.json`, by their name in the file;
//! - [`pc8`]... [`long`]: a table entry designated by its address, which the original computes from a table address
//!   passed around (step tables of the reactions and cinematics, rows chosen by a caller) or reads from another table
//!   (pointers by character, by class...). `pc*` read in program space (PC-relative tables), the others in data
//!   space. The address is looked up in the tables above, then in the speed tables and the collision boxes.
//!
//! A read past the extracted bytes goes to the residue (counted, see `crate::data`).

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::world::World;

pub(crate) use crate::families::effects::tables::Read;
pub(crate) use ddsom_data::effects::Table;
pub(crate) use ddsom_data::player_tables::PlayerTables;

use crate::data::{data, Missing, RomAddr};

/// Word and long reads of a table as the 68000 bus does them: the address is rounded down to an even address (the
/// bus ignores bit 0 for a word; `rd16`, `fetch16`). The original never reads a word at an odd address, but the
/// tools (fuzz) and some glitches do.
pub(crate) trait Aligned: Read {
    fn rom(&self) -> u32;
    fn program(&self) -> bool;

    /// Byte at offset `off`, read by address: an offset past the table (or negative) reads the tables that follow
    /// (or precede) it, like the original.
    fn byte_past(&self, b: &mut World, off: u32) -> u8 {
        let a = self.rom().wrapping_add(off);
        if self.program() { pc8(b, a) } else { byte(b, a) }
    }

    /// Word at offset `off`, read by address (see `byte_past`).
    fn word_past(&self, b: &mut World, off: u32) -> u16 {
        let a = self.rom().wrapping_add(off);
        if self.program() { pc16(b, a) } else { word(b, a) }
    }

    /// Long at offset `off`, read by address (see `byte_past`).
    fn long_past(&self, b: &mut World, off: u32) -> u32 {
        let a = self.rom().wrapping_add(off);
        if self.program() { pc32(b, a) } else { long(b, a) }
    }

    /// Word at offset `off`, the address rounded down to even.
    fn word_at(&self, b: &mut World, off: u32) -> u16 {
        self.at16(b, off.wrapping_sub(self.rom().wrapping_add(off) & 1))
    }

    /// Long at offset `off`, the address rounded down to even.
    fn long_at(&self, b: &mut World, off: u32) -> u32 {
        self.at32(b, off.wrapping_sub(self.rom().wrapping_add(off) & 1))
    }
}

impl Aligned for Table {
    fn rom(&self) -> u32 {
        self.rom
    }
    fn program(&self) -> bool {
        self.program
    }
}

/// The tables of `data/player_tables.json`.
pub(crate) fn player_data() -> &'static PlayerTables {
    &data().player_tables
}

struct Index {
    /// Tables of `players.json` and `combat.json` by name.
    by_name: &'static HashMap<&'static str, Table>,
    /// Every table, by increasing address (both spaces).
    by_address: Vec<&'static Table>,
}

/// A table of a `DataFile` as bytes (elements of one byte).
fn bytes_table(t: &ddsom_data::data::Table) -> Table {
    let name: &'static str = Box::leak(t.name.clone().into_boxed_str());
    let values = t.bytes().unwrap_or_else(|e| panic!("player tables: {e}")).into_iter().map(u32::from).collect();
    Table { rom: t.rom, program: t.program, size: 1, values, name }
}

fn index() -> &'static Index {
    static INDEX: OnceLock<Index> = OnceLock::new();
    INDEX.get_or_init(|| {
        let d = data();
        let mut by_name = HashMap::new();
        for t in d.players.tables.iter().chain(&d.combat.tables) {
            by_name.insert(Box::leak(t.name.clone().into_boxed_str()) as &str, bytes_table(t));
        }
        let by_name: &'static HashMap<&'static str, Table> = Box::leak(Box::new(by_name));
        let mut by_address: Vec<&'static Table> = d.player_tables.tables();
        by_address.extend(by_name.values());
        by_address.sort_by_key(|t| (t.rom, std::cmp::Reverse(t.len())));
        Index { by_name, by_address }
    })
}

/// The table `name` of `data/players.json`. Panics if it does not exist: the names are those of the extraction.
pub(crate) fn players(name: &str) -> &'static Table {
    index().by_name.get(name).unwrap_or_else(|| panic!("data/players.json: no table `{name}`"))
}

/// The table `name` of `data/combat.json`.
pub(crate) fn combat(name: &str) -> &'static Table {
    index().by_name.get(name).unwrap_or_else(|| panic!("data/combat.json: no table `{name}`"))
}

/// The table of the space `program` that holds the address `a`, and the offset of `a` in it.
fn holding(a: u32, program: bool) -> Option<(&'static Table, u32)> {
    let i = index();
    let end = i.by_address.partition_point(|t| t.rom <= a);
    i.by_address[..end].iter().rev().find(|t| t.program == program && t.contains(a)).map(|t| (*t, a - t.rom))
}

fn residue(a: u32, program: bool) -> Missing {
    Missing { table: if program { "players by address (program)" } else { "players by address" }, key: a }
}

/// Byte of the collision boxes (`sprites/tables.json`) at the address `a`, if a box block holds it.
fn box_byte(a: u32) -> Option<u8> {
    let blocks = &data().sprite_tables.box_blocks;
    let i = blocks.partition_point(|k| k.rom <= a).checked_sub(1)?;
    let k = &blocks[i];
    let off = (a - k.rom) as usize;
    k.boxes.get(off / 8).map(|h| h.bytes()[off % 8]).or_else(|| k.tail.get(off.checked_sub(8 * k.boxes.len())?).copied())
}

/// The six useful bytes of the collision box at the address `a` (offset and half-size per axis), as [`byte`] reads
/// them one by one: a box of a box block is copied at once (the collision tests read every box of every pair).
pub(crate) fn box_bytes(b: &mut World, a: u32) -> [u8; 6] {
    if a & 0xff_ffff < RAM {
        let blocks = &data().sprite_tables.box_blocks;
        if let Some(k) = blocks.partition_point(|k| k.rom <= a).checked_sub(1).map(|i| &blocks[i]) {
            let off = (a - k.rom) as usize;
            if off.is_multiple_of(8) {
                if let Some(h) = k.boxes.get(off / 8) {
                    let x = h.bytes();
                    return [x[0], x[1], x[2], x[3], x[4], x[5]];
                }
            }
        }
    }
    std::array::from_fn(|i| byte(b, a.wrapping_add(i as u32)))
}

/// Byte at the address `a` of a table read PC-relative.
pub(crate) fn pc8(b: &mut World, a: u32) -> u8 {
    match holding(a, true) {
        Some((t, off)) => t.at8(b, off),
        None => residue(a, true).byte_in(b, a, true),
    }
}

/// Word at the address `a` of a table read PC-relative.
pub(crate) fn pc16(b: &mut World, a: u32) -> u16 {
    let a = a & !1;
    match holding(a, true) {
        Some((t, off)) => t.at16(b, off),
        None => residue(a, true).word_in(b, a, true),
    }
}

/// Long at the address `a` of a table read PC-relative.
pub(crate) fn pc32(b: &mut World, a: u32) -> u32 {
    let a = a & !1;
    match holding(a, true) {
        Some((t, off)) => t.at32(b, off),
        None => residue(a, true).long_in(b, a, true),
    }
}

/// First address of the work RAM: an address the original computes there is read on the bus (a table row may be
/// designated in RAM, like the player's equipment).
const RAM: u32 = 0xff_0000;

/// Byte at the address `a` of a table (data space).
pub(crate) fn byte(b: &mut World, a: u32) -> u8 {
    if a & 0xff_ffff >= RAM {
        return crate::mem::rd8(b, a);
    }
    match holding(a, false) {
        Some((t, off)) => t.at8(b, off),
        None => match box_byte(a).or_else(|| speed_byte(a)) {
            Some(v) => v,
            None => residue(a, false).read8(b, a),
        },
    }
}

/// Word at the address `a` of a table (data space).
pub(crate) fn word(b: &mut World, a: u32) -> u16 {
    let a = a & !1;
    if a & 0xff_ffff >= RAM {
        return crate::mem::rd16(b, a);
    }
    match holding(a, false) {
        Some((t, off)) => t.at16(b, off),
        None => match data().speeds.at(RomAddr(a)) {
            Some(v) => v,
            None => match (box_byte(a), box_byte(a.wrapping_add(1))) {
                (Some(h), Some(l)) => u16::from_be_bytes([h, l]),
                _ => residue(a, false).read16(b, a),
            },
        },
    }
}

/// Long at the address `a` of a table (data space).
pub(crate) fn long(b: &mut World, a: u32) -> u32 {
    let a = a & !1;
    if a & 0xff_ffff >= RAM {
        return crate::mem::rd32(b, a);
    }
    match holding(a, false) {
        Some((t, off)) => t.at32(b, off),
        None => match box_set(a) {
            Some(v) => v,
            None => (word(b, a) as u32) << 16 | word(b, a.wrapping_add(2)) as u32,
        },
    }
}

/// Entry of a table of box sets (`sprites/tables.json`, `box_tables`: one long per set) at the address `a`.
fn box_set(a: u32) -> Option<u32> {
    let t = data().sprite_tables.box_tables.iter().find(|t| a >= t.rom && a < t.rom + 4 * t.sets.len() as u32)?;
    let off = a - t.rom;
    off.is_multiple_of(4).then(|| t.sets[off as usize / 4])
}

/// Byte of a speed table (`data/speeds.json`) at the address `a`.
fn speed_byte(a: u32) -> Option<u8> {
    let w = data().speeds.at(RomAddr(a & !1))?;
    Some(if a & 1 == 0 { (w >> 8) as u8 } else { w as u8 })
}
