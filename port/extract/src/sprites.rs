//! Définitions des sprites : images (assemblages de cellules) et scripts d'animation, en JSON.
//! Voir docs/assets.md pour les tables lues et le format.

use std::collections::{BTreeMap, HashSet};
use std::io;
use std::ops::Range;
use std::path::Path;

use ddsom_assets::json;
use ddsom_assets::sprites::{AnimSet, Anims, Entry, Family, Frame, Frames, Piece};

use crate::rom::Program;

/// Table des adresses d'images (un long par image, dans l'ordre des images).
const FRAME_TABLE: u32 = 0x293000;
/// Images, les unes à la suite des autres, puis tables de décalages des morceaux.
const FRAMES: Range<u32> = 0x2b45d8..0x37c3d2;
const OFFSETS: Range<u32> = 0x37c3d2..0x3ec70a;
/// Visualiseur de sprites du menu de test caché : noms des familles (4 caractères chacun), table de
/// décalages (mots, relatifs à la table) vers les listes de jeux par variante, et nombre de variantes
/// (octet bas d'un long par famille : 4 x (variantes - 1)).
const VIEWER_NAMES: u32 = 0xefe16;
const VIEWER_FAMILIES: u32 = 0xf0532;
const VIEWER_VARIANTS: u32 = 0xf06c4;
/// Familles à entrées longues (les douze `plNN`, test `cmpi.w #0x16` en 0xefa76).
const LONG_FAMILIES: usize = 12;
/// Région des scripts d'animation, parcourue pour les entrées qu'aucun jeu connu n'atteint.
const SCRIPTS: Range<u32> = 0x120000..0x1d0000;

pub fn extract(roms: &Path, dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let rom = Program::load(roms)?;
    let frames = frames(&rom)?;
    let anims = anims(&rom, &frames)?;
    json::save(&dir.join("frames.json"), &frames, 2)?;
    json::save(&dir.join("anims.json"), &anims, 2)?;
    Ok(())
}

fn bad(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

/// Lit l'image dont l'en-tête est en `at` ; renvoie aussi l'adresse qui la suit.
fn frame_at(rom: &Program, at: u32) -> (Frame, u32) {
    let (cost, attr, bank, mode, offsets) = (rom.rd16(at), rom.rd16(at + 2), rom.rd8(at + 4), rom.rd8(at + 5), rom.rd32(at + 6));
    let n = if mode != 0 { attr } else { cost } as u32;
    let step = if mode != 0 { 4 } else { 2 };
    let pieces = (0..n)
        .map(|i| {
            let p = at + 10 + step * i;
            let a = if mode != 0 { rom.rd16(p + 2) } else { attr };
            Piece(rom.rd16(offsets + 4 * i) as i16, rom.rd16(offsets + 4 * i + 2) as i16, rom.rd16(p), a)
        })
        .collect();
    (Frame { rom: at, cost, attr, bank, mode, offsets, pieces }, at + 10 + step * n)
}

fn frames(rom: &Program) -> io::Result<Frames> {
    let mut frames = Vec::new();
    let mut at = FRAMES.start;
    while at < FRAMES.end {
        let (f, next) = frame_at(rom, at);
        if !OFFSETS.contains(&f.offsets) || f.pieces.is_empty() {
            return Err(bad(format!("image illisible en {at:#x}")));
        }
        frames.push(f);
        at = next;
    }
    if at != FRAMES.end {
        return Err(bad(format!("la dernière image déborde ({at:#x})")));
    }
    for (i, f) in frames.iter().enumerate() {
        if rom.rd32(FRAME_TABLE + 4 * i as u32) != f.rom {
            return Err(bad(format!("table des images : entrée {i} différente")));
        }
    }
    // les tables de décalages couvrent exactement leur région
    let mut tables: BTreeMap<u32, u32> = BTreeMap::new();
    for f in &frames {
        let len = tables.entry(f.offsets).or_default();
        *len = (*len).max(4 * f.pieces.len() as u32);
    }
    let mut end = OFFSETS.start;
    for (&a, &len) in &tables {
        if a != end {
            return Err(bad(format!("tables de décalages : trou ou chevauchement en {a:#x}")));
        }
        end = a + len;
    }
    if end != OFFSETS.end {
        return Err(bad(format!("tables de décalages : fin en {end:#x}")));
    }
    Ok(Frames { table: FRAME_TABLE, frames })
}

struct Scan<'a> {
    rom: &'a Program,
    starts: HashSet<u32>,
    frame_of: BTreeMap<u32, u32>,
    entries: BTreeMap<u32, Entry>,
}

