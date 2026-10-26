//! Les ressources extraites redonnent exactement les données des ROMs.
//! Ignoré si ../../assets ou les ROMs manquent (lancer `extract <roms> assets` à la racine). Les ROMs
//! sont lues dans `DDSOM_ROMS`, sinon dans ../../rom.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ddsom_assets::data::{Cell, DataFile, Kind};
use ddsom_assets::rebuild::{self, Program};
use ddsom_assets::{Anims, Cells, Frames, Levels, Music, Palettes, Sample, ScenePalettes, Spawns, SpriteTables, Terrain, Texts};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Dossier des ressources : `DDSOM_ASSETS`, sinon `assets/` à la racine.
fn assets() -> PathBuf {
    std::env::var_os("DDSOM_ASSETS").map(PathBuf::from).unwrap_or_else(|| root().join("assets"))
}

fn roms() -> PathBuf {
    std::env::var_os("DDSOM_ROMS").map(PathBuf::from).unwrap_or_else(|| root().join("rom"))
}

fn available() -> bool {
    assets().join("palettes.png").exists() && roms().join("dd2.13m").exists()
}

/// Les ROMs et le fichier extrait `rel` sont présents.
fn has(rel: &str) -> bool {
    let ok = assets().join(rel).exists() && roms().join("dd2.13m").exists();
    if !ok {
        eprintln!("{rel} ou les ROMs absents : test ignoré");
    }
    ok
}

/// ROM programme : données (lectures du 68000) et espace programme déchiffré, en octets.
fn program() -> (Vec<u8>, Vec<u8>) {
    let (data, prog) = cps2::load_program(&roms()).unwrap();
    let bytes = |w: Vec<u16>| w.iter().flat_map(|v| v.to_be_bytes()).collect();
    (bytes(data), bytes(prog))
}

/// Les tables que `put` réécrit redonnent la ROM, octet pour octet, sur les zones qu'elles décrivent.
fn check(what: &str, put: impl FnOnce(&mut Program)) {
    let mut p = Program::default();
    put(&mut p);
    let (data, prog) = program();
    for (name, img, rom) in [("données", &p.data, &data), ("programme", &p.prog, &prog)] {
        assert!(img.conflicts.is_empty(), "{what} : écritures contradictoires en {:#x}", img.conflicts[0]);
        if let Some(a) = rebuild::first_difference(img, rom) {
            panic!("{what} : espace {name}, octet {a:#x} : {:#04x} au lieu de {:#04x}", img.bytes[a as usize], rom[a as usize]);
        }
    }
}

#[test]
fn graphismes_sans_perte() {
    if !available() {
        return;
    }
    let cells = Cells::load(&assets().join("gfx")).unwrap();
    let rom = cps2_gfx::rom::load_gfx(&roms(), cps2_gfx::rom::DDSOM_GFX, cps2_gfx::rom::DDSOM_GFX_SIZE).unwrap();
    assert_eq!(cells.len() * 128, rom.len());
    // chaque cellule réencodée au format matériel (4 plans par groupe de 8 pixels)
    for c in 0..cells.len() {
        assert_eq!(&cells.encode(c)[..], &rom[c * 128..][..128], "cellule {c}");
    }
}

#[test]
fn palettes_sans_perte() {
    if !available() {
        return;
    }
    let pals = Palettes::load(&assets().join("palettes.png")).unwrap();
    assert_eq!(pals.0.len() * 32, 0x293000 - 0x253000);
    check("palettes", |p| p.put_palettes(&pals));
}

#[test]
fn sons_sans_perte() {
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
            assert!(!assets().join(format!("sounds/{i:03}.wav")).exists(), "fiche invalide {i} : fichier");
            continue;
        }
        count += 1;
        let s = Sample::load(&assets().join(format!("sounds/{i:03}.wav"))).unwrap();
        let expect: Vec<i8> = pcm[base + start..=base + end].iter().map(|&b| b as i8).collect();
        assert_eq!(s.pcm, expect, "échantillon {i}");
        assert_eq!(s.root, e[0], "note de l'échantillon {i}");
        assert_eq!(s.loop_start, (loop_start < end).then_some(loop_start - start), "boucle de l'échantillon {i}");
    }
    assert_eq!(count, 342, "fiches valides");
}

