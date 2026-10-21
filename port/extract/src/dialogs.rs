//! Fenêtres de dialogue et textes des trois langues : `data/dialogs.json` (SET 24 `0xd7056`,
//! `families/set_b.rs` module `dialog` ; liste de messages du menu de test `0xf2dc4`).
//!
//! Pour chaque langue : une table de 256 définitions (une par variante de SET 24), les définitions de
//! 18 octets qu'elle désigne directement ou indirectement, les listes d'adresses de définitions ou de
//! textes, et les textes. Le tout forme un bloc continu par langue : la région est parcourue depuis les
//! tables (même logique que le chargement 0xd7124 et la construction des textes), puis chaque trou est
//! classé (définitions, textes, listes, sinon mots bruts).

use std::collections::BTreeMap;

use ddsom_assets::data::{encode_units, Region, TextBlock};

use crate::data::{data, prog, region, Spec};
use crate::rom::Program;

/// Définition de fenêtre (0xd7124) : colonne, ligne, petit cadre (`+0x18`), lignes (`+0x19`), texte
/// (`+0x24`), X (`+8`), Z (`+0xc`), sorte (`+0x2f`), suite (`+0x1a` : >= 0, la définition suivante suit),
/// attente (`+0x32`, 0xffff : jusqu'à une touche), paramètre (`+0x34`).
const DEF_FORMAT: &str = "u8*4 addr i16*2 u8 i8 u16 u16";
const DEF_FIELDS: &[&str] = &["col", "row", "small", "lines", "text", "x", "z", "kind", "next", "wait", "param"];
const DEF_SIZE: u32 = 18;

struct Lang {
    code: &'static str,
    /// Table des 256 définitions.
    definitions: u32,
    /// Liste des messages (menu de test caché).
    messages: u32,
    count: u32,
    /// Textes en tuiles (mots) plutôt qu'en octets.
    tiles: bool,
    /// Fin du bloc de la langue.
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

/// Fin d'un texte : mots jusqu'à 0 (une commande négative prend deux mots de plus, 0xfffe un seul), ou
/// octets jusqu'à 0xfd, suivis d'un octet nul si la longueur est impaire.
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

    /// Parcourt les définitions depuis `queue`.
    fn defs(&mut self, mut queue: Vec<u32>) {
        let rom = self.rom;
        while let Some(p) = queue.pop() {
            if self.items.contains_key(&p) || !self.inside(p) {
                continue;
            }
            self.items.insert(p, (p + DEF_SIZE, Item::Defs));
            let (t, kind, next) = (rom.rd32(p + 4), rom.rd8(p + 12), rom.rd8(p + 13));
            match kind {
                // la définition est remplacée par une autre (0xd7196)
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
                // texte choisi (10 : texte relié, puis texte ; 15 : un ou plusieurs joueurs)
                10 | 15 => {
                    self.list(t, 2);
                    self.text(rom.rd32(t));
                    self.text(rom.rd32(t + 4));
                }
                // pas de texte
                2 | 4 | 12 => {}
                _ => self.text(t),
            }
            if (next as i8) >= 0 && !matches!(kind, 1 | 5 | 11 | 19 | 20) {
                queue.push(p + DEF_SIZE);
            }
        }
    }

    /// Le trou `[a, b)` est-il une suite de définitions plausibles ?
    fn defs_fit(&self, a: u32, b: u32) -> bool {
        (b - a) % DEF_SIZE == 0
            && (a..b).step_by(DEF_SIZE as usize).all(|p| {
                let (col, row, kind) = (self.rom.rd8(p), self.rom.rd8(p + 1), self.rom.rd8(p + 12));
                col < 0x40 && row < 0x20 && kind < 24 && (matches!(kind, 2 | 4 | 12) || self.inside(self.rom.rd32(p + 4)))
            })
    }

    /// Classe chaque trou de la région. Un texte désigné en son milieu (suite après un saut de page)
    /// coupe le texte qui le contient : chaque morceau va jusqu'au début du suivant.
    fn fill(&mut self) -> (u32, u32) {
        let starts: Vec<(u32, u32, Item)> = self.items.iter().map(|(&a, &(e, i))| (a, e, i)).collect();
        for w in starts.windows(2) {
            let ((a, e, i), (b, f, j)) = (w[0], w[1]);
            if i == Item::Text && j == Item::Text && b < e {
                assert_eq!(f, e, "dialogues : textes {a:#x} et {b:#x} de fins différentes");
                self.items.insert(a, (b, Item::Text));
            }
        }
        let (mut filled, mut raw) = (0, 0);
        let mut at = self.start;
        let spans: Vec<(u32, u32, Item)> = self.items.iter().map(|(&a, &(e, i))| (a, e, i)).collect();
        let mut gaps = Vec::new();
        let mut prev = (0, 0, Item::Raw);
        for (a, e, i) in spans {
            assert!(a >= at, "dialogues : chevauchement en {a:#x} ({i:?}) avec {:#x}-{:#x} ({:?})", prev.0, prev.1, prev.2);
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

/// Décalages des définitions de la sorte 5 (0xd7196 : `2 x +0x42 + (A5+0x6f & 1)`), octets, espace programme.
const KIND5_STEPS: u32 = 0xd7218;

/// Bulles de SET 10 (`families/set.rs` module `dialogue`) : par langue (`A5+0x72`), 51 adresses de textes
/// (une par variante) ; texte : largeur, taille (mots), puis le texte (tuiles ou octets).
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
    // les listes sont suivies de code (0xcc6a8), puis des textes
    let first = *items.keys().next().unwrap();
    let end = items.values().map(|&(e, _)| e).max().unwrap();
    // trous entre les textes : octets bruts
    let mut at = first;
    let spans: Vec<(u32, u32)> = items.iter().map(|(&a, &(e, _))| (a, e)).collect();
    for (a, e) in spans {
        assert!(a >= at, "bulles : chevauchement en {a:#x}");
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
            // dernière langue : le bloc s'arrête après le dernier élément atteint
            w.end = w.items.values().map(|&(e, _)| e).max().unwrap();
        }
        let end = w.end;
        let (gaps, raw) = w.fill();
        eprintln!("dialogues {} : {} éléments, {gaps} trous, dont {raw} bruts", lang.code, w.items.len());
        // tables : définitions jointives regroupées
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
    (regions, specs, texts)
}
