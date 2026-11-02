//! Rebuilding of the ROM contents from the extracted resources: graphics region, QSound sample
//! ROM and tables of the 68000 program ROM, at the original addresses.
//!
//! Each `put_*` function is the inverse of loading a file: it rewrites its tables as the ROM
//! stores them (the tests of `tests/roundtrip.rs` compare the result byte for byte).
//! Only the bytes described by the files are written: `Image::covered` gives the covered zones,
//! the rest of the image is zero.

use std::io;
use std::path::Path;

use crate::data::DataFile;
use crate::pointers::Pointers;
use crate::sprites::Frame;
use crate::remainder::Remainder;
use crate::texts;
use crate::{
    Anims, Cells, Frames, Levels, Music, Palettes, ScenePalettes, Spawns, Speeds, SpriteTables, Terrain, Texts,
};

/// Size of the program ROM (space 0x000000-0x3fffff).
pub const PROGRAM_SIZE: usize = 0x40_0000;

/// Bytes of a ROM zone, with the list of bytes that the resources describe.
pub struct Image {
    pub bytes: Vec<u8>,
    known: Vec<u64>,
    /// Addresses where two writes contradict each other (should stay empty).
    pub conflicts: Vec<u32>,
}

impl Image {
    pub fn new(size: usize) -> Self {
        Image { bytes: vec![0; size], known: vec![0; size.div_ceil(64)], conflicts: Vec::new() }
    }

    /// Writes `data` at address `at`.
    pub fn put(&mut self, at: u32, data: &[u8]) {
        for (i, &b) in data.iter().enumerate() {
            let a = at as usize + i;
            if a >= self.bytes.len() {
                panic!("write outside the image at {a:#x}");
            }
            let known = self.known[a / 64] >> (a % 64) & 1 != 0;
            if known && self.bytes[a] != b {
                self.conflicts.push(a as u32);
            }
            self.bytes[a] = b;
            self.known[a / 64] |= 1 << (a % 64);
        }
    }

    pub fn put16(&mut self, at: u32, v: u16) {
        self.put(at, &v.to_be_bytes());
    }

    pub fn put32(&mut self, at: u32, v: u32) {
        self.put(at, &v.to_be_bytes());
    }

    pub fn put_words(&mut self, at: u32, words: &[u16]) {
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_be_bytes()).collect();
        self.put(at, &bytes);
    }

    pub fn is_covered(&self, a: u32) -> bool {
        let a = a as usize;
        a < self.bytes.len() && self.known[a / 64] >> (a % 64) & 1 != 0
    }

    /// Covered zones `(start, end exclusive)`, sorted, non-overlapping.
    pub fn covered(&self) -> Vec<(u32, u32)> {
        let mut out: Vec<(u32, u32)> = Vec::new();
        for a in 0..self.bytes.len() as u32 {
            if self.is_covered(a) {
                match out.last_mut() {
                    Some(r) if r.1 == a => r.1 += 1,
                    _ => out.push((a, a + 1)),
                }
            }
        }
        out
    }

    /// Covered bytes.
    pub fn covered_len(&self) -> usize {
        self.known.iter().map(|w| w.count_ones() as usize).sum()
    }
}

/// Program ROM: data view (68000 reads) and decrypted program-space view (opcodes and
/// PC-relative reads), like `cps2::load_program`.
pub struct Program {
    pub data: Image,
    pub prog: Image,
    /// Files missing from the assets folder (their tables are not rebuilt).
    pub missing: Vec<String>,
}

impl Default for Program {
    fn default() -> Self {
        Program { data: Image::new(PROGRAM_SIZE), prog: Image::new(PROGRAM_SIZE), missing: Vec::new() }
    }
}

impl Program {
    /// Rebuilds all the tables whose file exists in `dir`.
    pub fn load(dir: &Path) -> io::Result<Self> {
        Self::load_files(dir, &|_| true)
    }