impl Scan<'_> {
    /// Une entrée d'animation peut commencer en `a` : son image est un début d'image, sa durée non nulle.
    fn plausible(&self, a: u32) -> bool {
        a % 2 == 0 && a + 24 <= self.rom.len() && self.starts.contains(&self.rom.rd32(a + 4)) && self.rom.rd16(a) != 0
    }

    fn entry(&self, a: u32, long: bool, guessed: bool) -> Entry {
        let rom = self.rom;
        let size = if long { 24 } else { 16 };
        let flags = rom.rd16(a + 2);
        let next = (!matches!(flags >> 8, 0 | 0xfe)).then(|| rom.rd32(a + size));
        Entry {
            rom: a,
            long,
            guessed,
            time: rom.rd16(a),
            flags,
            frame: self.frame_of[&rom.rd32(a + 4)],
            data: (8..size).map(|o| rom.rd8(a + o)).collect(),
            next,
        }
    }

    /// Entrées du script qui commence en `a` (suites et sauts), sans les entrées déjà connues. Si l'une
    /// d'elles n'est pas plausible : `None` (`strict`) ou arrêt du chemin (quelques scripts se terminent
    /// sur une entrée « suite » que le jeu ne dépasse jamais).
    fn chain(&self, a: u32, long: bool, guessed: bool, strict: bool) -> Option<Vec<Entry>> {
        let mut out: Vec<Entry> = Vec::new();
        let mut todo = vec![a];
        while let Some(mut a) = todo.pop() {
            while !self.entries.contains_key(&a) && !out.iter().any(|e| e.rom == a) {
                if !self.plausible(a) {
                    if strict || out.is_empty() {
                        return None;
                    }
                    break;
                }
                let e = self.entry(a, long, guessed);
                let (flags, next, end) = (e.flags >> 8, e.next, e.rom + if long { 24 } else { 16 });
                out.push(e);
                match flags {
                    0 => a = end,
                    0xfe => break,
                    _ => {
                        todo.push(next.unwrap());
                        break;
                    }
                }
            }
        }
        Some(out)
    }
}

