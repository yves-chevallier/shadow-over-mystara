//! Textes : descripteurs lus par les routines de texte V2 (`ddsom-game/src/text.rs`) et noms des bulles.
//!
//! Les descripteurs sont trouvés en suivant le code : adresses chargées dans A4 avant un appel aux routines
//! de texte (`SEEDS`), tables de descripteurs (`TABLES`) ; chaque descripteur trouvé est suivi de ceux qui
//! lui succèdent en mémoire tant qu'ils sont plausibles (les appelants enchaînent les textes). Une recherche
//! de motifs dans toute la ROM ajoute les descripteurs de texte que le code désigne autrement (`scanned`).

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use ddsom_assets::json;
use ddsom_assets::texts::{encode, NameGroup, PopupNames, TextDesc, TextKind, TextTable, Texts};

use crate::rom::Program;

use TextKind::*;

/// Descripteurs chargés dans A4 (`lea`, `movea.l #`) avant un appel à une routine de texte.
const SEEDS: &[(u32, TextKind)] = &[
    (0x13b52, Text), (0x14e54, Box), (0x14ea2, Text), (0x19f02, Box), (0x19f06, Box), (0x19f0a, Box),
    (0x19f0e, Box), (0x19f12, Box), (0x19f16, Box), (0x19f1a, Box), (0x19f1e, Box), (0x1c546, Box),
    (0x1c54a, Box), (0x1c54e, Box), (0x29f1e, Text), (0x2a03a, Text), (0x2a076, Text), (0x2a224, Text),
    (0x2a298, Text), (0x2a2c0, Text), (0x2a364, Text), (0x2a38a, Text), (0x2a6e4, Text), (0x2a726, Text),
    (0x2acd2, Text), (0x2acf4, Text), (0x2ad16, Text), (0x2ad9c, Text), (0x2addc, Text), (0x2af0e, Text),
    (0x2af18, Text), (0x2af26, Text), (0x2af34, Text), (0x2b386, Text), (0x2b40a, Text), (0x2b42e, Text),
    (0x2b436, Text), (0x2b43e, Text), (0x2b446, Text), (0x2b618, Text), (0x2b7e4, Text), (0x2b92a, Text),
    (0x2b94a, Text), (0x2b96c, Text), (0x2b984, Text), (0x2b9b0, Text), (0x2baf0, Text), (0x2caa6, Text),
    (0x2cad2, Text), (0x2cb16, Text), (0x2cb36, Text), (0x2cb76, Text), (0x2cb96, Text), (0x2cbc8, Text),
    (0x2ccc0, Text), (0x2ccee, Text), (0x2cd12, Text), (0x2cd36, Text), (0x2cd82, Text), (0x2cdce, Text),
    (0x2d184, Text), (0x2d1a8, Text), (0xcbc02, Box), (0xd6922, Box), (0xd694e, Box), (0xd6f0e, Box),
    (0xd86e8, Box), (0xf7336, Tall), (0xf734e, Tall), (0xf985e, Tall), (0xfacc4, Text),
];

/// Tables de descripteurs : (adresse, espace programme, déplacements (mots) plutôt que longs, entrées, sorte).
const TABLES: &[(u32, bool, bool, u32, TextKind)] = &[
    (0x11cba, false, false, 6, Text),
    (0x11cd2, false, false, 6, Text),
    (0x11cea, false, false, 6, Text),
    (0x11d02, false, false, 6, Text),
    (0x15060, false, false, 6, Text),
    (0x1ad98, false, false, 10, Text),
    (0x1adc0, false, false, 10, Text),
    (0x1ade8, false, false, 10, Text),
    (0x1ae10, false, false, 28, Tall),
    (0x1ae80, false, false, 9, Tall),
    (0x1aea4, false, false, 10, Tall),
    (0x1aecc, false, false, 19, TallAt),
    (0x1af18, false, false, 3, TallAt),
    (0x1af24, false, false, 19, TallAt),
    (0x1af70, false, false, 3, TallAt),
    (0x1af7c, false, false, 19, TallAt),
    (0x1afc8, false, false, 3, TallAt),
    (0x2a558, true, false, 3, Text),
    (0xf2968, true, true, 4, Text),
    (0xfad92, false, false, 6, Text),
];

