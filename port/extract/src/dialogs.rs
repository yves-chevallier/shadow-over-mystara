//! Dialog windows and texts in the three languages: `data/dialogs.json` (SET 24 `0xd7056`,
//! `families/set_b.rs` module `dialog`; message list of the test menu `0xf2dc4`).
//!
//! For each language: a table of 256 definitions (one per SET 24 variant), the 18-byte definitions
//! it designates directly or indirectly, the lists of definition or text addresses,
//! and the texts. The whole forms one continuous block per language: the region is walked from the
//! tables (same logic as the loading at 0xd7124 and the text construction), then each gap is
//! classified (definitions, texts, lists, otherwise raw words).

use std::collections::BTreeMap;

use ddsom_assets::data::{encode_units, Region, TextBlock};

use crate::data::{data, prog, region, span, Spec};
use crate::rom::Program;

/// Window definition (0xd7124): column, row, small frame (`+0x18`), rows (`+0x19`), text
/// (`+0x24`), X (`+8`), Z (`+0xc`), kind (`+0x2f`), continuation (`+0x1a`: >= 0, the next definition follows),
/// wait (`+0x32`, 0xffff: until a key), parameter (`+0x34`).
const DEF_FORMAT: &str = "u8*4 addr i16*2 u8 i8 u16 u16";
const DEF_FIELDS: &[&str] = &["col", "row", "small", "lines", "text", "x", "z", "kind", "next", "wait", "param"];
const DEF_SIZE: u32 = 18;

struct Lang {
    code: &'static str,
    /// Table of the 256 definitions.
    definitions: u32,
    /// Message list (hidden test menu).
    messages: u32,
    count: u32,
    /// Texts in tiles (words) rather than bytes.
    tiles: bool,
    /// End of the language's block.
    end: u32,
}

const LANGS: [Lang; 3] = [
    Lang { code: "fr", definitions: 0x1cd000, messages: 0x1cefae, count: 444, tiles: false, end: 0x1d78d0 },
    Lang { code: "en", definitions: 0x1d78d0, messages: 0x1d986c, count: 466, tiles: true, end: 0x1e78da },
    Lang { code: "de", definitions: 0x1e78da, messages: 0x1e9876, count: 444, tiles: false, end: 0 },
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Item {
    Defs,
    List,
    Text,
    Raw,
}

/// End of a text: words up to 0 (a negative command takes two more words, 0xfffe only one), or
/// bytes up to 0xfd, followed by a null byte if the length is odd.
fn text_end(rom: &Program, at: u32, tiles: bool, limit: u32) -> Option<u32> {
    let mut p = at;
    if tiles {
        loop {
            if p + 2 > limit {
                return None;
            }
            let w = rom.rd16(p);
            p += 2;
            match w {
                0 => return Some(p),
                0xfffe => p += 2,
                w if w & 0x8000 != 0 => p += 4,
                _ => {}
            }
        }
    }
    loop {
        if p >= limit {
            return None;
        }
        let b = rom.rd8(p);
        p += 1;
        if b == 0xfd {
            if p & 1 == 1 {
                if rom.rd8(p) != 0 {
                    return None;
                }
                p += 1;
            }
            return Some(p);
        }
    }
}

struct Walk<'a> {
    rom: &'a Program,
    start: u32,
    end: u32,
    tiles: bool,
    items: BTreeMap<u32, (u32, Item)>,
}

