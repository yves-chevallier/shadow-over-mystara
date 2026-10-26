//! Reconstruction des contenus des ROMs à partir des ressources extraites : région graphique, ROM
//! d'échantillons QSound et tables de la ROM programme 68000, aux adresses d'origine.
//!
//! Chaque fonction `put_*` est l'inverse du chargement d'un fichier : elle réécrit ses tables telles
//! que la ROM les range (les tests de `tests/roundtrip.rs` comparent le résultat octet pour octet).
//! Seuls les octets décrits par les fichiers sont écrits : `Image::covered` donne les zones couvertes,
//! le reste de l'image vaut zéro.

use std::io;
use std::path::Path;

use crate::data::DataFile;
use crate::texts;
use crate::{
    Anims, Cells, Frames, Levels, Music, Palettes, Sample, ScenePalettes, Spawns, SpriteTables, Terrain, Texts,
};

/// Taille de la ROM programme (espace 0x000000-0x3fffff).
pub const PROGRAM_SIZE: usize = 0x40_0000;

/// Octets d'une zone de ROM, avec la liste des octets que les ressources décrivent.
pub struct Image {
    pub bytes: Vec<u8>,
    known: Vec<u64>,
    /// Adresses où deux écritures se contredisent (devrait rester vide).
    pub conflicts: Vec<u32>,
}

impl Image {
    pub fn new(size: usize) -> Self {
        Image { bytes: vec![0; size], known: vec![0; size.div_ceil(64)], conflicts: Vec::new() }
    }