    /// Files (paths relative to `dir`) used to rebuild the program.
    pub const FILES: [&'static str; 24] = [
        "palettes.png",
        "sprites/frames.json",
        "sprites/anims.json",
        "sprites/tables.json",
        "levels.json",
        "terrain.json",
        "scene_palettes.json",
        "texts.json",
        "data/spawns.json",
        "data/speeds.json",
        "data/players.json",
        "data/combat.json",
        "data/enemies.json",
        "data/boxes.json",
        "data/scenes.json",
        "data/misc.json",
        "data/dialogs.json",
        "data/rest.json",
        "data/natives.json",
        "data/set.json",
        "data/screens.json",
        "data/reste.json",
        "data/effects.json",
        "data/objects.json",
    ];

    /// Like `load`, but only the files that `want` accepts are written (the sprite frames are read
    /// anyway: the animations and the lists refer to them).
    pub fn load_files(dir: &Path, want: &dyn Fn(&str) -> bool) -> io::Result<Self> {
        let mut p = Program::default();
        let need = |p: &mut Program, rel: &str| {
            if !want(rel) {
                return false;
            }
            let ok = dir.join(rel).exists();
            if !ok {
                p.missing.push(rel.to_string());
            }
            ok
        };
        if need(&mut p, "palettes.png") {
            p.put_palettes(&Palettes::load(&dir.join("palettes.png"))?);
        }
        let frames_file = dir.join("sprites/frames.json");
        let frames = if frames_file.exists() { Some(Frames::load(&frames_file)?) } else { None };
        match &frames {
            Some(f) => {
                if need(&mut p, "sprites/frames.json") {
                    p.put_frames(f);
                }
                if need(&mut p, "sprites/anims.json") {
                    p.put_anims(&Anims::load(&dir.join("sprites/anims.json"))?, f);
                }
                if need(&mut p, "sprites/tables.json") {
                    p.put_sprite_tables(&SpriteTables::load(&dir.join("sprites/tables.json"))?, f);
                }
            }
            None => p.missing.push("sprites/frames.json".to_string()),
        }
        if need(&mut p, "levels.json") {
            let l = Levels::load(&dir.join("levels.json"))?;
            p.put_levels(&l);
            p.put_scroll1(&l.scroll1);
        }
        if need(&mut p, "terrain.json") {
            p.put_terrain(&Terrain::load(&dir.join("terrain.json"))?);
        }
        if need(&mut p, "scene_palettes.json") {
            p.put_scene_palettes(&ScenePalettes::load(&dir.join("scene_palettes.json"))?);
        }
        if need(&mut p, "texts.json") {
            p.put_texts(&Texts::load(&dir.join("texts.json"))?);
        }
        if need(&mut p, "data/spawns.json") {
            p.put_spawns(&Spawns::load(&dir.join("data/spawns.json"))?);
        }
        if need(&mut p, "data/speeds.json") {
            p.put_speeds(&Speeds::load(&dir.join("data/speeds.json"))?);
        }
        if need(&mut p, "data/effects.json") {
            p.put_effects(&ddsom_data::EffectTables::load(&dir.join("data/effects.json"))?);
        }
        if need(&mut p, "data/objects.json") {
            p.put_named(&ddsom_data::ObjectTables::load(&dir.join("data/objects.json"))?.tables());
        }
        for rel in &Self::FILES[10..21] {
            if need(&mut p, rel) {
                p.put_data(&DataFile::load(&dir.join(rel))?).map_err(io::Error::other)?;
            }
        }
        // last: the remainder completes what the other files describe
        if need(&mut p, "data/reste.json") {
            p.put_rest(&Remainder::load(&dir.join("data/reste.json"))?).map_err(io::Error::other)?;
        }
        Ok(p)
    }

    fn image(&mut self, program: bool) -> &mut Image {
        if program { &mut self.prog } else { &mut self.data }
    }

    /// Palette library (`palettes.png`): 0x253000-0x293000.
    pub fn put_palettes(&mut self, p: &Palettes) {
        let words: Vec<u16> = p.0.iter().flatten().copied().collect();
        self.data.put_words(0x253000, &words);
    }