impl Walk<'_> {
    fn inside(&self, a: u32) -> bool {
        a >= self.start && a < self.end && a & 1 == 0
    }

    fn text(&mut self, t: u32) {
        if self.items.contains_key(&t) || !self.inside(t) {
            return;
        }
        if let Some(e) = text_end(self.rom, t, self.tiles, self.end) {
            self.items.insert(t, (e, Item::Text));
        }
    }

    fn list(&mut self, at: u32, n: u32) {
        if self.inside(at) {
            self.items.entry(at).or_insert((at + 4 * n, Item::List));
        }
    }

    /// Walks the definitions from `queue`.
    fn defs(&mut self, mut queue: Vec<u32>) {
        let rom = self.rom;
        while let Some(p) = queue.pop() {
            if self.items.contains_key(&p) || !self.inside(p) {
                continue;
            }
            self.items.insert(p, (p + DEF_SIZE, Item::Defs));
            let (t, kind, next) = (rom.rd32(p + 4), rom.rd8(p + 12), rom.rd8(p + 13));
            match kind {
                // the definition is replaced by another (0xd7196)
                1 => {
                    self.list(t, 2);
                    queue.extend([rom.rd32(t), rom.rd32(t + 4)]);
                }
                11 => {
                    let n = (0..8).take_while(|&k| self.inside(rom.rd32(t + 4 * k))).count() as u32;
                    self.list(t, n);
                    queue.extend((0..n).map(|k| rom.rd32(t + 4 * k)));
                }
                5 => queue.extend((0..8).map(|i| t + rom.prog8(KIND5_STEPS + i) as u32)),
                19 => queue.extend((0..4).map(|n| t + DEF_SIZE * n)),
                20 => queue.extend([t, t + 2 * DEF_SIZE]),
                // chosen text (10: linked text, then text; 15: one or several players)
                10 | 15 => {
                    self.list(t, 2);
                    self.text(rom.rd32(t));
                    self.text(rom.rd32(t + 4));
                }
                // no text
                2 | 4 | 12 => {}
                _ => self.text(t),
            }
            if (next as i8) >= 0 && !matches!(kind, 1 | 5 | 11 | 19 | 20) {
                queue.push(p + DEF_SIZE);
            }
        }
    }

    /// Is the gap `[a, b)` a sequence of plausible definitions?
    fn defs_fit(&self, a: u32, b: u32) -> bool {
        (b - a).is_multiple_of(DEF_SIZE)
            && (a..b).step_by(DEF_SIZE as usize).all(|p| {
                let (col, row, kind) = (self.rom.rd8(p), self.rom.rd8(p + 1), self.rom.rd8(p + 12));
                col < 0x40 && row < 0x20 && kind < 24 && (matches!(kind, 2 | 4 | 12) || self.inside(self.rom.rd32(p + 4)))
            })
    }

    /// Classifies each gap of the region. A text designated in its middle (continuation after a page break)
    /// cuts the text containing it: each piece goes up to the start of the next one.
    fn fill(&mut self) -> (u32, u32) {
        let starts: Vec<(u32, u32, Item)> = self.items.iter().map(|(&a, &(e, i))| (a, e, i)).collect();
        for w in starts.windows(2) {
            let ((a, e, i), (b, f, j)) = (w[0], w[1]);
            if i == Item::Text && j == Item::Text && b < e {
                assert_eq!(f, e, "dialogs: texts {a:#x} and {b:#x} have different ends");
                self.items.insert(a, (b, Item::Text));
            }
        }
        let (mut filled, mut raw) = (0, 0);
        let mut at = self.start;
        let spans: Vec<(u32, u32, Item)> = self.items.iter().map(|(&a, &(e, i))| (a, e, i)).collect();
        let mut gaps = Vec::new();
        let mut prev = (0, 0, Item::Raw);
        for (a, e, i) in spans {
            assert!(a >= at, "dialogs: overlap at {a:#x} ({i:?}) with {:#x}-{:#x} ({:?})", prev.0, prev.1, prev.2);
            prev = (a, e, i);
            if a > at {
                gaps.push((at, a));
            }
            at = e;
        }
        if at < self.end {
            gaps.push((at, self.end));
        }
        for (a, b) in gaps {
            filled += 1;
            if self.defs_fit(a, b) {
                for p in (a..b).step_by(DEF_SIZE as usize) {
                    self.items.insert(p, (p + DEF_SIZE, Item::Defs));
                }
                continue;
            }
            let mut texts = Vec::new();
            let mut p = a;
            while p < b {
                match text_end(self.rom, p, self.tiles, b) {
                    Some(e) => {
                        texts.push((p, e));
                        p = e;
                    }
                    None => break,
                }
            }
            for &(s, e) in &texts {
                self.items.insert(s, (e, Item::Text));
            }
            if p == b {
                continue;
            }
            let a = p;
            if (b - a) % 4 == 0 && (a..b).step_by(4).all(|q| self.inside(self.rom.rd32(q))) {
                self.list(a, (b - a) / 4);
                continue;
            }
            raw += 1;
            self.items.insert(a, (b, Item::Raw));
        }
        (filled, raw)
    }
}

/// Offsets of the kind 5 definitions (0xd7196: `2 x +0x42 + (A5+0x6f & 1)`), bytes, program space.
const KIND5_STEPS: u32 = 0xd7218;

/// SET 10 balloons (`families/set.rs` module `dialogue`): per language (`A5+0x72`), 51 text addresses
/// (one per variant); text: width, size (words), then the text (tiles or bytes).
const BUBBLES: u32 = 0xcc444;
const BUBBLE_LANGS: [(&str, bool); 3] = [("en", true), ("fr", false), ("de", false)];
const BUBBLE_COUNT: u32 = 51;

