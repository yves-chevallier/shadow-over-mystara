//! Game data tables: `assets/data/players.json`, `combat.json`, `enemies.json`, `boxes.json`,
//! `scenes.json`. See docs/assets.md (« Tables de données ») for the meaning of each table.
//!
//! A table is a sequence of rows of the same format, stored as-is in ROM (big-endian):
//! `format` gives the type of each column, `fields` (optional) their name. A single-column table
//! writes its rows as numbers, the others as lists. `addr` columns (longs that designate a ROM
//! address) are written in hexadecimal, the others in decimal.
//!
//! Types: `u8`, `i8`, `u16`, `i16`, `u32`, `i32`, `addr`, and `x8`, `x16`, `x32` (bit masks, written
//! in hexadecimal like addresses); `T*n` repeats the type `T` `n` times.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::json::hex;

/// Type of a column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    /// Long: ROM address.
    Addr,
    /// Bit masks (written in hexadecimal).
    X8,
    X16,
    X32,
}

impl Kind {
    pub fn parse(s: &str) -> Option<Kind> {
        Some(match s {
            "u8" => Kind::U8,
            "i8" => Kind::I8,
            "u16" => Kind::U16,
            "i16" => Kind::I16,
            "u32" => Kind::U32,
            "i32" => Kind::I32,
            "addr" => Kind::Addr,
            "x8" => Kind::X8,
            "x16" => Kind::X16,
            "x32" => Kind::X32,
            _ => return None,
        })
    }

    /// Written in hexadecimal in the JSON.
    pub fn is_hex(self) -> bool {
        matches!(self, Kind::Addr | Kind::X8 | Kind::X16 | Kind::X32)
    }

    pub fn size(self) -> usize {
        match self {
            Kind::U8 | Kind::I8 | Kind::X8 => 1,
            Kind::U16 | Kind::I16 | Kind::X16 => 2,
            Kind::U32 | Kind::I32 | Kind::Addr | Kind::X32 => 4,
        }
    }

    /// Value read from `b` (`size()` bytes, big-endian).
    pub fn read(self, b: &[u8]) -> i64 {
        let u = b[..self.size()].iter().fold(0u64, |v, &x| v << 8 | x as u64);
        match self {
            Kind::I8 => u as u8 as i8 as i64,
            Kind::I16 => u as u16 as i16 as i64,
            Kind::I32 => u as u32 as i32 as i64,
            _ => u as i64,
        }
    }

    /// Bytes of `v` (the value must fit in the type).
    pub fn write(self, v: i64, out: &mut Vec<u8>) -> Result<(), String> {
        let (lo, hi) = match self {
            Kind::U8 | Kind::X8 => (0, 0xff),
            Kind::I8 => (-0x80, 0x7f),
            Kind::U16 | Kind::X16 => (0, 0xffff),
            Kind::I16 => (-0x8000, 0x7fff),
            Kind::U32 | Kind::Addr | Kind::X32 => (0, 0xffff_ffff),
            Kind::I32 => (-0x8000_0000, 0x7fff_ffff),
        };
        if v < lo || v > hi {
            return Err(format!("{v} out of range for {self:?}"));
        }
        let n = self.size();
        out.extend(&(v as u64).to_be_bytes()[8 - n..]);
        Ok(())
    }
}

/// List of the column types of a format (`"u8 i16*4 addr"`).
pub fn parse_format(format: &str) -> Result<Vec<Kind>, String> {
    let mut kinds = Vec::new();
    for tok in format.split_whitespace() {
        let (t, n) = match tok.split_once('*') {
            Some((t, n)) => (t, n.parse::<usize>().map_err(|_| format!("invalid repetition: {tok}"))?),
            None => (tok, 1),
        };
        let k = Kind::parse(t).ok_or_else(|| format!("unknown type: {t}"))?;
        kinds.extend(std::iter::repeat_n(k, n));
    }
    if kinds.is_empty() {
        return Err("empty format".into());
    }
    Ok(kinds)
}