    /// Table of frame addresses and frames (header, pieces), plus their offset tables; then the frames outside the
    /// table.
    pub fn put_frames(&mut self, f: &Frames) {
        for (i, fr) in f.frames.iter().enumerate() {
            self.data.put32(f.table + 4 * i as u32, fr.rom);
            self.put_frame(fr);
        }
        for fr in &f.extra {
            self.put_frame(fr);
        }
    }

    fn put_frame(&mut self, fr: &Frame) {
        let mut bytes = Vec::new();
        bytes.extend(fr.cost.to_be_bytes());
        bytes.extend(fr.attr.to_be_bytes());
        bytes.extend([fr.bank, fr.mode]);
        bytes.extend(fr.offsets.to_be_bytes());
        let mut table = Vec::new();
        for p in &fr.pieces {
            bytes.extend(p.code().to_be_bytes());
            if fr.per_piece() {
                bytes.extend(p.attr().to_be_bytes());
            }
            table.extend(p.dx().to_be_bytes());
            table.extend(p.dy().to_be_bytes());
        }
        self.data.put(fr.rom, &bytes);
        self.data.put(fr.offsets, &table);
    }

    /// Viewer families, animation sets and animation entries.
    pub fn put_anims(&mut self, a: &Anims, frames: &Frames) {
        // Names, per-family lists of sets (stored right after the variants table) and number of
        // variants (low byte of the entry; the three high bytes are not data).
        let mut list = a.variants_rom + 4 * a.families.len() as u32 - a.families_rom;
        for (i, fam) in a.families.iter().enumerate() {
            let i = i as u32;
            self.data.put(a.names_rom + 4 * i, fam.name.as_bytes());
            self.data.put16(a.families_rom + 2 * i, list as u16);
            self.data.put(a.variants_rom + 4 * i + 3, &[4 * (fam.sets.len() as u32 - 1) as u8]);
            for (v, &s) in fam.sets.iter().enumerate() {
                self.data.put32(a.families_rom + list + 4 * v as u32, s);
            }
            list += 4 * fam.sets.len() as u32;
        }
        for s in &a.sets {
            for (k, &o) in s.scripts.iter().enumerate() {
                self.data.put16(s.rom + 2 * k as u32, o as u16);
            }
        }
        for e in &a.entries {
            let mut b = Vec::new();
            b.extend(e.time.to_be_bytes());
            b.extend(e.flags.to_be_bytes());
            b.extend(frames.number(e.frame).rom.to_be_bytes());
            b.extend(&e.data);
            if let Some(n) = e.next {
                b.extend(n.to_be_bytes());
            }
            self.data.put(e.rom, &b);
        }
        for p in &a.character_sets {
            self.put_pointers(p);
        }
    }

    /// Array of addresses in the program space (stored in the code).
    pub fn put_pointers(&mut self, p: &Pointers) {
        for (i, &t) in p.targets.iter().enumerate() {
            self.prog.put32(p.rom + 4 * i as u32, t);
        }
    }

    /// Speed tables (data space) and their arrays (program space).
    pub fn put_speeds(&mut self, s: &Speeds) {
        for a in &s.arrays {
            self.put_pointers(a);
        }
        for t in &s.tables {
            let words: Vec<u16> = t.speeds.iter().flat_map(|&(vx, vy)| [vx as u16, vy as u16]).collect();
            self.data.put_words(t.rom, &words);
        }
    }

    /// Tables of the EFFCT family, each in its space.
    pub fn put_effects(&mut self, e: &ddsom_data::EffectTables) {
        self.put_named(&e.tables());
    }

    /// Named tables (`ddsom_data::effects::tables!`: effects, objects), each in its space.
    pub fn put_named(&mut self, tables: &[&ddsom_data::effects::Table]) {
        for t in tables {
            let image = if t.program { &mut self.prog } else { &mut self.data };
            image.put(t.rom, &t.bytes());
        }
    }

