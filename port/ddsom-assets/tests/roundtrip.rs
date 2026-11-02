//! The extracted resources give back exactly the data of the ROMs.
//! Skipped if ../../assets or the ROMs are missing (run `extract <roms> assets` at the root). The ROMs
//! are read from `DDSOM_ROMS`, otherwise from ../../rom.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ddsom_assets::data::{Cell, DataFile, Kind};
use ddsom_assets::rebuild::{self, Program};
use ddsom_assets::{Anims, Cells, Frames, Levels, Music, Palettes, Sample, ScenePalettes, Spawns, SpriteTables, Terrain, Texts};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Resources folder: `DDSOM_ASSETS`, otherwise `assets/` at the root.
fn assets() -> PathBuf {
    std::env::var_os("DDSOM_ASSETS").map(PathBuf::from).unwrap_or_else(|| root().join("assets"))
}

fn roms() -> PathBuf {
    std::env::var_os("DDSOM_ROMS").map(PathBuf::from).unwrap_or_else(|| root().join("rom"))
}

fn available() -> bool {
    assets().join("palettes.png").exists() && roms().join("dd2.13m").exists()
}

/// The ROMs and the extracted file `rel` are present.
fn has(rel: &str) -> bool {
    let ok = assets().join(rel).exists() && roms().join("dd2.13m").exists();
    if !ok {
        eprintln!("{rel} or the ROMs missing: test skipped");
    }
    ok
}

/// Program ROM: data (68000 reads) and decrypted program space, as bytes.
fn program() -> (Vec<u8>, Vec<u8>) {
    let (data, prog) = cps2::load_program(&roms()).unwrap();
    let bytes = |w: Vec<u16>| w.iter().flat_map(|v| v.to_be_bytes()).collect();
    (bytes(data), bytes(prog))
}

/// The tables that `put` rewrites give back the ROM, byte for byte, on the zones they describe.
fn check(what: &str, put: impl FnOnce(&mut Program)) {
    let mut p = Program::default();
    put(&mut p);
    let (data, prog) = program();
    for (name, img, rom) in [("data", &p.data, &data), ("program", &p.prog, &prog)] {
        assert!(img.conflicts.is_empty(), "{what}: contradictory writes at {:#x}", img.conflicts[0]);
        if let Some(a) = rebuild::first_difference(img, rom) {
            panic!("{what}: {name} space, byte {a:#x}: {:#04x} instead of {:#04x}", img.bytes[a as usize], rom[a as usize]);
        }
    }
}

#[test]
fn graphics_lossless() {
    if !available() {
        return;
    }
    let cells = Cells::load(&assets().join("gfx")).unwrap();
    let rom = cps2_gfx::rom::load_gfx(&roms(), cps2_gfx::rom::DDSOM_GFX, cps2_gfx::rom::DDSOM_GFX_SIZE).unwrap();
    assert_eq!(cells.len() * 128, rom.len());
    // each cell re-encoded in hardware format (4 planes per group of 8 pixels)
    for c in 0..cells.len() {
        assert_eq!(&cells.encode(c)[..], &rom[c * 128..][..128], "cell {c}");
    }
}

#[test]
fn palettes_lossless() {
    if !available() {
        return;
    }
    let pals = Palettes::load(&assets().join("palettes.png")).unwrap();
    assert_eq!(pals.0.len() * 32, 0x293000 - 0x253000);
    check("palettes", |p| p.put_palettes(&pals));
}