/// Value of a cell: a number, or an address (or mask) written in hexadecimal.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(untagged)]
pub enum Cell {
    Int(i64),
    Addr(#[serde(with = "hex")] u32),
}

impl Cell {
    pub fn value(self) -> i64 {
        match self {
            Cell::Int(v) => v,
            Cell::Addr(a) => a as i64,
        }
    }
}

/// Row: a single value (single-column table) or the list of columns.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(untagged)]
pub enum Row {
    One(Cell),
    Many(Vec<Cell>),
}

impl Row {
    pub fn cells(&self) -> &[Cell] {
        match self {
            Row::One(c) => std::slice::from_ref(c),
            Row::Many(v) => v,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Table {
    /// Name (unique in the file); `role/character` for per-character tables.
    pub name: String,
    #[serde(with = "hex")]
    pub rom: u32,
    /// Read by the code with PC-relative addressing: decrypted program space.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub program: bool,
    pub format: String,
    /// Column names (optional).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<String>,
    pub rows: Vec<Row>,
}

impl Table {
    /// Table read from `bytes` (`rows` rows in format `format`); `addr` columns become addresses.
    pub fn read(name: &str, rom: u32, program: bool, format: &str, fields: &[&str], bytes: &[u8], rows: usize) -> Table {
        let kinds = parse_format(format).unwrap();
        let size: usize = kinds.iter().map(|k| k.size()).sum();
        assert!(fields.is_empty() || fields.len() == kinds.len(), "{name}: {} names for {} columns", fields.len(), kinds.len());
        let rows = (0..rows)
            .map(|r| {
                let mut at = r * size;
                let cells: Vec<Cell> = kinds
                    .iter()
                    .map(|&k| {
                        let v = k.read(&bytes[at..]);
                        at += k.size();
                        if k.is_hex() {
                            Cell::Addr(v as u32)
                        } else {
                            Cell::Int(v)
                        }
                    })
                    .collect();
                if cells.len() == 1 {
                    Row::One(cells[0])
                } else {
                    Row::Many(cells)
                }
            })
            .collect();
        Table {
            name: name.into(),
            rom,
            program,
            format: format.into(),
            fields: fields.iter().map(|s| s.to_string()).collect(),
            rows,
        }
    }

    pub fn kinds(&self) -> Vec<Kind> {
        parse_format(&self.format).unwrap()
    }

    /// Size of a row in bytes.
    pub fn row_size(&self) -> usize {
        self.kinds().iter().map(|k| k.size()).sum()
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Size of the table in bytes.
    pub fn size(&self) -> usize {
        self.row_size() * self.rows.len()
    }

    /// Address following the table.
    pub fn end(&self) -> u32 {
        self.rom + self.size() as u32
    }

    /// Column `col` of row `row`.
    pub fn get(&self, row: usize, col: usize) -> i64 {
        self.rows[row].cells()[col].value()
    }

    /// Value of a single-column table.
    pub fn value(&self, row: usize) -> i64 {
        self.get(row, 0)
    }

    /// Column named `field` of row `row`.
    pub fn field(&self, row: usize, field: &str) -> Option<i64> {
        let col = self.fields.iter().position(|f| f == field)?;
        Some(self.get(row, col))
    }

    /// Bytes of the table, as the ROM stores them.
    pub fn bytes(&self) -> Result<Vec<u8>, String> {
        let kinds = parse_format(&self.format)?;
        if !self.fields.is_empty() && self.fields.len() != kinds.len() {
            return Err(format!("{}: {} names for {} columns", self.name, self.fields.len(), kinds.len()));
        }
        let mut out = Vec::with_capacity(self.size());
        for (r, row) in self.rows.iter().enumerate() {
            let cells = row.cells();
            if cells.len() != kinds.len() {
                return Err(format!("{} row {r}: {} values for {} columns", self.name, cells.len(), kinds.len()));
            }
            for (&k, c) in kinds.iter().zip(cells) {
                k.write(c.value(), &mut out).map_err(|e| format!("{} row {r}: {e}", self.name))?;
            }
        }
        Ok(out)
    }
}

/// ROM range entirely covered by the tables of the file (checked by the tests).
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Region {
    pub name: String,
    #[serde(with = "hex")]
    pub start: u32,
    #[serde(with = "hex")]
    pub end: u32,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub program: bool,
}

/// Text (string of characters or tiles) split into lines for reading. `encoding`:
/// * absent: ASCII bytes, written like the texts of `texts.json` (`texts::encode`: ASCII as is,
///   other bytes `{xx}`);
/// * `"high"`: ASCII bytes + 0x80 (French and German dialog texts): `0x80` is the space,
///   `0xa1..0xfe` the matching ASCII character, any other byte `{xx}`;
/// * `"tiles"`: words, tiles of the large font (English dialog texts): `0x1800` is the space,
///   `0x18a1..0x18fe` the ASCII character, any other word `{xxxx}` (negative commands, final 0 included).
///
/// `{` and `}` are always written in hexadecimal.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TextBlock {
    pub name: String,
    #[serde(with = "hex")]
    pub rom: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    pub lines: Vec<String>,
}

/// Printable ASCII character written as is in the texts.
fn plain_char(c: u32) -> Option<char> {
    let c = char::from_u32(c)?;
    (c.is_ascii_graphic() && c != '{' && c != '}').then_some(c)
}

/// « high » or « tiles » text -> string (see `TextBlock`).
pub fn encode_units(units: &[u16], tiles: bool) -> String {
    let (space, base) = if tiles { (0x1800, 0x1880) } else { (0x80, 0x80) };
    let mut s = String::new();
    for &u in units {
        let u = u as u32;
        match u.checked_sub(base).and_then(plain_char) {
            _ if u == space => s.push(' '),
            Some(c) if u - base < 0x80 => s.push(c),
            _ if tiles => s.push_str(&format!("{{{u:04x}}}")),
            _ => s.push_str(&format!("{{{u:02x}}}")),
        }
    }
    s
}

/// String -> words or bytes (inverse of `encode_units`).
pub fn decode_units(s: &str, tiles: bool) -> Vec<u16> {
    let (space, base) = if tiles { (0x1800u16, 0x1880u16) } else { (0x80, 0x80) };
    let mut out = Vec::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        match c {
            '{' => {
                let h: String = chars.by_ref().take_while(|&c| c != '}').collect();
                out.push(u16::from_str_radix(&h, 16).unwrap_or(0));
            }
            ' ' => out.push(space),
            c => out.push(base + c as u16),
        }
    }
    out
}

impl TextBlock {
    pub fn bytes(&self) -> Vec<u8> {
        match self.encoding.as_deref() {
            Some("tiles") => {
                self.lines.iter().flat_map(|l| decode_units(l, true)).flat_map(|w| w.to_be_bytes()).collect()
            }
            Some("high") => self.lines.iter().flat_map(|l| decode_units(l, false)).map(|b| b as u8).collect(),
            _ => self.lines.iter().flat_map(|l| crate::texts::decode(l)).collect(),
        }
    }

    pub fn end(&self) -> u32 {
        self.rom + self.bytes().len() as u32
    }
}

#[derive(Serialize, Deserialize, Debug, Default)]
pub struct DataFile {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regions: Vec<Region>,
    pub tables: Vec<Table>,
    /// Byte texts (data space).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub texts: Vec<TextBlock>,
}

impl DataFile {
    pub fn load(path: &Path) -> io::Result<Self> {
        crate::json::load(path)
    }

    pub fn table(&self, name: &str) -> Option<&Table> {
        self.tables.iter().find(|t| t.name == name)
    }

    /// Table starting at address `rom` (data space).
    pub fn at(&self, rom: u32) -> Option<&Table> {
        self.tables.iter().find(|t| t.rom == rom && !t.program)
    }

    /// Table and row starting at address `rom` (data space).
    pub fn row_at(&self, rom: u32) -> Option<(&Table, usize)> {
        self.tables.iter().filter(|t| !t.program && rom >= t.rom && rom < t.end()).find_map(|t| {
            let d = (rom - t.rom) as usize;
            d.is_multiple_of(t.row_size()).then_some((t, d / t.row_size()))
        })
    }

    /// Table designated by entry `index` of the address table `pointers` (per-character tables...).
    pub fn via(&self, pointers: &str, index: usize) -> Option<&Table> {
        self.at(self.table(pointers)?.value(index) as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_and_lines() {
        assert_eq!(parse_format("u8 i16*2 addr").unwrap(), vec![Kind::U8, Kind::I16, Kind::I16, Kind::Addr]);
        assert!(parse_format("u7").is_err());
        let bytes = [0x01, 0xff, 0xfe, 0x00, 0x10, 0x00, 0x12, 0x34, 0x56];
        let t = Table::read("t", 0x1000, false, "u8 i16*2 addr", &["a", "b", "c", "d"], &bytes, 1);
        assert_eq!(t.get(0, 1), -2);
        assert_eq!(t.field(0, "d"), Some(0x0012_3456));
        assert_eq!(t.bytes().unwrap(), bytes);
        let json = serde_json::to_string(&t).unwrap();
        let back: Table = serde_json::from_str(&json).unwrap();
        assert_eq!(back.bytes().unwrap(), bytes);
    }

    #[test]
    fn texts_tiles_and_bytes() {
        let words = [0x18c8, 0x18e9, 0x1800, 0xfffe, 0x000c, 0x18fb, 0x0000];
        let s = encode_units(&words, true);
        assert_eq!(s, "Hi {fffe}{000c}{18fb}{0000}");
        assert_eq!(decode_units(&s, true), words);
        let bytes = [0xc1, 0x80, 0xfe, 0x0f, 0xfd, 0x00];
        let s = encode_units(&bytes, false);
        assert_eq!(s, "A ~{0f}{fd}{00}");
        assert_eq!(decode_units(&s, false), bytes);
    }
}
