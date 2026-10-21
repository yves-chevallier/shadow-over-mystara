//! Tables de données du jeu : `assets/data/players.json`, `combat.json`, `enemies.json`, `boxes.json`,
//! `scenes.json`. Voir docs/assets.md (« Tables de données ») pour le sens de chaque table.
//!
//! Une table est une suite de lignes de même format, rangées telles quelles en ROM (grand-boutiste) :
//! `format` donne le type de chaque colonne, `fields` (facultatif) leur nom. Une table d'une seule
//! colonne écrit ses lignes comme des nombres, les autres comme des listes. Les colonnes `addr` (longs qui
//! désignent une adresse de la ROM) s'écrivent en hexadécimal, les autres en décimal.
//!
//! Types : `u8`, `i8`, `u16`, `i16`, `u32`, `i32`, `addr`, et `x8`, `x16`, `x32` (masques de bits, écrits
//! en hexadécimal comme les adresses) ; `T*n` répète `n` fois le type `T`.

use std::io;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::json::hex;

/// Type d'une colonne.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    /// Long : adresse de la ROM.
    Addr,
    /// Masques de bits (écrits en hexadécimal).
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

    /// Écrit en hexadécimal dans le JSON.
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

    /// Valeur lue dans `b` (`size()` octets, grand-boutiste).
    pub fn read(self, b: &[u8]) -> i64 {
        let u = b[..self.size()].iter().fold(0u64, |v, &x| v << 8 | x as u64);
        match self {
            Kind::I8 => u as u8 as i8 as i64,
            Kind::I16 => u as u16 as i16 as i64,
            Kind::I32 => u as u32 as i32 as i64,
            _ => u as i64,
        }
    }

    /// Octets de `v` (la valeur doit tenir dans le type).
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
            return Err(format!("{v} hors de {self:?}"));
        }
        let n = self.size();
        out.extend(&(v as u64).to_be_bytes()[8 - n..]);
        Ok(())
    }
}

/// Liste des types des colonnes d'un format (`"u8 i16*4 addr"`).
pub fn parse_format(format: &str) -> Result<Vec<Kind>, String> {
    let mut kinds = Vec::new();
    for tok in format.split_whitespace() {
        let (t, n) = match tok.split_once('*') {
            Some((t, n)) => (t, n.parse::<usize>().map_err(|_| format!("répétition invalide : {tok}"))?),
            None => (tok, 1),
        };
        let k = Kind::parse(t).ok_or_else(|| format!("type inconnu : {t}"))?;
        kinds.extend(std::iter::repeat(k).take(n));
    }
    if kinds.is_empty() {
        return Err("format vide".into());
    }
    Ok(kinds)
}

/// Valeur d'une case : nombre, ou adresse (ou masque) écrite en hexadécimal.
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

/// Ligne : une valeur (table d'une colonne) ou la liste des colonnes.
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
    /// Nom (unique dans le fichier) ; `rôle/personnage` pour les tables par personnage.
    pub name: String,
    #[serde(with = "hex")]
    pub rom: u32,
    /// Lue par le code en adressage relatif au PC : espace programme déchiffré.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub program: bool,
    pub format: String,
    /// Noms des colonnes (facultatif).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<String>,
    pub rows: Vec<Row>,
}