#[test]
fn sounds_lossless() {
    if !available() {
        return;
    }
    let rom = roms();
    let z80 = std::fs::read(rom.join("dd2.01")).unwrap();
    let mut pcm = Vec::new();
    for name in ["dd2.11m", "dd2.12m"] {
        pcm.extend(std::fs::read(rom.join(name)).unwrap().chunks_exact(2).flat_map(|w| [w[1], w[0]]));
    }
    let mut count = 0;
    for i in 0..345 {
        let e = &z80[0x49e8 + 8 * i..][..8];
        let word = |o: usize| u16::from_le_bytes([e[o], e[o + 1]]) as usize;
        let (base, start, loop_start, end) = ((e[1] as usize) << 16, word(2), word(4), word(6));
        if !(e[1] < 0x40 && start <= end && (start..=end).contains(&loop_start)) {
            assert!(!assets().join(format!("sounds/{i:03}.wav")).exists(), "invalid record {i}: file");
            continue;
        }
        count += 1;
        let s = Sample::load(&assets().join(format!("sounds/{i:03}.wav"))).unwrap();
        let expect: Vec<i8> = pcm[base + start..=base + end].iter().map(|&b| b as i8).collect();
        assert_eq!(s.pcm, expect, "sample {i}");
        assert_eq!(s.root, e[0], "note of sample {i}");
        assert_eq!(s.loop_start, (loop_start < end).then_some(loop_start - start), "loop of sample {i}");
    }
    assert_eq!(count, 342, "valid records");
}

#[test]
fn sprite_frames_lossless() {
    if !has("sprites/frames.json") {
        return;
    }
    let frames = Frames::load(&assets().join("sprites/frames.json")).unwrap();
    // address table, frames and offset tables
    check("sprite frames", |p| p.put_frames(&frames));
    // the frames follow each other without gaps, then the (shared) offset tables, also without gaps
    let start = frames.frames[0].rom;
    let mut next = start;
    let mut offsets: BTreeMap<u32, usize> = BTreeMap::new();
    for f in &frames.frames {
        assert_eq!(next, f.rom, "frame {:#x}: position", f.rom);
        let n = if f.per_piece() { f.attr } else { f.cost } as usize;
        assert_eq!(f.pieces.len(), n, "frame {:#x}: number of pieces", f.rom);
        if !f.per_piece() {
            assert!(f.pieces.iter().all(|p| p.attr() == f.attr), "frame {:#x}: common attributes", f.rom);
        }
        let piece = if f.per_piece() { 4 } else { 2 };
        next += 10 + piece * n as u32;
        let len = offsets.entry(f.offsets).or_default();
        *len = (*len).max(4 * n);
    }
    let first = *offsets.keys().next().unwrap();
    assert_eq!(next, first, "the offsets follow the frames");
    let mut at = first;
    for (&a, &len) in &offsets {
        assert_eq!(at, a, "offset table {a:#x}: position");
        at += len as u32;
    }
}

#[test]
fn animations_lossless() {
    if !has("sprites/anims.json") || !has("sprites/frames.json") {
        return;
    }
    let frames = Frames::load(&assets().join("sprites/frames.json")).unwrap();
    let anims = Anims::load(&assets().join("sprites/anims.json")).unwrap();
    check("animations", |p| p.put_anims(&anims, &frames));
    for e in &anims.entries {
        assert_eq!(e.data.len(), if e.long { 16 } else { 8 });
        assert_eq!(e.next.is_some(), !matches!(e.flags >> 8, 0 | 0xfe));
    }
}