#[test]
fn images_de_sprites_sans_perte() {
    if !has("sprites/frames.json") {
        return;
    }
    let frames = Frames::load(&assets().join("sprites/frames.json")).unwrap();
    // table des adresses, images et tables de décalages
    check("images de sprites", |p| p.put_frames(&frames));
    // les images se suivent sans trou, puis les tables de décalages (partagées) sans trou non plus
    let start = frames.frames[0].rom;
    let mut next = start;
    let mut offsets: BTreeMap<u32, usize> = BTreeMap::new();
    for f in &frames.frames {
        assert_eq!(next, f.rom, "image {:#x} : position", f.rom);
        let n = if f.per_piece() { f.attr } else { f.cost } as usize;
        assert_eq!(f.pieces.len(), n, "image {:#x} : nombre de morceaux", f.rom);
        if !f.per_piece() {
            assert!(f.pieces.iter().all(|p| p.attr() == f.attr), "image {:#x} : attributs communs", f.rom);
        }
        let piece = if f.per_piece() { 4 } else { 2 };
        next += 10 + piece * n as u32;
        let len = offsets.entry(f.offsets).or_default();
        *len = (*len).max(4 * n);
    }
    let first = *offsets.keys().next().unwrap();
    assert_eq!(next, first, "les décalages suivent les images");
    let mut at = first;
    for (&a, &len) in &offsets {
        assert_eq!(at, a, "table de décalages {a:#x} : position");
        at += len as u32;
    }
}

#[test]
fn animations_sans_perte() {
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
fn decors_sans_perte() {
    if !has("levels.json") {
        return;
    }
    let levels = Levels::load(&assets().join("levels.json")).unwrap();
    // banques, bandes, tables de bandes et pointeurs des scènes (espace programme)
    check("décors", |p| p.put_levels(&levels));
    let (rom, _) = program();
    let at = |a: u32, n: usize| &rom[a as usize..a as usize + n];
    let be32 = |m: &[u8], a: u32| u32::from_be_bytes(m[a as usize..a as usize + 4].try_into().unwrap());
    for l in &levels.layers {
        // tables de bandes : jointives, terminées par la table des banques
        let mut next = l.tables[0].rom;
        for t in &l.tables {
            assert_eq!(t.rom, next, "{} table {:#x} : position", l.name, t.rom);
            assert!(t.rows.iter().all(|r| r.len() == t.width()));
            next = t.rom + 2 + 2 * (t.rows.len() * t.width()) as u32;
        }
        assert_eq!(next, l.banks_rom, "{} : fin des tables", l.name);
        // cases : même lecture que les caméras du jeu (`map_cell` de scroll.rs, `case_a` de cartes.rs)
        let rd16 = |a: u32| u16::from_be_bytes(at(a, 2).try_into().unwrap());
        let n = l.strip_cases();
        let colonne = if l.case == 16 { 0x40 } else { 0x20 };
        for strip in 0..l.strips.len() {
            for cx in 0..n {
                for cy in 0..n {
                    let off = (strip * n * n * 4 + cx * colonne + cy * 4) as u32;
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
                    assert_eq!(l.case_at(strip, cx, cy), expect, "{} bande {strip} case ({cx}, {cy})", l.name);
                }
            }
        }
    }
}

#[test]
fn scroll1_sans_perte() {
    if !has("levels.json") {
        return;
    }
    let s1 = Levels::load(&assets().join("levels.json")).unwrap().scroll1;
    check("scroll 1", |p| p.put_scroll1(&s1));
    let (rom, _) = program();
    assert!(s1.grids.iter().all(|g| g.len() == 64));
    assert!(s1.blocks.iter().all(|b| b.len() == 32));
    // les blocs finissent où commencent les bandes du scroll 2
    let end = s1.blocks_rom as usize + 64 * s1.blocks.len();
    assert_eq!(end, 0x1f9ba0, "les blocs du scroll 1 précèdent les bandes du scroll 2");
    // cases : même calcul que 0x260c (destination A1 en colonnes de 0x80 octets, cases de 4 octets)
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
                assert_eq!(s1.case_at(g, cx, cy), ((v >> 16) as u16, v as u16), "image {g} case ({cx}, {cy})");
            }
        }
    }
}