    /// Scroll 2 and 3: block banks, strips, strip tables, scene pointers (program space).
    pub fn put_levels(&mut self, levels: &Levels) {
        for l in &levels.layers {
            for (b, bank) in l.banks.iter().enumerate() {
                self.data.put32(l.banks_rom + 4 * b as u32, bank.rom);
                let words: Vec<u16> = bank.blocks.iter().flatten().copied().collect();
                self.data.put_words(bank.rom, &words);
            }
            let words: Vec<u16> = l.strips.iter().flatten().copied().collect();
            self.data.put_words(l.strips_rom, &words);
            for t in &l.tables {
                self.data.put16(t.rom, t.width() as u16);
                let words: Vec<u16> = t.rows.iter().flatten().copied().collect();
                self.data.put_words(t.rom + 2, &words);
            }
            for (s, &t) in l.scenes.iter().enumerate() {
                self.prog.put32(l.scenes_rom + 4 * s as u32, l.tables[t].pointer());
            }
        }
    }

    /// Scroll 1: grids (padded with zeros up to the blocks) and blocks.
    pub fn put_scroll1(&mut self, s: &crate::levels::Scroll1) {
        let mut words: Vec<u16> = s.grids.iter().flatten().copied().collect();
        words.resize((s.blocks_rom - s.grids_rom) as usize / 2, 0);
        self.data.put_words(s.grids_rom, &words);
        let words: Vec<u16> = s.blocks.iter().flatten().copied().collect();
        self.data.put_words(s.blocks_rom, &words);
    }

    /// Fixed terrain maps and terrain types.
    pub fn put_terrain(&mut self, t: &Terrain) {
        for m in &t.maps {
            let words: Vec<u16> = m.columns.iter().flatten().copied().collect();
            self.data.put_words(m.rom, &words);
        }
        for k in &t.kinds {
            let words: Vec<u16> = k.kinds.iter().flat_map(|e| e.words()).collect();
            self.data.put_words(k.rom, &words);
        }
    }

    /// Palettes of each scene: page 0, added palettes, sets and pages 1 to 3.
    pub fn put_scene_palettes(&mut self, p: &ScenePalettes) {
        let words: Vec<u16> = p.stages.iter().flatten().map(|&v| ScenePalettes::stage_word(v)).collect();
        self.data.put_words(p.stages_rom, &words);
        for t in [&p.extra, &p.sets].into_iter().chain(p.pages.iter()) {
            for (i, &a) in t.entries.iter().enumerate() {
                self.image(t.program).put32(t.rom + 4 * i as u32, a);
            }
        }
    }

    /// Boxes (sets, blocks, visibility), ground marks, frame lists and ESHL 5 shot.
    pub fn put_sprite_tables(&mut self, t: &SpriteTables, frames: &Frames) {
        for bt in &t.box_tables {
            for (i, &a) in bt.sets.iter().enumerate() {
                self.data.put32(bt.rom + 4 * i as u32, a);
            }
        }
        for b in &t.box_blocks {
            let mut bytes: Vec<u8> = b.boxes.iter().flat_map(|x| x.bytes()).collect();
            bytes.extend(&b.tail);
            self.data.put(b.rom, &bytes);
        }
        let words: Vec<u16> = t.bounds.iter().flat_map(|b| [b.0 as u16, b.1, b.2 as u16, b.3]).collect();
        self.data.put_words(t.bounds_rom, &words);
        let words: Vec<u16> = t.marks.iter().flat_map(|m| m.words()).collect();
        self.data.put_words(t.marks_rom, &words);
        for l in &t.frame_lists {
            for (i, &f) in l.frames.iter().enumerate() {
                self.data.put32(l.rom + 4 * i as u32, frames.frames[f as usize].rom);
            }
        }
        let s5 = &t.shot5;
        let bytes: Vec<u8> = s5.boxes.iter().flatten().copied().collect();
        self.prog.put(s5.boxes_rom, &bytes);
        for (base, offs) in [(s5.even_rom, &s5.even), (s5.odd_rom, &s5.odd)] {
            for (i, &d) in offs.iter().enumerate() {
                self.prog.put16(base + 2 * i as u32, d as u16);
            }
        }
        let bytes: Vec<u8> = s5.mirrored.iter().flatten().copied().collect();
        self.data.put(s5.mirrored_rom, &bytes);
    }