#[test]
fn scenery_lossless() {
    if !has("levels.json") {
        return;
    }
    let levels = Levels::load(&assets().join("levels.json")).unwrap();
    // banks, strips, strip tables and scene pointers (program space)
    check("scenery", |p| p.put_levels(&levels));
    let (rom, _) = program();
    let at = |a: u32, n: usize| &rom[a as usize..a as usize + n];
    let be32 = |m: &[u8], a: u32| u32::from_be_bytes(m[a as usize..a as usize + 4].try_into().unwrap());
    for l in &levels.layers {
        // strip tables: contiguous, ending at the bank table
        let mut next = l.tables[0].rom;
        for t in &l.tables {
            assert_eq!(t.rom, next, "{} table {:#x}: position", l.name, t.rom);
            assert!(t.rows.iter().all(|r| r.len() == t.width()));
            next = t.rom + 2 + 2 * (t.rows.len() * t.width()) as u32;
        }
        assert_eq!(next, l.banks_rom, "{}: end of the tables", l.name);
        // cells: same read as the game's cameras (`map_cell` of scroll.rs, `case_a` of cards.rs)
        let rd16 = |a: u32| u16::from_be_bytes(at(a, 2).try_into().unwrap());
        let n = l.strip_cases();
        let column = if l.case == 16 { 0x40 } else { 0x20 };
        for strip in 0..l.strips.len() {
            for cx in 0..n {
                for cy in 0..n {
                    let off = (strip * n * n * 4 + cx * column + cy * 4) as u32;
                    let c = off >> 2;
                    let (bank, index, sub) = if l.case == 16 {
                        let bank = be32(&rom, l.banks_rom + (off >> 16 & 0xc));
                        (bank, rd16(l.strips_rom + (c & 0xe) + (c >> 1 & 0x3fff0)) as u32, (off as u16 >> 3 & 8) as u32 + (off & 4))
                    } else {
                        let bank = be32(&rom, l.banks_rom + ((off >> 16 & 0xf) << 2));
                        (bank, rd16(l.strips_rom + (c & 6) + (c >> 1 & !7)) as u32, (off as u16 >> 2 & 8) as u32 + (off & 4))
                    };
                    let v = be32(&rom, bank + (index << 4) + sub);
                    let expect = ((v >> 16) as u16 + l.tile_offset, v as u16);
                    assert_eq!(l.case_at(strip, cx, cy), expect, "{} strip {strip} cell ({cx}, {cy})", l.name);
                }
            }
        }
    }
}

#[test]
fn scroll1_lossless() {
    if !has("levels.json") {
        return;
    }
    let s1 = Levels::load(&assets().join("levels.json")).unwrap().scroll1;
    check("scroll 1", |p| p.put_scroll1(&s1));
    let (rom, _) = program();
    assert!(s1.grids.iter().all(|g| g.len() == 64));
    assert!(s1.blocks.iter().all(|b| b.len() == 32));
    // the blocks end where the scroll 2 strips begin
    let end = s1.blocks_rom as usize + 64 * s1.blocks.len();
    assert_eq!(end, 0x1f9ba0, "the scroll 1 blocks precede the scroll 2 strips");
    // cells: same computation as 0x260c (destination A1 in columns of 0x80 bytes, 4-byte cells)
    let be32 = |a: usize| u32::from_be_bytes(rom[a..a + 4].try_into().unwrap());
    for g in 0..s1.grids.len() {
        let mut layer = vec![0u32; 32 * 32];
        let mut a2 = s1.grids_rom as usize + 0x80 * g;
        let mut a1 = 0usize;
        for _ in 0..8 {
            for _ in 0..8 {
                let mut a3 = s1.blocks_rom as usize + 64 * u16::from_be_bytes(rom[a2..a2 + 2].try_into().unwrap()) as usize;
                a2 += 2;
                for col in 0..4 {
                    for row in 0..4 {
                        layer[(a1 + 0x80 * col + 4 * row) / 4] = be32(a3).wrapping_add(0xf800_0000);
                        a3 += 4;
                    }
                }
                a1 += 0x10;
            }
            a1 += 0x180;
        }
        for cx in 0..32 {
            for cy in 0..32 {
                let v = layer[cx * 32 + cy];
                assert_eq!(s1.case_at(g, cx, cy), ((v >> 16) as u16, v as u16), "image {g} cell ({cx}, {cy})");
            }
        }
    }
}

#[test]
fn terrain_lossless() {
    if !has("terrain.json") {
        return;
    }
    let t = Terrain::load(&assets().join("terrain.json")).unwrap();
    let (rom, _) = program();
    let at = |a: u32, n: usize| &rom[a as usize..a as usize + n];
    check("terrain", |p| p.put_terrain(&t));
    for m in &t.maps {
        // read of 0x24b0: row 0x1e - ((y >> 3) & 0x1e), column (2 x) & 0xffe0
        for x in (0..32 * m.columns.len() as u16 / 2).step_by(5) {
            for y in (0..0x200u16).step_by(7) {
                let y2 = y.wrapping_add(m.dy as u16);
                let off = (0x1e - ((y2 as u8 >> 3) & 0x1e) as u16).wrapping_add(x.wrapping_add(x) & 0xffe0);
                let a = m.rom.wrapping_add(off as i16 as u32);
                let w = u16::from_be_bytes(at(a, 2).try_into().unwrap());
                assert_eq!(m.word(x, y), Some(w), "map {:#x} ({x}, {y})", m.rom);
            }
        }
    }
    // the two maps follow each other; the second is followed by the end-of-ROM padding
    let end = t.maps[1].rom as usize + 32 * t.maps[1].columns.len();
    assert_eq!(t.maps[0].rom as usize + 32 * t.maps[0].columns.len(), t.maps[1].rom as usize);
    assert!(rom[end..end + 64].iter().all(|&b| b == 0xff));
    assert_eq!(t.kinds[0].rom + 16 * t.kinds[0].kinds.len() as u32, t.kinds[1].rom);
}