#[test]
fn terrain_sans_perte() {
    if !has("terrain.json") {
        return;
    }
    let t = Terrain::load(&assets().join("terrain.json")).unwrap();
    let (rom, _) = program();
    let at = |a: u32, n: usize| &rom[a as usize..a as usize + n];
    check("terrain", |p| p.put_terrain(&t));
    for m in &t.maps {
        // lecture de 0x24b0 : rang 0x1e - ((y >> 3) & 0x1e), colonne (2 x) & 0xffe0
        for x in (0..32 * m.columns.len() as u16 / 2).step_by(5) {
            for y in (0..0x200u16).step_by(7) {
                let y2 = y.wrapping_add(m.dy as u16);
                let off = (0x1e - ((y2 as u8 >> 3) & 0x1e) as u16).wrapping_add(x.wrapping_add(x) & 0xffe0);
                let a = m.rom.wrapping_add(off as i16 as u32);
                let w = u16::from_be_bytes(at(a, 2).try_into().unwrap());
                assert_eq!(m.word(x, y), Some(w), "carte {:#x} ({x}, {y})", m.rom);
            }
        }
    }
    // les deux cartes se suivent ; la seconde est suivie du remplissage de fin de ROM
    let end = t.maps[1].rom as usize + 32 * t.maps[1].columns.len();
    assert_eq!(t.maps[0].rom as usize + 32 * t.maps[0].columns.len(), t.maps[1].rom as usize);
    assert!(rom[end..end + 64].iter().all(|&b| b == 0xff));
    assert_eq!(t.kinds[0].rom + 16 * t.kinds[0].kinds.len() as u32, t.kinds[1].rom);
}

#[test]
fn palettes_des_scenes_sans_perte() {
    if !has("scene_palettes.json") {
        return;
    }
    let p = ScenePalettes::load(&assets().join("scene_palettes.json")).unwrap();
    check("palettes des scènes", |q| q.put_scene_palettes(&p));
    for t in [&p.extra, &p.sets].into_iter().chain(p.pages.iter()) {
        // toutes les palettes désignées sont dans la bibliothèque (sauf les entrées nulles)
        for &a in t.entries.iter().filter(|&&a| a != 0) {
            let i = p.library_index(a).unwrap_or_else(|| panic!("table {:#x} : {a:#x} hors bibliothèque", t.rom));
            assert!(i + t.count as usize <= 8192, "table {:#x} : {a:#x}", t.rom);
        }
    }
}

#[test]
fn tables_des_objets_sans_perte() {
    if !has("sprites/tables.json") || !has("sprites/frames.json") {
        return;
    }
    let t = SpriteTables::load(&assets().join("sprites/tables.json")).unwrap();
    let frames = Frames::load(&assets().join("sprites/frames.json")).unwrap();
    check("tables des objets", |p| p.put_sprite_tables(&t, &frames));
    let (rom, _) = program();
    let at = |a: u32, n: usize| &rom[a as usize..a as usize + n];
    // tables de jeux de boîtes, à la suite, puis les blocs de boîtes jointifs jusqu'à `box_end`
    let mut next = t.box_tables[0].rom;
    for bt in &t.box_tables {
        assert_eq!(bt.rom, next, "table de boîtes {}", bt.name);
        next = bt.rom + 4 * bt.sets.len() as u32;
        for &s in &bt.sets {
            assert!(t.box_blocks.iter().any(|b| b.rom == s), "jeu {s:#x} sans bloc");
        }
    }
    for b in &t.box_blocks {
        assert_eq!(b.rom, next, "bloc de boîtes {:#x} : position", b.rom);
        assert!(b.tail.len() < 8);
        next = b.rom + b.len() as u32;
    }
    assert_eq!(next, t.box_end, "fin des boîtes");
    // lecture du jeu : adresse du jeu + 8 n
    for (k, bt) in t.box_tables.iter().enumerate() {
        for (s, &a) in bt.sets.iter().enumerate() {
            for n in 1..4 {
                let r = a + 8 * n as u32;
                if r + 8 <= t.box_end {
                    assert_eq!(t.hit_box(k, s, n).unwrap().bytes(), at(r, 8), "{} jeu {s} boîte {n}", bt.name);
                }
            }
        }
    }
    // tir ESHL 5 : les déplacements désignent des listes d'images ; les retournements sont suivis de la première
    let s5 = &t.shot5;
    for (base, offs) in [(s5.even_rom, &s5.even), (s5.odd_rom, &s5.odd)] {
        for &d in offs.iter() {
            let target = base.wrapping_add(d as i32 as u32);
            assert!(t.frame_lists.iter().any(|l| l.rom == target), "tir 5 : liste {target:#x}");
        }
    }
    assert_eq!(s5.mirrored_rom + 2 * s5.mirrored.len() as u32, t.frame_lists[0].rom);
}