fn bubbles(rom: &Program, specs: &mut Vec<Spec>, texts: &mut Vec<TextBlock>) -> [Region; 2] {
    specs.push(prog("bubble_tables", 0xcc438, "addr", &[], 3));
    let mut items: BTreeMap<u32, (u32, Option<bool>)> = BTreeMap::new();
    for (k, &(code, tiles)) in BUBBLE_LANGS.iter().enumerate() {
        let list = BUBBLES + 4 * BUBBLE_COUNT * k as u32;
        specs.push(data(&format!("{code}/bubbles"), list, "addr", &[], BUBBLE_COUNT as usize));
        for i in 0..BUBBLE_COUNT {
            let t = rom.rd32(list + 4 * i);
            let end = text_end(rom, t + 4, tiles, 0x100000).expect("bulle");
            items.insert(t, (end, Some(tiles)));
        }
    }
    // the lists are followed by code (0xcc6a8), then the texts
    let first = *items.keys().next().unwrap();
    let end = items.values().map(|&(e, _)| e).max().unwrap();
    // gaps between texts: raw bytes
    let mut at = first;
    let spans: Vec<(u32, u32)> = items.iter().map(|(&a, &(e, _))| (a, e)).collect();
    for (a, e) in spans {
        assert!(a >= at, "balloons: overlap at {a:#x}");
        if a > at {
            items.insert(at, (a, None));
        }
        at = e;
    }
    for (&a, &(e, tiles)) in &items {
        let Some(tiles) = tiles else {
            specs.push(data(&format!("bubble_raw@{a:#x}"), a, "x8", &[], (e - a) as usize));
            continue;
        };
        let code = if tiles { "en" } else if a >= rom.rd32(BUBBLES + 4 * BUBBLE_COUNT) { "fr" } else { "de" };
        specs.push(data(&format!("{code}/bubble@{a:#x}"), a, "u16*2", &["width", "size"], 1));
        let units: Vec<u16> = if tiles {
            (a + 4..e).step_by(2).map(|p| rom.rd16(p)).collect()
        } else {
            (a + 4..e).map(|p| rom.rd8(p) as u16).collect()
        };
        texts.push(TextBlock {
            name: format!("{code}/bubble_text@{:#x}", a + 4),
            rom: a + 4,
            encoding: Some(if tiles { "tiles" } else { "high" }.into()),
            lines: vec![encode_units(&units, tiles)],
        });
    }
    [region("bubble_lists", BUBBLES, BUBBLES + 4 * BUBBLE_COUNT * 3), region("bubbles", first, end)]
}

/// Text in tiles (words up to and including the 0) from `a` to `e`.
fn tile_text(rom: &Program, name: String, a: u32, e: u32) -> TextBlock {
    let units: Vec<u16> = (a..e).step_by(2).map(|p| rom.rd16(p)).collect();
    TextBlock { name, rom: a, encoding: Some("tiles".into()), lines: vec![encode_units(&units, true)] }
}

/// Tables of the text construction (module `dialog`): predefined names, texts depending on the number of
/// players, class titles, tiles of the name characters, objects and their texts.
fn construction(rom: &Program, specs: &mut Vec<Spec>, texts: &mut Vec<TextBlock>) -> Vec<Region> {
    // predefined names (0xd7604): word n, then at most n + 1 characters (0xff ends), 10 bytes each
    specs.push(data("names", 0xd7604, "addr", &[], 5));
    for i in 0..5u32 {
        let at = rom.rd32(0xd7604 + 4 * i);
        specs.push(data(&format!("name@{at:#x}"), at, "u16 u8*8", &[], 1));
    }
    specs.push(data("players_one_two", 0xd7d96, "addr", &[], 4));
    specs.push(data("players_one_two_b", 0xd7da6, "addr", &[], 4));
    specs.push(data("class_titles", 0xd7db6, "addr", &[], 6));
    // texts up to the character table
    const CHARS: u32 = 0xd7e8a;
    // (a text designated in its middle cuts the one containing it)
    let targets: Vec<u32> = (0..14).map(|i| rom.rd32(0xd7d96 + 4 * i)).collect();
    let mut p = 0xd7dce;
    while p < CHARS {
        let start = p;
        p += 2;
        while rom.rd16(p - 2) != 0 && !targets.contains(&p) {
            p += 2;
        }
        texts.push(tile_text(rom, format!("build_text@{start:#x}"), start, p));
    }
    assert_eq!(p, CHARS);
    // tile of each name character code (0xd75a6...), up to the object texts
    let items: Vec<u32> = (0..5).map(|i| rom.prog32(0xd7a48 + 4 * i)).collect();
    specs.push(span("name_chars", CHARS, items[0], false, "u16", &[]));
    specs.push(prog("item_texts", 0xd7a48, "addr", &[], 5));
    specs.push(data("item_lists", 0xd7a20, "u8*6", &[], 4));
    let mut end = items[0];
    for &at in &items {
        assert_eq!(at, end, "object texts not contiguous");
        let mut e = at;
        while rom.rd16(e) != 0 {
            e += 2;
        }
        end = e + 2;
        texts.push(tile_text(rom, format!("item_text@{at:#x}"), at, end));
    }
    vec![region("names", 0xd7604, 0xd764a), region("build", 0xd7d96, end)]
}