/// Noms terminés par `@` : (table de longs, espace programme, entrées).
const NAMES: &[(u32, bool, u32)] = &[(0xd6232, false, 21), (0xb20e8, true, 6)];

/// Noms des bulles : table de déplacements (espace programme) ; le premier déplacement donne sa taille.
const POPUP_TABLE: u32 = 0x1a01a;
const POPUP_END: u8 = b'/';

/// Lit le descripteur de sorte `kind` en `at` ; `strict` : refuse ce qui ne ressemble pas à un texte
/// (position hors de l'écran, attributs > 0x1f, octets hors de la police, tuiles >= 0x2000).
fn parse(rom: &Program, at: u32, kind: TextKind, strict: bool) -> Option<TextDesc> {
    let mut d = TextDesc { rom: at, kind, col: None, row: None, attr: None, text: None, words: None, size: None, scanned: false };
    let mut p = at;
    if kind != TallAt {
        let (c, r) = (rom.rd8(p), rom.rd8(p + 1));
        if strict && (c >= 0x40 || r >= 0x20) {
            return None;
        }
        d.col = Some(c);
        d.row = Some(r);
        p += 2;
    }
    if kind == Box {
        d.size = Some([rom.rd8(p), rom.rd8(p + 1)]);
        return Some(d);
    }
    let attr = rom.rd16(p);
    if attr > if strict { 0x1f } else { 0xff } && attr & 0x8000 == 0 {
        return None;
    }
    d.attr = Some(attr);
    p += 2;
    let limit = at + 0x1000;
    if kind == Text {
        let mut bytes = Vec::new();
        while p < limit {
            let b = rom.rd8(p);
            p += 1;
            match b {
                b'@' => {
                    d.text = Some(encode(&bytes));
                    return Some(d);
                }
                0xff => {
                    bytes.extend([b, rom.rd8(p)]);
                    p += 1;
                }
                _ if strict && !(0x20..=0x7e).contains(&b) && b != 0xfc => return None,
                _ => bytes.push(b),
            }
        }
        return None;
    }
    let mut words = Vec::new();
    while p < limit {
        let w = rom.rd16(p);
        p += 2;
        match w {
            0x4040 => {
                d.words = Some(words);
                return Some(d);
            }
            0xffff if kind == Blocks => {
                words.extend([w, rom.rd16(p)]);
                p += 2;
            }
            0x2323 | 0x2f40 | 0x2f2f => words.push(w),
            _ if strict && w >= 0x2000 => return None,
            _ => words.push(w),
        }
    }
    None
}

fn end(d: &TextDesc) -> u32 {
    d.rom + d.bytes().len() as u32
}

/// Le descripteur en `at` (accepté sans contrôle : le code le désigne) puis ceux qui le suivent.
fn run(rom: &Program, at: u32, kind: TextKind, found: &mut BTreeMap<u32, TextDesc>) -> bool {
    let Some(first) = parse(rom, at, kind, false) else {
        return false;
    };
    let mut next = end(&first);
    found.entry(at).or_insert(first);
    if kind == Box {
        return true;
    }
    while let Some(d) = parse(rom, next, kind, true) {
        let e = end(&d);
        found.entry(next).or_insert(d);
        next = e;
    }
    true
}

/// Descripteurs de texte non désignés par le code suivi : adresse paire, colonne < 0x40, ligne < 0x20,
/// attributs < 0x20, au moins 6 octets de texte (ASCII ou ©) puis `@` ; puis ceux qui les suivent.
fn scan(rom: &Program, found: &mut BTreeMap<u32, TextDesc>) {
    let covered = |f: &BTreeMap<u32, TextDesc>, a: u32| f.range(..=a).next_back().is_some_and(|(_, d)| a < end(d));
    let mut a = 0;
    while a + 12 < rom.len() {
        let plausible = a % 2 == 0 && rom.rd8(a) < 0x40 && rom.rd8(a + 1) < 0x20 && rom.rd16(a + 2) < 0x20;
        if plausible && !covered(found, a) {
            if let Some(mut d) = parse(rom, a, Text, true) {
                let n = end(&d) - a - 5;
                if n >= 6 && !covered(found, end(&d) - 1) {
                    d.scanned = true;
                    let e = end(&d);
                    found.insert(a, d);
                    let mut next = e;
                    while let Some(mut d) = parse(rom, next, Text, true) {
                        d.scanned = true;
                        let e = end(&d);
                        if covered(found, e - 1) {
                            break;
                        }
                        found.insert(next, d);
                        next = e;
                    }
                    a = next;
                    continue;
                }
            }
        }
        a += 1;
    }
}