#[test]
fn textes_sans_perte() {
    if !has("texts.json") {
        return;
    }
    let t = Texts::load(&assets().join("texts.json")).unwrap();
    check("textes", |p| p.put_texts(&t));
    let mut end = 0;
    for d in &t.texts {
        assert!(d.rom >= end, "texte {:#x} : chevauche le précédent", d.rom);
        end = d.rom + d.bytes().len() as u32;
    }
    for tb in &t.tables {
        for &e in &tb.entries {
            assert!(t.by_rom(e).is_some(), "table {:#x} : {e:#x} sans texte", tb.rom);
        }
    }
    // noms des bulles : chaînes à la suite terminées par '/'
    let p = &t.popups;
    let mut len = 0;
    let mut starts = Vec::new();
    for s in &p.strings {
        starts.push(p.strings_rom + len);
        len += ddsom_assets::texts::decode(s).len() as u32 + 1;
    }
    for &o in &p.offsets {
        assert!(starts.contains(&p.table.wrapping_add(o as u32)), "bulle : déplacement {o}");
    }
    // noms terminés par '@'
    for g in &t.names {
        let starts = g.starts();
        assert!(g.entries.iter().all(|e| starts.contains(e)), "noms {:#x} : entrée hors des noms", g.table);
    }
}

#[test]
fn musique_sans_perte() {
    if !has("sound/music.json") {
        return;
    }
    let m = Music::load(&assets().join("sound/music.json")).unwrap();
    // même lecture que le pilote : adresses linéaires de ddsom_sound::Rom
    let rom = ddsom_sound::Rom::load(&roms()).unwrap();
    let at = |a: u32, n: usize| (0..n as u32).map(|k| rom.byte(a + k)).collect::<Vec<u8>>();
    assert_eq!(u32::from(rom.word(m.header_ptr)), m.header);
    let mut head = m.count.to_be_bytes().to_vec();
    head.extend([m.music_factor, m.sfx_volume]);
    head.extend(m.header_rest);
    assert_eq!(at(m.header, 6), head, "en-tête des sons");
    assert_eq!(m.songs.len(), m.count as usize);
    // table des sons, définitions
    for (i, s) in m.songs.iter().enumerate() {
        let a = s.as_ref().map_or(0, |s| s.rom);
        assert_eq!(at(m.header + 6 + 4 * i as u32, 4), [(a >> 16) as u8, (a >> 8) as u8, a as u8, 0], "son {i} : entrée");
        if let Some(s) = s {
            let b = s.bytes();
            assert_eq!(at(s.rom, b.len()), b, "son {i} : définition");
        }
    }
    // pistes : blocs jointifs réencodés ; chaque piste et chaque cible de saut tombe sur une instruction
    let mut starts = std::collections::BTreeSet::new();
    let mut targets = Vec::new();
    let mut end = 0;
    for b in &m.code {
        assert!(b.rom >= end, "bloc {:#x} : chevauchement", b.rom);
        let bytes = b.bytes().unwrap_or_else(|| panic!("bloc {:#x} : jeton illisible", b.rom));
        assert_eq!(at(b.rom, bytes.len()), bytes, "bloc {:#x}", b.rom);
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
        assert!(starts.contains(&t), "saut en {a:#x} vers {t:#x} : pas une instruction");
    }
    for s in m.songs.iter().flatten() {
        for (c, a) in s.track_starts() {
            assert!(starts.contains(&a), "son {:#x} canal {c} : piste {a:#x}", s.rom);
            assert!(m.track(a).is_some());
        }
    }
    // bancs d'instruments, échantillons, tables du pilote
    for (k, b) in m.banks.iter().enumerate() {
        assert_eq!(u32::from(rom.word(m.banks_table + 2 * k as u32)), b.rom, "banc {k}");
        let bytes: Vec<u8> = b.instruments.iter().flatten().copied().collect();
        assert_eq!(at(b.rom, bytes.len()), bytes, "banc {k}");
    }
    assert_eq!(u32::from(rom.word(m.samples_ptr)), m.samples_rom);
    let bytes: Vec<u8> = m.samples.iter().flatten().copied().collect();
    assert_eq!(at(m.samples_rom, bytes.len()), bytes, "fiches d'échantillons");
    for t in &m.tables {
        let b = t.bytes();
        assert_eq!(at(t.rom, b.len()), b, "table {}", t.name);
    }
}