impl Table {
    /// Table lue dans `bytes` (`rows` lignes au format `format`) ; les colonnes `addr` deviennent des adresses.
    pub fn read(name: &str, rom: u32, program: bool, format: &str, fields: &[&str], bytes: &[u8], rows: usize) -> Table {
        let kinds = parse_format(format).unwrap();
        let size: usize = kinds.iter().map(|k| k.size()).sum();
        assert!(fields.is_empty() || fields.len() == kinds.len(), "{name} : {} noms pour {} colonnes", fields.len(), kinds.len());
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

    /// Taille d'une ligne en octets.
    pub fn row_size(&self) -> usize {
        self.kinds().iter().map(|k| k.size()).sum()
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Taille de la table en octets.
    pub fn size(&self) -> usize {
        self.row_size() * self.rows.len()
    }

    /// Adresse qui suit la table.
    pub fn end(&self) -> u32 {
        self.rom + self.size() as u32
    }

    /// Colonne `col` de la ligne `row`.
    pub fn get(&self, row: usize, col: usize) -> i64 {
        self.rows[row].cells()[col].value()
    }

    /// Valeur d'une table d'une colonne.
    pub fn value(&self, row: usize) -> i64 {
        self.get(row, 0)
    }

    /// Colonne nommée `field` de la ligne `row`.
    pub fn field(&self, row: usize, field: &str) -> Option<i64> {
        let col = self.fields.iter().position(|f| f == field)?;
        Some(self.get(row, col))
    }

    /// Octets de la table, tels que la ROM les range.
    pub fn bytes(&self) -> Result<Vec<u8>, String> {
        let kinds = parse_format(&self.format)?;
        if !self.fields.is_empty() && self.fields.len() != kinds.len() {
            return Err(format!("{} : {} noms pour {} colonnes", self.name, self.fields.len(), kinds.len()));
        }
        let mut out = Vec::with_capacity(self.size());
        for (r, row) in self.rows.iter().enumerate() {
            let cells = row.cells();
            if cells.len() != kinds.len() {
                return Err(format!("{} ligne {r} : {} valeurs pour {} colonnes", self.name, cells.len(), kinds.len()));
            }
            for (&k, c) in kinds.iter().zip(cells) {
                k.write(c.value(), &mut out).map_err(|e| format!("{} ligne {r} : {e}", self.name))?;
            }
        }
        Ok(out)
    }
}

/// Plage de la ROM entièrement couverte par les tables du fichier (vérifié par les tests).
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

/// Texte (chaîne de caractères ou de tuiles) découpé en lignes pour la lecture. `encoding` :
/// * absent : octets ASCII, écrits comme les textes de `texts.json` (`texts::encode` : ASCII tel quel,
///   autres octets `{xx}`) ;
/// * `"high"` : octets ASCII + 0x80 (textes des dialogues français et allemands) : `0x80` est l'espace,
///   `0xa1..0xfe` le caractère ASCII correspondant, tout autre octet `{xx}` ;
/// * `"tiles"` : mots, tuiles de la grande police (textes des dialogues anglais) : `0x1800` est l'espace,
///   `0x18a1..0x18fe` le caractère ASCII, tout autre mot `{xxxx}` (commandes négatives, 0 final compris).
///
/// `{` et `}` sont toujours écrits en hexadécimal.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TextBlock {
    pub name: String,
    #[serde(with = "hex")]
    pub rom: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encoding: Option<String>,
    pub lines: Vec<String>,
}

/// Caractère ASCII imprimable écrit tel quel dans les textes.
fn plain_char(c: u32) -> Option<char> {
    let c = char::from_u32(c)?;
    (c.is_ascii_graphic() && c != '{' && c != '}').then_some(c)
}

/// Texte « high » ou « tiles » -> chaîne (voir `TextBlock`).
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

/// Chaîne -> mots ou octets (inverse de `encode_units`).
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
    /// Textes d'octets (espace données).
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

    /// Table qui commence à l'adresse `rom` (espace données).
    pub fn at(&self, rom: u32) -> Option<&Table> {
        self.tables.iter().find(|t| t.rom == rom && !t.program)
    }

    /// Table et ligne qui commencent à l'adresse `rom` (espace données).
    pub fn row_at(&self, rom: u32) -> Option<(&Table, usize)> {
        self.tables.iter().filter(|t| !t.program && rom >= t.rom && rom < t.end()).find_map(|t| {
            let d = (rom - t.rom) as usize;
            (d % t.row_size() == 0).then_some((t, d / t.row_size()))
        })
    }

    /// Table désignée par l'entrée `index` de la table d'adresses `pointers` (tables par personnage...).
    pub fn via(&self, pointers: &str, index: usize) -> Option<&Table> {
        self.at(self.table(pointers)?.value(index) as u32)
    }
}