fn anims(rom: &Program, frames: &Frames) -> io::Result<Anims> {
    let mut scan = Scan {
        rom,
        starts: frames.frames.iter().map(|f| f.rom).collect(),
        frame_of: frames.frames.iter().enumerate().map(|(i, f)| (f.rom, i as u32)).collect(),
        entries: BTreeMap::new(),
    };

    // familles du visualiseur
    let mut families = Vec::new();
    for i in 0.. {
        let at = VIEWER_NAMES + 4 * i;
        let name: Vec<u8> = (0..4).map(|k| rom.rd8(at + k)).collect();
        if !name.iter().all(|c| c.is_ascii_alphanumeric()) {
            break;
        }
        let list = VIEWER_FAMILIES + rom.rd16(VIEWER_FAMILIES + 2 * i) as u32;
        let variants = (rom.rd32(VIEWER_VARIANTS + 4 * i) & 0xff) / 4 + 1;
        families.push(Family {
            name: String::from_utf8(name).unwrap(),
            long: (i as usize) < LONG_FAMILIES,
            sets: (0..variants).map(|v| rom.rd32(list + 4 * v)).collect(),
        });
    }

    // jeux : décalages jusqu'au premier script, au jeu suivant ou à une entrée d'animation, sans les
    // derniers qui ne mènent à rien
    let mut kinds: BTreeMap<u32, bool> = BTreeMap::new();
    for f in &families {
        for &s in &f.sets {
            kinds.insert(s, f.long);
        }
    }
    let starts: Vec<u32> = kinds.keys().copied().collect();
    let mut sets = Vec::new();
    let (mut broken, mut holes) = (0, 0);
    for (&s, &long) in &kinds {
        let mut bound = starts.iter().copied().find(|&x| x > s).unwrap_or(u32::MAX);
        let mut scripts = Vec::new();
        let mut valid = 0;
        while s + 2 * (scripts.len() as u32) < bound && !scan.plausible(s + 2 * scripts.len() as u32) {
            let off = rom.rd16(s + 2 * scripts.len() as u32) as i16;
            scripts.push(off);
            let t = s.wrapping_add(off as i32 as u32);
            if scan.plausible(t) {
                valid = scripts.len();
                if t > s {
                    bound = bound.min(t);
                }
            }
        }
        // un jeu dont le premier script n'est pas lisible n'en est pas un (bs16, variante 2 : décalages
        // relatifs à un autre jeu)
        if !scripts.first().is_some_and(|&o| scan.plausible(s.wrapping_add(o as i32 as u32))) {
            valid = 0;
        }
        scripts.truncate(valid);
        for &off in &scripts {
            let t = s.wrapping_add(off as i32 as u32);
            if !scan.plausible(t) {
                holes += 1;
                continue;
            }
            match scan.chain(t, long, false, false) {
                Some(chain) => chain.into_iter().for_each(|e| {
                    scan.entries.insert(e.rom, e);
                }),
                None => broken += 1,
            }
        }
        sets.push(AnimSet { rom: s, long, scripts });
    }
    let from_sets = scan.entries.len();

    // entrées isolées de la région des scripts : taille choisie pour que le script soit cohérent et,
    // à défaut de mieux, suivi d'une autre entrée
    let mut covered: HashSet<u32> = HashSet::new();
    for e in scan.entries.values() {
        covered.extend((e.rom..e.end()).step_by(2));
    }
    let (mut orphans, mut unreadable) = (0, 0);
    let mut a = SCRIPTS.start;
    while a < SCRIPTS.end {
        if covered.contains(&a) || !scan.plausible(a) {
            a += 2;
            continue;
        }
        let short = scan.chain(a, false, true, true);
        let long = scan.chain(a, true, true, true);
        let followed = |c: &Option<Vec<Entry>>| c.as_ref().is_some_and(|c| c.last().is_some_and(|e| scan.plausible(e.end())));
        let pick = match (&short, &long) {
            (Some(_), Some(_)) if !followed(&short) && followed(&long) => long,
            (Some(_), _) => short,
            _ => long,
        };
        match pick {
            Some(chain) => {
                orphans += chain.len();
                for e in chain {
                    covered.extend((e.rom..e.end()).step_by(2));
                    scan.entries.insert(e.rom, e);
                }
            }
            None => unreadable += 1,
        }
        a += 2;
    }

    let entries: Vec<Entry> = scan.entries.into_values().collect();
    let overlaps = entries.windows(2).filter(|w| w[0].end() > w[1].rom).count();
    eprintln!(
        "sprites : {} images, {} familles, {} jeux, {} entrées ({} par les jeux, {} isolées), \
         {} scripts illisibles depuis les jeux, {} décalages sans script, {} débuts isolés illisibles, {} chevauchements",
        frames.frames.len(),
        families.len(),
        sets.len(),
        entries.len(),
        from_sets,
        orphans,
        broken,
        holes,
        unreadable,
        overlaps
    );
    Ok(Anims { names_rom: VIEWER_NAMES, families_rom: VIEWER_FAMILIES, variants_rom: VIEWER_VARIANTS, families, sets, entries })
}