#[test]
fn scene_palettes_lossless() {
    if !has("scene_palettes.json") {
        return;
    }
    let p = ScenePalettes::load(&assets().join("scene_palettes.json")).unwrap();
    check("scene palettes", |q| q.put_scene_palettes(&p));
    for t in [&p.extra, &p.sets].into_iter().chain(p.pages.iter()) {
        // all the designated palettes are in the library (except null entries)
        for &a in t.entries.iter().filter(|&&a| a != 0) {
            let i = p.library_index(a).unwrap_or_else(|| panic!("table {:#x}: {a:#x} outside the library", t.rom));
            assert!(i + t.count as usize <= 8192, "table {:#x}: {a:#x}", t.rom);
        }
    }
}

#[test]
fn object_tables_lossless() {
    if !has("sprites/tables.json") || !has("sprites/frames.json") {
        return;
    }
    let t = SpriteTables::load(&assets().join("sprites/tables.json")).unwrap();
    let frames = Frames::load(&assets().join("sprites/frames.json")).unwrap();
    check("object tables", |p| p.put_sprite_tables(&t, &frames));
    let (rom, _) = program();
    let at = |a: u32, n: usize| &rom[a as usize..a as usize + n];
    // box set tables, one after the other, then the contiguous box blocks up to `box_end`
    let mut next = t.box_tables[0].rom;
    for bt in &t.box_tables {
        assert_eq!(bt.rom, next, "box table {}", bt.name);
        next = bt.rom + 4 * bt.sets.len() as u32;
        for &s in &bt.sets {
            assert!(t.box_blocks.iter().any(|b| b.rom == s), "set {s:#x} without block");
        }
    }
    for b in &t.box_blocks {
        assert_eq!(b.rom, next, "box block {:#x}: position", b.rom);
        assert!(b.tail.len() < 8);
        next = b.rom + b.len() as u32;
    }
    assert_eq!(next, t.box_end, "end of the boxes");
    // read of the set: set address + 8 n
    for (k, bt) in t.box_tables.iter().enumerate() {
        for (s, &a) in bt.sets.iter().enumerate() {
            for n in 1..4 {
                let r = a + 8 * n as u32;
                if r + 8 <= t.box_end {
                    assert_eq!(t.hit_box(k, s, n).unwrap().bytes(), at(r, 8), "{} set {s} box {n}", bt.name);
                }
            }
        }
    }
    // ESHL 5 shot: the offsets designate frame lists; the flips are followed by the first one
    let s5 = &t.shot5;
    for (base, offs) in [(s5.even_rom, &s5.even), (s5.odd_rom, &s5.odd)] {
        for &d in offs.iter() {
            let target = base.wrapping_add(d as i32 as u32);
            assert!(t.frame_lists.iter().any(|l| l.rom == target), "shot 5: list {target:#x}");
        }
    }
    assert_eq!(s5.mirrored_rom + 2 * s5.mirrored.len() as u32, t.frame_lists[0].rom);
}

