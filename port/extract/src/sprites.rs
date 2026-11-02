//! Sprite definitions: frames (cell assemblies) and animation scripts, as JSON.
//! See docs/assets.md for the tables read and the format.

use std::collections::{BTreeMap, HashSet};
use std::io;
use std::ops::Range;
use std::path::Path;

use ddsom_assets::json;
use ddsom_assets::sprites::{AnimSet, Anims, Entry, Family, Frame, Frames, Piece};

use crate::rom::Program;

/// Table of frame addresses (one long per frame, in frame order).
const FRAME_TABLE: u32 = 0x293000;
/// Frames, one after the other, then piece offset tables.
const FRAMES: Range<u32> = 0x2b45d8..0x37c3d2;
const OFFSETS: Range<u32> = 0x37c3d2..0x3ec70a;
/// Sprite viewer of the hidden test menu: family names (4 characters each), offset
/// table (words, relative to the table) toward the set lists per variant, and number of variants
/// (low byte of a long per family: 4 x (variants - 1)).
const VIEWER_NAMES: u32 = 0xefe16;
const VIEWER_FAMILIES: u32 = 0xf0532;
const VIEWER_VARIANTS: u32 = 0xf06c4;
/// Families with long entries (the twelve `plNN`, test `cmpi.w #0x16` at 0xefa76).
const LONG_FAMILIES: usize = 12;
/// Region of the animation scripts, walked for the entries that no known set reaches.
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

/// Reads the frame whose header is at `at`; also returns the address that follows it.
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
            return Err(bad(format!("unreadable frame at {at:#x}")));
        }
        frames.push(f);
        at = next;
    }
    if at != FRAMES.end {
        return Err(bad(format!("the last frame overflows ({at:#x})")));
    }
    for (i, f) in frames.iter().enumerate() {
        if rom.rd32(FRAME_TABLE + 4 * i as u32) != f.rom {
            return Err(bad(format!("frame table: entry {i} differs")));
        }
    }
    // the offset tables cover exactly their region
    let mut tables: BTreeMap<u32, u32> = BTreeMap::new();
    for f in &frames {
        let len = tables.entry(f.offsets).or_default();
        *len = (*len).max(4 * f.pieces.len() as u32);
    }
    let mut end = OFFSETS.start;
    for (&a, &len) in &tables {
        if a != end {
            return Err(bad(format!("offset tables: gap or overlap at {a:#x}")));
        }
        end = a + len;
    }
    if end != OFFSETS.end {
        return Err(bad(format!("offset tables: end at {end:#x}")));
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
    /// An animation entry can start at `a`: its frame is a frame start, its duration non-null.
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

    /// Entries of the script starting at `a` (continuations and jumps), without the already known entries. If one
    /// of them is not plausible: `None` (`strict`) or the path stops (a few scripts end
    /// on a "continue" entry that the game never goes past).
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

    // viewer families
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

    // sets: offsets up to the first script, the next set or an animation entry, without the
    // last ones which lead nowhere
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
    // a set whose first script is not readable is not one (bs16, variant 2: offsets
    // relative to another set)
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

    // isolated entries of the region of the scripts: size chosen so the script is coherent and,
    // failing better, followed by another entry
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
        "sprites: {} frames, {} families, {} sets, {} entries ({} by the sets, {} isolated), \
         {} scripts unreadable from the sets, {} offsets without script, {} isolated starts unreadable, {} overlaps",
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