/// End of a level transition message (0xce9ea): words like the dialogs, or bytes up to 0xfd
/// (0xfe: one parameter byte, 0xff: two, byte <= 0xf: accented letter on two bytes), then
/// a null byte if the length is odd.
fn level_text_end(rom: &Program, at: u32, tiles: bool) -> u32 {
    if tiles {
        return text_end(rom, at, true, 0x100000).expect("level message");
    }
    let mut p = at;
    loop {
        let c = rom.rd8(p);
        p += 1;
        match c {
            0xfd => break,
            0xfe | 0..=0xf => p += 1,
            0xff => p += 2,
            _ => {}
        }
    }
    if p & 1 == 1 && rom.rd8(p) == 0 {
        p += 1;
    }
    p
}

/// Level transition (SET C 7, `families/set_c/finale/status.rs`): per class, three message lists (one
/// per language: English in tiles, French, German), then a 20-byte record per level
/// reached; the message lists follow.
fn levelup(rom: &Program, specs: &mut Vec<Spec>, texts: &mut Vec<TextBlock>) -> Vec<Region> {
    const TABLES: u32 = 0xceaa2;
    const RANGE: u32 = 0xce728;
    const RECORD: &str = "u16*3 i8*10 u8*4";
    const FIELDS: &[&str] = &[
        "hp", "message_a", "message_b", "stats", "s1", "s2", "s3", "s4", "s5", "s6", "s7", "s8", "s9", "new_a",
        "old_a", "new_b", "old_b",
    ];
    const LANGS: [(&str, bool); 3] = [("en", true), ("fr", false), ("de", false)];
    specs.push(prog("levelup_tables", TABLES, "addr", &[], 6));
    specs.push(prog("levelup_range", RANGE, "u8*2", &["min", "max"], 6));
    specs.push(prog("levelup_frame_offsets", 0xce7ac, "u16", &[], 6));
    specs.push(span("levelup_frame_by_step", 0xce7b8, 0xce7e4, true, "u8", &[]));
    specs.push(prog("levelup_frames", 0xce7e4, "x32 u16 u16", &["at", "width", "unused"], 3));
    specs.push(data("levelup_names", 0xce9a4, "addr", &[], 5));
    for i in 0..5u32 {
        let at = rom.rd32(0xce9a4 + 4 * i);
        specs.push(data(&format!("levelup_name@{at:#x}"), at, "u16 u8*8", &[], 1));
    }
    let mut messages: BTreeMap<u32, bool> = BTreeMap::new();
    let mut end = 0;
    let first = rom.prog32(TABLES);
    for c in 0..6u32 {
        let class = ["fighter", "cleric", "elf", "dwarf", "magic_user", "thief"][c as usize];
        let t = rom.prog32(TABLES + 4 * c);
        let n = (rom.prog8(RANGE + 2 * c + 1) - rom.prog8(RANGE + 2 * c) + 1) as u32;
        specs.push(data(&format!("levelup/{class}.lists"), t, "addr", &[], 3));
        specs.push(data(&format!("levelup/{class}"), t + 12, RECORD, FIELDS, n as usize));
        let choices = (0..n).flat_map(|k| [rom.rd16(t + 14 + 20 * k), rom.rd16(t + 16 + 20 * k)]).max().unwrap();
        let count = choices as u32 / 4 + 1;
        for (k, &(lang, tiles)) in LANGS.iter().enumerate() {
            let list = rom.rd32(t + 4 * k as u32);
            specs.push(data(&format!("levelup/{class}/{lang}"), list, "addr", &[], count as usize));
            for i in 0..count {
                messages.insert(rom.rd32(list + 4 * i), tiles);
            }
            end = end.max(list + 4 * count);
        }
    }
    // messages: contiguous, each up to the start of the next (a message designated in its middle cuts the
    // previous one)
    let starts: Vec<(u32, bool)> = messages.into_iter().collect();
    let mut text_end_max = 0;
    for (i, &(a, tiles)) in starts.iter().enumerate() {
        let full = level_text_end(rom, a, tiles);
        let e = starts.get(i + 1).map(|&(b, _)| b.min(full)).unwrap_or(full);
        if let Some(&(b, _)) = starts.get(i + 1) {
            assert!(b >= e, "level messages: gap before {b:#x}");
        }
        text_end_max = e;
        let (units, enc): (Vec<u16>, &str) = if tiles {
            ((a..e).step_by(2).map(|p| rom.rd16(p)).collect(), "tiles")
        } else {
            ((a..e).map(|p| rom.rd8(p) as u16).collect(), "high")
        };
        texts.push(TextBlock {
            name: format!("levelup_text@{a:#x}"),
            rom: a,
            encoding: Some(enc.into()),
            lines: vec![encode_units(&units, tiles)],
        });
    }
    vec![
        region("levelup", first, end),
        region("levelup_names", 0xce9a4, 0xce9ea),
        region("levelup_texts", starts[0].0, text_end_max),
    ]
}