#[test]
fn texts_lossless() {
    if !has("texts.json") {
        return;
    }
    let t = Texts::load(&assets().join("texts.json")).unwrap();
    check("texts", |p| p.put_texts(&t));
    let mut end = 0;
    for d in &t.texts {
        assert!(d.rom >= end, "text {:#x}: overlaps the previous one", d.rom);
        end = d.rom + d.bytes().len() as u32;
    }
    for tb in &t.tables {
        for &e in &tb.entries {
            assert!(t.by_rom(e).is_some(), "table {:#x}: {e:#x} without text", tb.rom);
        }
    }
    // popup names: contiguous strings terminated by '/'
    let p = &t.popups;
    let mut len = 0;
    let mut starts = Vec::new();
    for s in &p.strings {
        starts.push(p.strings_rom + len);
        len += ddsom_assets::texts::decode(s).len() as u32 + 1;
    }
    for &o in &p.offsets {
        assert!(starts.contains(&p.table.wrapping_add(o as u32)), "popup: offset {o}");
    }
    // names terminated by '@'
    for g in &t.names {
        let starts = g.starts();
        assert!(g.entries.iter().all(|e| starts.contains(e)), "names {:#x}: entry outside the names", g.table);
    }
}

#[test]
fn music_lossless() {
    if !has("sound/music.json") {
        return;
    }
    let m = Music::load(&assets().join("sound/music.json")).unwrap();
    // same read as the driver: linear addresses of ddsom_sound::Rom
    let rom = ddsom_sound::Rom::load(&roms()).unwrap();
    let at = |a: u32, n: usize| (0..n as u32).map(|k| rom.byte(a + k)).collect::<Vec<u8>>();
    assert_eq!(u32::from(rom.word(m.header_ptr)), m.header);
    let mut head = m.count.to_be_bytes().to_vec();
    head.extend([m.music_factor, m.sfx_volume]);
    head.extend(m.header_rest);
    assert_eq!(at(m.header, 6), head, "sound header");
    assert_eq!(m.songs.len(), m.count as usize);
    // sound table, definitions
    for (i, s) in m.songs.iter().enumerate() {
        let a = s.as_ref().map_or(0, |s| s.rom);
        assert_eq!(at(m.header + 6 + 4 * i as u32, 4), [(a >> 16) as u8, (a >> 8) as u8, a as u8, 0], "sound {i}: entry");
        if let Some(s) = s {
            let b = s.bytes();
            assert_eq!(at(s.rom, b.len()), b, "sound {i}: definition");
        }
    }
    // tracks: contiguous blocks re-encoded; each track and each jump target lands on an instruction
    let mut starts = std::collections::BTreeSet::new();
    let mut targets = Vec::new();
    let mut end = 0;
    for b in &m.code {
        assert!(b.rom >= end, "block {:#x}: overlap", b.rom);
        let bytes = b.bytes().unwrap_or_else(|| panic!("block {:#x}: unreadable token", b.rom));
        assert_eq!(at(b.rom, bytes.len()), bytes, "block {:#x}", b.rom);
        let mut a = b.rom;
        for t in b.ops.split(' ') {
            starts.insert(a);
            let n = ddsom_assets::music::encode_op(t).unwrap().len() as u32;
            if let Some((name, args)) = t.split_once('=') {
                if name == "jump" || name.starts_with("loop") || name.starts_with("exit") {
                    let off: i32 = args.rsplit(',').next().unwrap().parse().unwrap();
                    targets.push((a, (a + n).wrapping_add(off as u32)));
                }
            }
            a += n;
        }
        end = a;
    }
    for (a, t) in targets {
        assert!(starts.contains(&t), "jump at {a:#x} to {t:#x}: not an instruction");
    }
    for s in m.songs.iter().flatten() {
        for (c, a) in s.track_starts() {
            assert!(starts.contains(&a), "sound {:#x} channel {c}: track {a:#x}", s.rom);
            assert!(m.track(a).is_some());
        }
    }
    // instrument banks, samples, driver tables
    for (k, b) in m.banks.iter().enumerate() {
        assert_eq!(u32::from(rom.word(m.banks_table + 2 * k as u32)), b.rom, "bank {k}");
        let bytes: Vec<u8> = b.instruments.iter().flatten().copied().collect();
        assert_eq!(at(b.rom, bytes.len()), bytes, "bank {k}");
    }
    assert_eq!(u32::from(rom.word(m.samples_ptr)), m.samples_rom);
    let bytes: Vec<u8> = m.samples.iter().flatten().copied().collect();
    assert_eq!(at(m.samples_rom, bytes.len()), bytes, "sample records");
    for t in &m.tables {
        let b = t.bytes();
        assert_eq!(at(t.rom, b.len()), b, "table {}", t.name);
    }
}