#[test]
fn apparitions_sans_perte() {
    if !has("data/spawns.json") {
        return;
    }
    let sp = Spawns::load(&assets().join("data/spawns.json")).unwrap();
    check("apparitions", |p| p.put_spawns(&sp));
    for (s, &a) in sp.scenes.iter().enumerate() {
        assert!(sp.of_scene(s).is_some(), "scène {s} : script {a:#x} absent");
    }
    // scripts : à la suite les uns des autres
    let mut next = sp.scripts[0].rom;
    for sc in &sp.scripts {
        assert_eq!(sc.rom, next, "script {:#x} : position", sc.rom);
        next = sc.rom + sc.bytes().len() as u32;
    }
}

/// Tables de données (`data/*.json`) : chaque table réencodée est identique à la ROM (dans son espace) ;
/// chaque région est couverte exactement par ses tables ; toute adresse d'une colonne `addr` qui tombe
/// dans une région désigne le début d'une ligne d'une table.
fn check_data(rel: &str) {
    if !has(rel) {
        return;
    }
    let file = DataFile::load(&assets().join(rel)).unwrap();
    check(rel, |p| p.put_data(&file).unwrap_or_else(|e| panic!("{rel} : {e}")));
    let mut names = std::collections::HashSet::new();
    for t in &file.tables {
        assert!(names.insert(&t.name), "{rel} : nom {} en double", t.name);
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
            assert_eq!(start, at, "{rel} : région {}, {name}", r.name);
            at = end;
        }
        assert_eq!(at, r.end, "{rel} : fin de la région {}", r.name);
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
                            "{rel} : {} ligne {i} : {a:#x} n'est ni un début de ligne ni un texte",
                            t.name
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn joueurs_sans_perte() {
    check_data("data/players.json");
    if !has("data/players.json") {
        return;
    }
    // accès par personnage, comme init.rs : points de vie du clerc (personnage 3) au niveau 13
    let f = DataFile::load(&assets().join("data/players.json")).unwrap();
    let hp = f.via("hp_by_level", 3).unwrap();
    let (rom, prog) = program();
    let t = u32::from_be_bytes(prog[0x325e + 12..0x325e + 16].try_into().unwrap()) as usize;
    assert_eq!(hp.value(13), u16::from_be_bytes([rom[t + 26], rom[t + 27]]) as i64);
}

#[test]
fn combat_sans_perte() {
    check_data("data/combat.json");
}

#[test]
fn ennemis_sans_perte() {
    check_data("data/enemies.json");
    if !has("data/enemies.json") {
        return;
    }
    // PV initiaux comme 0x4548 : type 3, difficulté normale, 2 joueurs, rang 5
    let f = DataFile::load(&assets().join("data/enemies.json")).unwrap();
    let hp = f.table("hp_enemy").unwrap();
    let (rom, _) = program();
    let at = 0x5c8c + (3 << 8) + (1 << 5) + 2 * 5;
    assert_eq!(hp.get(3 * 8 + 1, 5), u16::from_be_bytes([rom[at], rom[at + 1]]) as i64);
}

#[test]
fn caisses_sans_perte() {
    check_data("data/boxes.json");
}

#[test]
fn scenes_sans_perte() {
    check_data("data/scenes.json");
}

#[test]
fn divers_sans_perte() {
    check_data("data/misc.json");
}

#[test]
fn dialogues_sans_perte() {
    check_data("data/dialogs.json");
}

#[test]
fn restes_sans_perte() {
    check_data("data/rest.json");
}