pub fn specs(rom: &Program) -> (Vec<Region>, Vec<Spec>, Vec<TextBlock>) {
    let mut specs = vec![
        prog("message_lists", 0xf2dc4, "addr", &[], 7),
        prog("kind5_steps", KIND5_STEPS, "u8", &[], 8),
    ];
    let mut texts = Vec::new();
    let mut regions = Vec::new();
    for lang in &LANGS {
        let start = lang.definitions;
        let mut w = Walk { rom, start, end: if lang.end != 0 { lang.end } else { 0x200000 }, tiles: lang.tiles, items: BTreeMap::new() };
        w.items.insert(start, (start + 0x400, Item::List));
        w.items.insert(lang.messages, (lang.messages + 4 * lang.count, Item::List));
        let roots: Vec<u32> = (0..256).map(|i| rom.rd32(start + 4 * i)).collect();
        w.defs(roots);
        for i in 0..lang.count {
            w.text(rom.rd32(lang.messages + 4 * i));
        }
        if lang.end == 0 {
            // last language: the block stops after the last element reached
            w.end = w.items.values().map(|&(e, _)| e).max().unwrap();
        }
        let end = w.end;
        let (gaps, raw) = w.fill();
        eprintln!("dialogs {}: {} elements, {gaps} gaps, of which {raw} raw", lang.code, w.items.len());
        // tables: contiguous definitions grouped
        let mut run: Option<(u32, u32)> = None;
        let flush = |run: &mut Option<(u32, u32)>, specs: &mut Vec<Spec>| {
            if let Some((a, e)) = run.take() {
                let name = format!("{}/defs@{a:#x}", lang.code);
                specs.push(data(&name, a, DEF_FORMAT, DEF_FIELDS, ((e - a) / DEF_SIZE) as usize));
            }
        };
        for (&a, &(e, item)) in &w.items {
            if item != Item::Defs {
                flush(&mut run, &mut specs);
            }
            match item {
                Item::Defs => {
                    run = match run {
                        Some((s, re)) if re == a => Some((s, e)),
                        _ => {
                            flush(&mut run, &mut specs);
                            Some((a, e))
                        }
                    }
                }
                Item::List => {
                    let name = match a {
                        _ if a == start => format!("{}/definitions", lang.code),
                        _ if a == lang.messages => format!("{}/messages", lang.code),
                        _ => format!("{}/list@{a:#x}", lang.code),
                    };
                    specs.push(data(&name, a, "addr", &[], ((e - a) / 4) as usize));
                }
                Item::Text => {
                    let units: Vec<u16> = if lang.tiles {
                        (a..e).step_by(2).map(|p| rom.rd16(p)).collect()
                    } else {
                        (a..e).map(|p| rom.rd8(p) as u16).collect()
                    };
                    texts.push(TextBlock {
                        name: format!("{}/text@{a:#x}", lang.code),
                        rom: a,
                        encoding: Some(if lang.tiles { "tiles" } else { "high" }.into()),
                        lines: vec![encode_units(&units, lang.tiles)],
                    });
                }
                Item::Raw => {
                    let format = if (e - a) % 2 == 0 { "x16" } else { "x8" };
                    let size = if format == "x16" { 2 } else { 1 };
                    specs.push(data(&format!("{}/raw@{a:#x}", lang.code), a, format, &[], ((e - a) / size) as usize));
                }
            }
        }
        flush(&mut run, &mut specs);
        regions.push(region(lang.code, start, end));
    }
    regions.extend(bubbles(rom, &mut specs, &mut texts));
    regions.extend(construction(rom, &mut specs, &mut texts));
    regions.extend(levelup(rom, &mut specs, &mut texts));
    (regions, specs, texts)
}