    /// Écrit `data` à l'adresse `at`.
    pub fn put(&mut self, at: u32, data: &[u8]) {
        for (i, &b) in data.iter().enumerate() {
            let a = at as usize + i;
            if a >= self.bytes.len() {
                panic!("écriture hors de l'image en {a:#x}");
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

    /// Zones couvertes `(début, fin exclue)`, triées, sans chevauchement.
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

    /// Octets couverts.
    pub fn covered_len(&self) -> usize {
        self.known.iter().map(|w| w.count_ones() as usize).sum()
    }
}

/// ROM programme : vue données (lectures du 68000) et vue espace programme déchiffrée (opcodes et
/// lectures relatives au PC), comme `cps2::load_program`.
pub struct Program {
    pub data: Image,
    pub prog: Image,
    /// Fichiers absents du dossier d'assets (leurs tables ne sont pas reconstruites).
    pub missing: Vec<String>,
}

impl Default for Program {
    fn default() -> Self {
        Program { data: Image::new(PROGRAM_SIZE), prog: Image::new(PROGRAM_SIZE), missing: Vec::new() }
    }
}

impl Program {
    /// Reconstruit toutes les tables dont le fichier existe dans `dir`.
    pub fn load(dir: &Path) -> io::Result<Self> {
        let mut p = Program::default();
        let has = |rel: &str| dir.join(rel).exists();
        let need = |p: &mut Program, rel: &str| {
            let ok = has(rel);
            if !ok {
                p.missing.push(rel.to_string());
            }
            ok
        };
        if need(&mut p, "palettes.png") {
            p.put_palettes(&Palettes::load(&dir.join("palettes.png"))?);
        }
        let frames = if need(&mut p, "sprites/frames.json") { Some(Frames::load(&dir.join("sprites/frames.json"))?) } else { None };
        if let Some(f) = &frames {
            p.put_frames(f);
            if need(&mut p, "sprites/anims.json") {
                p.put_anims(&Anims::load(&dir.join("sprites/anims.json"))?, f);
            }
            if need(&mut p, "sprites/tables.json") {
                p.put_sprite_tables(&SpriteTables::load(&dir.join("sprites/tables.json"))?, f);
            }
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
        for name in ["players", "combat", "enemies", "boxes", "scenes", "misc", "dialogs"] {
            let rel = format!("data/{name}.json");
            if need(&mut p, &rel) {
                p.put_data(&DataFile::load(&dir.join(rel))?).map_err(io::Error::other)?;
            }
        }
        Ok(p)
    }

    fn image(&mut self, program: bool) -> &mut Image {
        if program { &mut self.prog } else { &mut self.data }
    }

    /// Bibliothèque de palettes (`palettes.png`) : 0x253000-0x293000.
    pub fn put_palettes(&mut self, p: &Palettes) {
        let words: Vec<u16> = p.0.iter().flatten().copied().collect();
        self.data.put_words(0x253000, &words);
    }

    /// Table des adresses d'images et images (en-tête, morceaux), plus leurs tables de décalages.
    pub fn put_frames(&mut self, f: &Frames) {
        for (i, fr) in f.frames.iter().enumerate() {
            self.data.put32(f.table + 4 * i as u32, fr.rom);
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
    }

    /// Familles du visualiseur, jeux d'animations et entrées d'animation.
    pub fn put_anims(&mut self, a: &Anims, frames: &Frames) {
        // Noms, listes de jeux par famille (rangées à la suite de la table des variantes) et nombre de
        // variantes (octet bas de l'entrée ; les trois octets hauts ne sont pas des données).
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
            b.extend(frames.frames[e.frame as usize].rom.to_be_bytes());
            b.extend(&e.data);
            if let Some(n) = e.next {
                b.extend(n.to_be_bytes());
            }
            self.data.put(e.rom, &b);
        }
    }

    /// Scroll 2 et 3 : banques de blocs, bandes, tables de bandes, pointeurs de scènes (espace programme).
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

    /// Scroll 1 : grilles (complétées de zéros jusqu'aux blocs) et blocs.
    pub fn put_scroll1(&mut self, s: &crate::levels::Scroll1) {
        let mut words: Vec<u16> = s.grids.iter().flatten().copied().collect();
        words.resize((s.blocks_rom - s.grids_rom) as usize / 2, 0);
        self.data.put_words(s.grids_rom, &words);
        let words: Vec<u16> = s.blocks.iter().flatten().copied().collect();
        self.data.put_words(s.blocks_rom, &words);
    }

    /// Cartes de terrain fixes et types de terrain.
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

    /// Palettes de chaque scène : page 0, palettes ajoutées, ensembles et pages 1 à 3.
    pub fn put_scene_palettes(&mut self, p: &ScenePalettes) {
        let words: Vec<u16> = p.stages.iter().flatten().map(|&v| ScenePalettes::stage_word(v)).collect();
        self.data.put_words(p.stages_rom, &words);
        for t in [&p.extra, &p.sets].into_iter().chain(p.pages.iter()) {
            for (i, &a) in t.entries.iter().enumerate() {
                self.image(t.program).put32(t.rom + 4 * i as u32, a);
            }
        }
    }

    /// Boîtes (jeux, blocs, visibilité), marques au sol, listes d'images et tir ESHL 5.
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

    /// Textes de la couche 8x8, tables de textes, noms des bulles et noms terminés par `@`.
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

    /// Scripts d'apparition des scènes.
    pub fn put_spawns(&mut self, s: &Spawns) {
        for (i, &a) in s.scenes.iter().enumerate() {
            self.data.put32(s.table + 4 * i as u32, a);
        }
        for sc in &s.scripts {
            self.data.put(sc.rom, &sc.bytes());
        }
    }

    /// Tables de données (`data/*.json`) et leurs textes.
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

/// Région graphique (cellules 16x16 au format matériel : 4 plans par groupe de 8 pixels, 128 octets par
/// cellule), telle que `cps2_gfx::rom::load_gfx` la construit depuis les ROMs.
pub fn gfx_region(dir: &Path) -> io::Result<Vec<u8>> {
    Ok(Cells::load(dir)?.to_region())
}

impl Cells {
    /// Octets de la cellule `c` au format matériel.
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

    /// Toute la région graphique.
    pub fn to_region(&self) -> Vec<u8> {
        (0..self.len()).flat_map(|c| self.encode(c)).collect()
    }
}

/// Taille de la ROM d'échantillons QSound (`dd2.11m` et `dd2.12m`).
pub const SAMPLES_SIZE: usize = 0x40_0000;

/// ROM d'échantillons : les zones décrites par la table des échantillons du pilote (`music.json`) et les
/// fichiers `sounds/NNN.wav` et ce qui suit chaque échantillon (`Music::sample_tails`), le reste à zéro. `Image::covered` donne les zones remplies.
pub fn qsound_samples(dir: &Path) -> io::Result<Image> {
    let music = Music::load(&dir.join("sound/music.json"))?;
    let mut img = Image::new(SAMPLES_SIZE);
    for i in 0..music.samples.len() {
        let path = dir.join(format!("sounds/{i:03}.wav"));
        if !path.exists() || !music.sample_valid(i) {
            continue;
        }
        let pcm: Vec<u8> = Sample::load(&path)?.pcm.iter().map(|&v| v as u8).collect();
        let (start, end) = music.sample_span(i);
        img.put(start as u32, &pcm);
        // la suite jusqu'au prochain échantillon (zéros omis dans le fichier)
        let mut tail = vec![0; music.sample_tail_end(i) - end - 1];
        if let Some(hex) = music.sample_tails.get(i) {
            for (k, b) in tail.iter_mut().zip(hex.as_bytes().chunks(2)) {
                *k = u8::from_str_radix(std::str::from_utf8(b).unwrap_or("00"), 16).unwrap_or(0);
            }
        }
        img.put(end as u32 + 1, &tail);
    }
    Ok(img)
}

/// Zones de `a` et `b` qui diffèrent sur les octets couverts de `img` (pour les tests et les mesures).
pub fn first_difference(img: &Image, rom: &[u8]) -> Option<u32> {
    (0..img.bytes.len() as u32).find(|&a| img.is_covered(a) && img.bytes[a as usize] != rom[a as usize])
}

/// Regroupe des adresses lues en zones : `(début, fin exclue)` par mot de 2 octets.
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

/// Octets de `ranges` non couverts par `img`, regroupés en zones.
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