    /// Texts of the 8x8 layer, text tables, popup names and `@`-terminated names.
    pub fn put_texts(&mut self, t: &Texts) {
        for d in &t.texts {
            self.data.put(d.rom, &d.bytes());
        }
        for tb in &t.tables {
            for (i, &e) in tb.entries.iter().enumerate() {
                if tb.offsets {
                    self.image(tb.program).put16(tb.rom + 2 * i as u32, e.wrapping_sub(tb.rom) as u16);
                } else {
                    self.image(tb.program).put32(tb.rom + 4 * i as u32, e);
                }
            }
        }
        let p = &t.popups;
        for (i, &o) in p.offsets.iter().enumerate() {
            self.prog.put16(p.table + 2 * i as u32, o as u16);
        }
        let mut bytes = Vec::new();
        for s in &p.strings {
            bytes.extend(texts::decode(s));
            bytes.push(b'/');
        }
        self.data.put(p.strings_rom, &bytes);
        for g in &t.names {
            for (i, &e) in g.entries.iter().enumerate() {
                self.image(g.program).put32(g.table + 4 * i as u32, e);
            }
            let bytes: Vec<u8> = g
                .names
                .iter()
                .flat_map(|n| {
                    let mut b = texts::decode(n);
                    b.push(b'@');
                    b
                })
                .collect();
            self.data.put(g.rom, &bytes);
        }
    }

    /// Spawn scripts of the scenes.
    pub fn put_spawns(&mut self, s: &Spawns) {
        for (i, &a) in s.scenes.iter().enumerate() {
            self.data.put32(s.table + 4 * i as u32, a);
        }
        for sc in &s.scripts {
            self.data.put(sc.rom, &sc.bytes());
        }
    }

    /// The remainder of the program region (`data/reste.json`): its blocks, then the program space outside the
    /// encrypted zone, copied from the data space (it is equal to it).
    pub fn put_rest(&mut self, r: &Remainder) -> Result<(), String> {
        for b in &r.data {
            self.data.put(b.start, &b.bytes()?);
        }
        for b in &r.program {
            self.prog.put(b.start, &b.bytes()?);
        }
        for a in r.encrypted_end..PROGRAM_SIZE as u32 {
            if self.data.is_covered(a) && !self.prog.is_covered(a) {
                let v = self.data.bytes[a as usize];
                self.prog.put(a, &[v]);
            }
        }
        Ok(())
    }

    /// Data tables (`data/*.json`) and their texts.
    pub fn put_data(&mut self, f: &DataFile) -> Result<(), String> {
        for t in &f.tables {
            let bytes = t.bytes()?;
            self.image(t.program).put(t.rom, &bytes);
        }
        for t in &f.texts {
            self.data.put(t.rom, &t.bytes());
        }
        Ok(())
    }
}

/// Graphics region (16x16 cells in hardware format: 4 planes per group of 8 pixels, 128 bytes per
/// cell), as `cps2_gfx::rom::load_gfx` builds it from the ROMs. Only for the lossless tests: the game reads the
/// cells decoded once (`crate::tile_atlas`).
pub fn gfx_region(dir: &Path) -> io::Result<Vec<u8>> {
    Ok(Cells::load(dir)?.to_region())
}

impl Cells {
    /// Bytes of cell `c` in hardware format.
    pub fn encode(&self, c: usize) -> [u8; 128] {
        let mut bytes = [0u8; 128];
        for y in 0..16 {
            for x in 0..16 {
                let p = self.pixel(c, x, y);
                for plane in 0..4 {
                    bytes[y * 8 + (x / 8) * 4 + plane] |= ((p >> plane) & 1) << (7 - x % 8);
                }
            }
        }
        bytes
    }