#[test]
fn spawns_lossless() {
    if !has("data/spawns.json") {
        return;
    }
    let sp = Spawns::load(&assets().join("data/spawns.json")).unwrap();
    check("spawns", |p| p.put_spawns(&sp));
    for (s, &a) in sp.scenes.iter().enumerate() {
        assert!(sp.of_scene(s).is_some(), "scene {s}: script {a:#x} missing");
    }
    // scripts: one after the other
    let mut next = sp.scripts[0].rom;
    for sc in &sp.scripts {
        assert_eq!(sc.rom, next, "script {:#x}: position", sc.rom);
        next = sc.rom + sc.bytes().len() as u32;
    }
}

/// Data tables (`data/*.json`): each re-encoded table is identical to the ROM (in its space);
/// each region is covered exactly by its tables; any address of an `addr` column that falls
/// in a region designates the start of a row of a table.
fn check_data(rel: &str) {
    if !has(rel) {
        return;
    }
    let file = DataFile::load(&assets().join(rel)).unwrap();
    check(rel, |p| p.put_data(&file).unwrap_or_else(|e| panic!("{rel}: {e}")));
    let mut names = std::collections::HashSet::new();
    for t in &file.tables {
        assert!(names.insert(&t.name), "{rel}: duplicate name {}", t.name);
    }
    for r in &file.regions {
        let mut inside: Vec<(u32, u32, &str)> = file
            .tables
            .iter()
            .filter(|t| t.program == r.program)
            .map(|t| (t.rom, t.end(), t.name.as_str()))
            .chain(file.texts.iter().filter(|_| !r.program).map(|t| (t.rom, t.end(), t.name.as_str())))
            .filter(|&(start, _, _)| start >= r.start && start < r.end)
            .collect();
        inside.sort();
        let mut at = r.start;
        for (start, end, name) in inside {
            assert_eq!(start, at, "{rel}: region {}, {name}", r.name);
            at = end;
        }
        assert_eq!(at, r.end, "{rel}: end of region {}", r.name);
    }
    for t in &file.tables {
        let kinds = t.kinds();
        for (i, row) in t.rows.iter().enumerate() {
            for (c, k) in row.cells().iter().zip(&kinds) {
                if let (Cell::Addr(a), Kind::Addr) = (*c, k) {
                    if file.regions.iter().any(|r| !r.program && a >= r.start && a < r.end) {
                        let text = file.texts.iter().any(|x| x.rom == a);
                        assert!(
                            text || file.row_at(a).is_some(),
                            "{rel}: {} row {i}: {a:#x} is neither the start of a row nor a text",
                            t.name
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn players_lossless() {
    check_data("data/players.json");
    if !has("data/players.json") {
        return;
    }
    // per-character access, like init.rs: cleric's hit points (character 3) at level 13
    let f = DataFile::load(&assets().join("data/players.json")).unwrap();
    let hp = f.via("hp_by_level", 3).unwrap();
    let (rom, prog) = program();
    let t = u32::from_be_bytes(prog[0x325e + 12..0x325e + 16].try_into().unwrap()) as usize;
    assert_eq!(hp.value(13), u16::from_be_bytes([rom[t + 26], rom[t + 27]]) as i64);
}

#[test]
fn combat_lossless() {
    check_data("data/combat.json");
}

#[test]
fn enemies_lossless() {
    check_data("data/enemies.json");
    if !has("data/enemies.json") {
        return;
    }
    // initial HP like 0x4548: type 3, normal difficulty, 2 players, rank 5
    let f = DataFile::load(&assets().join("data/enemies.json")).unwrap();
    let hp = f.table("hp_enemy").unwrap();
    let (rom, _) = program();
    let at = 0x5c8c + (3 << 8) + (1 << 5) + 2 * 5;
    assert_eq!(hp.get(3 * 8 + 1, 5), u16::from_be_bytes([rom[at], rom[at + 1]]) as i64);
}

#[test]
fn crates_lossless() {
    check_data("data/boxes.json");
}

#[test]
fn scenes_lossless() {
    check_data("data/scenes.json");
}

#[test]
fn misc_lossless() {
    check_data("data/misc.json");
}

#[test]
fn dialogues_lossless() {
    check_data("data/dialogs.json");
}

#[test]
fn leftovers_lossless() {
    check_data("data/rest.json");
}

#[test]
fn natives_lossless() {
    check_data("data/natives.json");
}

/// QSound DSP tables: the words of `qsound_dsp.json` are those of `dl-1425.bin`.
#[test]
fn dsp_lossless() {
    if !has("sound/qsound_dsp.json") {
        return;
    }
    let dsp = std::fs::read(roms().join("dl-1425.bin")).unwrap();
    let rebuilt = ddsom_assets::rebuild::qsound_dsp(&assets()).unwrap();
    let t = ddsom_assets::music::DspTables::load(&assets().join("sound/qsound_dsp.json")).unwrap();
    for table in &t.tables {
        let (a, n) = (2 * table.rom as usize, 2 * table.values.len());
        assert_eq!(rebuilt[a..a + n], dsp[a..a + n], "table {}", table.name);
    }
}

/// Rebuilt Z80 region: each written byte equals that of the `dd2.01`/`dd2.02` ROMs, without conflict.
#[test]
fn audio_region_lossless() {
    if !has("sound/music.json") {
        return;
    }
    let img = ddsom_assets::rebuild::audio_region(&assets()).unwrap();
    assert!(img.conflicts.is_empty(), "contradictory writes: {:x?}", &img.conflicts[..img.conflicts.len().min(8)]);
    let rom = ddsom_sound::Rom::load(&roms()).unwrap();
    let region = rom.region();
    let first = ddsom_assets::rebuild::first_difference(&img, region);
    assert!(first.is_none(), "Z80 region: difference at {:#x}", first.unwrap());
}

/// The sound driver plays every sound identically from the ROMs and from the Z80 region rebuilt from `music.json`:
/// same writes (QSound, shared RAM) and same state, interrupt by interrupt. Catches a byte the driver reads that the
/// assets do not describe (the 8th byte of the ADPCM definitions was missing until 2026-11-02).
#[test]
fn music_driver_identical() {
    if !has("sound/music.json") {
        return;
    }
    use ddsom_sound::{Driver, Rom, SHARED_SIZE};
    let from_roms = Rom::load(&roms()).unwrap();
    let rebuilt = Rom::from_region(rebuild::audio_region(&assets()).unwrap().bytes);
    let count = from_roms.sound_header().count;
    for n in 0..count {
        let mut drivers = [Driver::new(from_roms.clone()), Driver::new(rebuilt.clone())];
        let mut shared = [vec![0u8; SHARED_SIZE], vec![0u8; SHARED_SIZE]];
        let mut out: [Vec<ddsom_sound::Write>; 2] = Default::default();
        for (d, (s, o)) in drivers.iter_mut().zip(shared.iter_mut().zip(out.iter_mut())) {
            d.reset(s, o);
        }
        for k in 0..1500 {
            for (d, (s, o)) in drivers.iter_mut().zip(shared.iter_mut().zip(out.iter_mut())) {
                if k == 10 {
                    s[0xfff] = 0xff;
                    s[0xffd] = 0x88;
                    s[0xffe] = 1;
                    s[0xf] = 0xff;
                }
                if k == 20 {
                    s[..16].fill(0);
                    s[0] = (n >> 8) as u8;
                    s[1] = n as u8;
                    s[0xf] = 0;
                }
                d.irq_request(s, o);
            }
            assert_eq!(out[0], out[1], "sound {n}, interrupt {k}");
            assert_eq!(drivers[0].ram_image(), drivers[1].ram_image(), "sound {n}, interrupt {k}: state");
            out[0].clear();
            out[1].clear();
        }
    }
}