fn bad(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

pub fn extract(roms: &Path, path: &Path) -> io::Result<()> {
    let rom = Program::load(roms)?;
    let mut found = BTreeMap::new();
    for &(at, kind) in SEEDS {
        if !run(&rom, at, kind, &mut found) {
            return Err(bad(format!("texte illisible en {at:#x}")));
        }
    }
    let mut tables = Vec::new();
    for &(at, program, offsets, n, kind) in TABLES {
        let entries: Vec<u32> = (0..n)
            .map(|i| match (program, offsets) {
                (true, true) => at.wrapping_add(rom.prog16(at + 2 * i) as i16 as u32),
                (true, false) => rom.prog32(at + 4 * i),
                (false, true) => at.wrapping_add(rom.rd16(at + 2 * i) as i16 as u32),
                (false, false) => rom.rd32(at + 4 * i),
            })
            .collect();
        for &e in &entries {
            if !run(&rom, e, kind, &mut found) {
                return Err(bad(format!("table {at:#x} : texte illisible en {e:#x}")));
            }
        }
        tables.push(TextTable { rom: at, program, offsets, entries });
    }
    let followed = found.len();
    scan(&rom, &mut found);
    // pas de chevauchement
    let texts: Vec<TextDesc> = found.into_values().collect();
    for w in texts.windows(2) {
        if end(&w[0]) > w[1].rom {
            return Err(bad(format!("textes {:#x} et {:#x} : chevauchement", w[0].rom, w[1].rom)));
        }
    }
    // noms des bulles
    let count = rom.prog16(POPUP_TABLE) / 2;
    let offsets: Vec<i16> = (0..count as u32).map(|i| rom.prog16(POPUP_TABLE + 2 * i) as i16).collect();
    let mut starts: Vec<i16> = offsets.clone();
    starts.sort();
    starts.dedup();
    let strings_rom = POPUP_TABLE.wrapping_add(starts[0] as u32);
    let mut strings = Vec::new();
    let mut p = strings_rom;
    for &s in &starts {
        if POPUP_TABLE.wrapping_add(s as u32) != p {
            return Err(bad(format!("noms des bulles : trou avant {p:#x}")));
        }
        let mut bytes = Vec::new();
        while rom.rd8(p) != POPUP_END {
            bytes.push(rom.rd8(p));
            p += 1;
        }
        p += 1;
        strings.push(encode(&bytes));
    }
    eprintln!(
        "textes : {} descripteurs ({} par le code, {} par recherche), {} tables, {} noms de bulles",
        texts.len(),
        followed,
        texts.len() - followed,
        tables.len(),
        strings.len()
    );
    let popups = PopupNames { table: POPUP_TABLE, offsets, strings_rom, strings };
    let names = NAMES.iter().map(|&(table, program, n)| names(&rom, table, program, n)).collect();
    json::save(path, &Texts { texts, tables, popups, names }, 2)
}

/// Noms désignés par une table : tous ceux de la région, du premier désigné à la fin du dernier.
fn names(rom: &Program, table: u32, program: bool, n: u32) -> NameGroup {
    let entries: Vec<u32> = (0..n).map(|i| rom.long(table + 4 * i, program)).collect();
    let (first, last) = (*entries.iter().min().unwrap(), *entries.iter().max().unwrap());
    let mut names = Vec::new();
    let mut p = first;
    while p <= last {
        let mut bytes = Vec::new();
        while rom.rd8(p) != b'@' {
            bytes.push(rom.rd8(p));
            p += 1;
        }
        p += 1;
        names.push(encode(&bytes));
    }
    NameGroup { table, program, entries, rom: first, names }
}