    /// The whole graphics region.
    pub fn to_region(&self) -> Vec<u8> {
        (0..self.len()).flat_map(|c| self.encode(c)).collect()
    }
}

/// Z80 region (`cps2::sound::Sound::rom`) with the driver data of `sound/music.json` at their original
/// addresses (`ddsom_sound::SoundData::region_parts`). The Z80 program is not in it: only the Rust driver
/// can play it.
pub fn audio_region(dir: &Path) -> io::Result<Image> {
    let m = Music::load(&dir.join("sound/music.json"))?;
    let mut img = Image::new(ddsom_sound::region::REGION_SIZE);
    for (at, bytes) in m.data.region_parts() {
        for (k, &b) in bytes.iter().enumerate() {
            img.put(ddsom_sound::region::offset(at + k as u32) as u32, &[b]);
        }
    }
    Ok(img)
}

/// QSound DSP program as `qsound::QSound::new` reads it (0x2000 bytes, little-endian words): only the
/// tables of `sound/qsound_dsp.json` are filled, the code (which the high-level emulation replaces) is zero.
pub fn qsound_dsp(dir: &Path) -> io::Result<Vec<u8>> {
    let t = crate::music::DspTables::load(&dir.join("sound/qsound_dsp.json"))?;
    let mut bytes = vec![0u8; 0x2000];
    for table in &t.tables {
        for (k, v) in table.values.iter().enumerate() {
            let a = 2 * (table.rom as usize + k);
            bytes[a..a + 2].copy_from_slice(&v.to_le_bytes());
        }
    }
    Ok(bytes)
}

/// Big-endian words of a program ROM image (view of `cps2::Cps2Bus`).
fn words(img: &Image) -> Vec<u16> {
    img.bytes.as_chunks::<2>().0.iter().map(|&w| u16::from_be_bytes(w)).collect()
}

/// Everything the machine stores in its chips, rebuilt from the assets folder, without any ROM: 68000
/// program (only its tables: the game must run without the 68000, `ddsom_game::install_no_68k`), graphics,
/// Z80 region (without its program: Rust sound driver mandatory), QSound DSP tables, samples.
pub fn contents(dir: &Path) -> io::Result<cps2::Contents> {
    let p = Program::load(dir)?;
    if !p.missing.is_empty() {
        return Err(io::Error::new(io::ErrorKind::NotFound, format!("incomplete assets: {}", p.missing.join(", "))));
    }
    Ok(cps2::Contents {
        program: words(&p.data),
        decrypted_program: words(&p.prog),
        graphics: crate::tile_atlas(&dir.join("gfx"))?,
        audio: audio_region(dir)?.bytes,
        dsp: qsound_dsp(dir)?,
        samples: crate::samples::bank(dir)?.bytes,
    })
}

/// First address where `img` and `rom` differ on the bytes covered by `img` (for tests and measurements).
pub fn first_difference(img: &Image, rom: &[u8]) -> Option<u32> {
    (0..img.bytes.len() as u32).find(|&a| img.is_covered(a) && img.bytes[a as usize] != rom[a as usize])
}

/// Merges read addresses into zones: `(start, end exclusive)` per 2-byte word.
pub fn merge_ranges(mut v: Vec<(u32, u32)>) -> Vec<(u32, u32)> {
    v.sort();
    let mut out: Vec<(u32, u32)> = Vec::new();
    for (s, e) in v {
        match out.last_mut() {
            Some(r) if s <= r.1 => r.1 = r.1.max(e),
            _ => out.push((s, e)),
        }
    }
    out
}

/// Bytes of `ranges` not covered by `img`, merged into zones.
pub fn uncovered(img: &Image, ranges: &[(u32, u32)]) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = Vec::new();
    for &(s, e) in ranges {
        for a in s..e {
            if !img.is_covered(a) {
                match out.last_mut() {
                    Some(r) if r.1 == a => r.1 += 1,
                    _ => out.push((a, a + 1)),
                }
            }
        }
    }
    out
}
