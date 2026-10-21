//! Les ressources extraites redonnent exactement les données des ROMs.
//! Ignoré si ../../assets ou les ROMs manquent (lancer `extract <roms> assets` à la racine). Les ROMs
//! sont lues dans `DDSOM_ROMS`, sinon dans ../../rom.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ddsom_assets::data::{Cell, DataFile, Kind};
use ddsom_assets::{Anims, Cells, Frames, Levels, Music, Palettes, Sample, ScenePalettes, Spawns, SpriteTables, Terrain, Texts};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn roms() -> PathBuf {
    std::env::var_os("DDSOM_ROMS").map(PathBuf::from).unwrap_or_else(|| root().join("rom"))
}

fn available() -> bool {
    root().join("assets/palettes.png").exists() && roms().join("dd2.13m").exists()
}

/// Les ROMs et le fichier extrait `rel` sont présents.
fn has(rel: &str) -> bool {
    let ok = root().join("assets").join(rel).exists() && roms().join("dd2.13m").exists();
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

#[test]
fn graphismes_sans_perte() {
    if !available() {
        return;
    }
    let cells = Cells::load(&root().join("assets/gfx")).unwrap();
    let rom = cps2_gfx::rom::load_gfx(&roms(), cps2_gfx::rom::DDSOM_GFX, cps2_gfx::rom::DDSOM_GFX_SIZE).unwrap();
    assert_eq!(cells.len() * 128, rom.len());
    // réencode chaque cellule au format matériel (4 plans par groupe de 8 pixels)
    for c in 0..cells.len() {
        let mut bytes = [0u8; 128];
        for y in 0..16 {
            for x in 0..16 {
                let p = cells.pixel(c, x, y);
                for plane in 0..4 {
                    bytes[y * 8 + (x / 8) * 4 + plane] |= ((p >> plane) & 1) << (7 - x % 8);
                }
            }
        }
        assert_eq!(&bytes[..], &rom[c * 128..][..128], "cellule {c}");
    }
}

#[test]
fn palettes_sans_perte() {
    if !available() {
        return;
    }
    let pals = Palettes::load(&root().join("assets/palettes.png")).unwrap();
    let (rom, _) = cps2::load_program(&roms()).unwrap();
    let flat: Vec<u16> = pals.0.iter().flatten().copied().collect();
    assert_eq!(flat, rom[0x253000 / 2..0x293000 / 2]);
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
    for i in 0..290 {
        let e = &z80[0x49e8 + 8 * i..][..8];
        let word = |o: usize| u16::from_le_bytes([e[o], e[o + 1]]) as usize;
        let (base, start, loop_start, end) = ((e[1] as usize) << 16, word(2), word(4), word(6));
        let s = Sample::load(&root().join(format!("assets/sounds/{i:03}.wav"))).unwrap();
        let expect: Vec<i8> = pcm[base + start..=base + end].iter().map(|&b| b as i8).collect();
        assert_eq!(s.pcm, expect, "échantillon {i}");
        assert_eq!(s.root, e[0], "note de l'échantillon {i}");
        assert_eq!(s.loop_start, (loop_start < end).then_some(loop_start - start), "boucle de l'échantillon {i}");
    }
}

#[test]
fn images_de_sprites_sans_perte() {
    if !has("sprites/frames.json") {
        return;
    }
    let frames = Frames::load(&root().join("assets/sprites/frames.json")).unwrap();
    let (rom, _) = program();
    let be32 = |a: u32| u32::from_be_bytes(rom[a as usize..a as usize + 4].try_into().unwrap());
    // table des images
    for (i, f) in frames.frames.iter().enumerate() {
        assert_eq!(be32(frames.table + 4 * i as u32), f.rom, "table des images, entrée {i}");
    }
    // images, réencodées à la suite : elles redonnent toute leur région
    let start = frames.frames[0].rom;
    let mut bytes = Vec::new();
    let mut offsets: BTreeMap<u32, Vec<u8>> = BTreeMap::new();
    for f in &frames.frames {
        assert_eq!(start + bytes.len() as u32, f.rom, "image {:#x} : position", f.rom);
        let n = if f.per_piece() { f.attr } else { f.cost } as usize;
        assert_eq!(f.pieces.len(), n, "image {:#x} : nombre de morceaux", f.rom);
        bytes.extend(f.cost.to_be_bytes());
        bytes.extend(f.attr.to_be_bytes());
        bytes.extend([f.bank, f.mode]);
        bytes.extend(f.offsets.to_be_bytes());
        let mut table = Vec::new();
        for p in &f.pieces {
            bytes.extend(p.code().to_be_bytes());
            if f.per_piece() {
                bytes.extend(p.attr().to_be_bytes());
            } else {
                assert_eq!(p.attr(), f.attr, "image {:#x} : attributs communs", f.rom);
            }
            table.extend(p.dx().to_be_bytes());
            table.extend(p.dy().to_be_bytes());
        }
        // une table partagée : les images qui l'emploient lisent les mêmes décalages
        let t = offsets.entry(f.offsets).or_default();
        let common = t.len().min(table.len());
        assert_eq!(t[..common], table[..common], "table de décalages {:#x}", f.offsets);
        if table.len() > t.len() {
            *t = table;
        }
    }
    let end = start as usize + bytes.len();
    assert_eq!(&rom[start as usize..end], &bytes[..], "région des images");
    // tables de décalages : jointives, elles redonnent toute leur région
    let first = *offsets.keys().next().unwrap();
    let mut region: Vec<u8> = Vec::new();
    for (&a, t) in &offsets {
        assert_eq!(first + region.len() as u32, a, "table de décalages {a:#x} : position");
        region.extend_from_slice(t);
    }
    assert_eq!(&rom[first as usize..first as usize + region.len()], &region[..], "région des décalages");
    assert_eq!(end as u32, first, "les décalages suivent les images");
}

#[test]
fn animations_sans_perte() {
    if !has("sprites/anims.json") || !has("sprites/frames.json") {
        return;
    }
    let frames = Frames::load(&root().join("assets/sprites/frames.json")).unwrap();
    let anims = Anims::load(&root().join("assets/sprites/anims.json")).unwrap();
    let (rom, _) = program();
    let at = |a: u32, n: usize| &rom[a as usize..a as usize + n];
    let be16 = |a: u32| u16::from_be_bytes(at(a, 2).try_into().unwrap());
    let be32 = |a: u32| u32::from_be_bytes(at(a, 4).try_into().unwrap());
    // familles du visualiseur
    for (i, f) in anims.families.iter().enumerate() {
        let i = i as u32;
        assert_eq!(at(anims.names_rom + 4 * i, 4), f.name.as_bytes());
        let list = anims.families_rom + be16(anims.families_rom + 2 * i) as u32;
        assert_eq!(be32(anims.variants_rom + 4 * i) & 0xff, 4 * (f.sets.len() as u32 - 1), "{}", f.name);
        for (v, &s) in f.sets.iter().enumerate() {
            assert_eq!(be32(list + 4 * v as u32), s, "{} variante {v}", f.name);
        }
    }
    // jeux
    for s in &anims.sets {
        for (k, &o) in s.scripts.iter().enumerate() {
            assert_eq!(be16(s.rom + 2 * k as u32) as i16, o, "jeu {:#x}, script {k}", s.rom);
        }
    }
    // entrées
    for e in &anims.entries {
        let mut b = Vec::new();
        b.extend(e.time.to_be_bytes());
        b.extend(e.flags.to_be_bytes());
        b.extend(frames.frames[e.frame as usize].rom.to_be_bytes());
        b.extend(&e.data);
        assert_eq!(e.data.len(), if e.long { 16 } else { 8 });
        assert_eq!(e.next.is_some(), !matches!(e.flags >> 8, 0 | 0xfe));
        if let Some(n) = e.next {
            b.extend(n.to_be_bytes());
        }
        assert_eq!(at(e.rom, b.len()), &b[..], "entrée {:#x}", e.rom);
    }
}

#[test]
fn decors_sans_perte() {
    if !has("levels.json") {
        return;
    }
    let levels = Levels::load(&root().join("assets/levels.json")).unwrap();
    let (rom, prog) = program();
    let at = |a: u32, n: usize| &rom[a as usize..a as usize + n];
    let words = |v: &[u16]| v.iter().flat_map(|w| w.to_be_bytes()).collect::<Vec<u8>>();
    let be32 = |m: &[u8], a: u32| u32::from_be_bytes(m[a as usize..a as usize + 4].try_into().unwrap());
    for l in &levels.layers {
        // banques de blocs
        for (b, bank) in l.banks.iter().enumerate() {
            assert_eq!(be32(&rom, l.banks_rom + 4 * b as u32), bank.rom, "{} banque {b}", l.name);
            let bytes: Vec<u8> = bank.blocks.iter().flat_map(|k| words(k)).collect();
            assert_eq!(at(bank.rom, bytes.len()), &bytes[..], "{} blocs de la banque {b}", l.name);
        }
        // bandes, à la suite
        let bytes: Vec<u8> = l.strips.iter().flat_map(|s| words(s)).collect();
        assert_eq!(at(l.strips_rom, bytes.len()), &bytes[..], "{} bandes", l.name);
        // tables de bandes : jointives, terminées par la table des banques
        let mut next = l.tables[0].rom;
        for t in &l.tables {
            assert_eq!(t.rom, next, "{} table {:#x} : position", l.name, t.rom);
            let mut bytes = (t.width() as u16).to_be_bytes().to_vec();
            for r in &t.rows {
                assert_eq!(r.len(), t.width());
                bytes.extend(words(r));
            }
            assert_eq!(at(t.rom, bytes.len()), &bytes[..], "{} table {:#x}", l.name, t.rom);
            next = t.rom + bytes.len() as u32;
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
        // pointeurs des scènes (espace programme)
        for (s, &t) in l.scenes.iter().enumerate() {
            assert_eq!(be32(&prog, l.scenes_rom + 4 * s as u32), l.tables[t].pointer(), "{} scène {s}", l.name);
        }
    }
}

#[test]
fn scroll1_sans_perte() {
    if !has("levels.json") {
        return;
    }
    let s1 = Levels::load(&root().join("assets/levels.json")).unwrap().scroll1;
    let (rom, _) = program();
    let words = |v: &[u16]| v.iter().flat_map(|w| w.to_be_bytes()).collect::<Vec<u8>>();
    // grilles, puis zéros jusqu'aux blocs
    let mut bytes: Vec<u8> = s1.grids.iter().flat_map(|g| {
        assert_eq!(g.len(), 64);
        words(g)
    }).collect();
    bytes.resize((s1.blocks_rom - s1.grids_rom) as usize, 0);
    assert_eq!(&rom[s1.grids_rom as usize..s1.blocks_rom as usize], &bytes[..], "grilles du scroll 1");
    // blocs, à la suite : ils finissent où commencent les bandes du scroll 2
    let bytes: Vec<u8> = s1.blocks.iter().flat_map(|b| {
        assert_eq!(b.len(), 32);
        words(b)
    }).collect();
    let end = s1.blocks_rom as usize + bytes.len();
    assert_eq!(&rom[s1.blocks_rom as usize..end], &bytes[..], "blocs du scroll 1");
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
    let t = Terrain::load(&root().join("assets/terrain.json")).unwrap();
    let (rom, _) = program();
    let at = |a: u32, n: usize| &rom[a as usize..a as usize + n];
    let words = |v: &[u16]| v.iter().flat_map(|w| w.to_be_bytes()).collect::<Vec<u8>>();
    for m in &t.maps {
        let bytes: Vec<u8> = m.columns.iter().flat_map(|c| words(c)).collect();
        assert_eq!(at(m.rom, bytes.len()), &bytes[..], "carte {:#x}", m.rom);
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
    for k in &t.kinds {
        let bytes: Vec<u8> = k.kinds.iter().flat_map(|e| words(&e.words())).collect();
        assert_eq!(at(k.rom, bytes.len()), &bytes[..], "types de terrain {:#x}", k.rom);
    }
    assert_eq!(t.kinds[0].rom + 16 * t.kinds[0].kinds.len() as u32, t.kinds[1].rom);
}

#[test]
fn palettes_des_scenes_sans_perte() {
    if !has("scene_palettes.json") {
        return;
    }
    let p = ScenePalettes::load(&root().join("assets/scene_palettes.json")).unwrap();
    let (rom, prog) = program();
    let bytes: Vec<u8> = p.stages.iter().flatten().flat_map(|&v| ScenePalettes::stage_word(v).to_be_bytes()).collect();
    assert_eq!(&rom[p.stages_rom as usize..p.stages_rom as usize + bytes.len()], &bytes[..], "palettes de la page 0");
    for t in [&p.extra, &p.sets].into_iter().chain(p.pages.iter()) {
        let m = if t.program { &prog } else { &rom };
        let bytes: Vec<u8> = t.entries.iter().flat_map(|a| a.to_be_bytes()).collect();
        assert_eq!(&m[t.rom as usize..t.rom as usize + bytes.len()], &bytes[..], "table {:#x}", t.rom);
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
    let t = SpriteTables::load(&root().join("assets/sprites/tables.json")).unwrap();
    let frames = Frames::load(&root().join("assets/sprites/frames.json")).unwrap();
    let (rom, prog) = program();
    let at = |a: u32, n: usize| &rom[a as usize..a as usize + n];
    // tables de jeux de boîtes, à la suite, puis les blocs de boîtes jointifs jusqu'à `box_end`
    let mut next = t.box_tables[0].rom;
    for bt in &t.box_tables {
        assert_eq!(bt.rom, next, "table de boîtes {}", bt.name);
        let bytes: Vec<u8> = bt.sets.iter().flat_map(|a| a.to_be_bytes()).collect();
        assert_eq!(at(bt.rom, bytes.len()), &bytes[..], "table de boîtes {}", bt.name);
        next = bt.rom + bytes.len() as u32;
        for &s in &bt.sets {
            assert!(t.box_blocks.iter().any(|b| b.rom == s), "jeu {s:#x} sans bloc");
        }
    }
    for b in &t.box_blocks {
        assert_eq!(b.rom, next, "bloc de boîtes {:#x} : position", b.rom);
        let mut bytes: Vec<u8> = b.boxes.iter().flat_map(|x| x.bytes()).collect();
        bytes.extend(&b.tail);
        assert!(b.tail.len() < 8);
        assert_eq!(at(b.rom, bytes.len()), &bytes[..], "bloc de boîtes {:#x}", b.rom);
        next = b.rom + bytes.len() as u32;
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
    // boîtes de visibilité et marques au sol
    let bytes: Vec<u8> = t.bounds.iter().flat_map(|b| [b.0 as u16, b.1, b.2 as u16, b.3]).flat_map(|w| w.to_be_bytes()).collect();
    assert_eq!(at(t.bounds_rom, bytes.len()), &bytes[..], "boîtes de visibilité");
    let bytes: Vec<u8> = t.marks.iter().flat_map(|m| m.words()).flat_map(|w| w.to_be_bytes()).collect();
    assert_eq!(at(t.marks_rom, bytes.len()), &bytes[..], "marques au sol");
    // listes d'images
    for l in &t.frame_lists {
        let bytes: Vec<u8> = l.frames.iter().flat_map(|&f| frames.frames[f as usize].rom.to_be_bytes()).collect();
        assert_eq!(at(l.rom, bytes.len()), &bytes[..], "liste d'images {:#x}", l.rom);
    }
    // tir ESHL 5 : boîtes et déplacements en espace programme, retournements en espace données
    let s5 = &t.shot5;
    let pat = |a: u32, n: usize| &prog[a as usize..a as usize + n];
    let bytes: Vec<u8> = s5.boxes.iter().flatten().copied().collect();
    assert_eq!(pat(s5.boxes_rom, bytes.len()), &bytes[..], "tir 5 : boîtes");
    for (base, offs) in [(s5.even_rom, &s5.even), (s5.odd_rom, &s5.odd)] {
        let bytes: Vec<u8> = offs.iter().flat_map(|d| d.to_be_bytes()).collect();
        assert_eq!(pat(base, bytes.len()), &bytes[..], "tir 5 : déplacements {base:#x}");
        for &d in offs.iter() {
            let target = base.wrapping_add(d as i32 as u32);
            assert!(t.frame_lists.iter().any(|l| l.rom == target), "tir 5 : liste {target:#x}");
        }
    }
    let bytes: Vec<u8> = s5.mirrored.iter().flatten().copied().collect();
    assert_eq!(at(s5.mirrored_rom, bytes.len()), &bytes[..], "tir 5 : retournements");
    // les retournements sont suivis de la première liste d'images
    assert_eq!(s5.mirrored_rom + bytes.len() as u32, t.frame_lists[0].rom);
}

#[test]
fn textes_sans_perte() {
    if !has("texts.json") {
        return;
    }
    let t = Texts::load(&root().join("assets/texts.json")).unwrap();
    let (rom, prog) = program();
    let mut end = 0;
    for d in &t.texts {
        assert!(d.rom >= end, "texte {:#x} : chevauche le précédent", d.rom);
        let bytes = d.bytes();
        assert_eq!(&rom[d.rom as usize..d.rom as usize + bytes.len()], &bytes[..], "texte {:#x}", d.rom);
        end = d.rom + bytes.len() as u32;
    }
    for tb in &t.tables {
        let m = if tb.program { &prog } else { &rom };
        for (i, &e) in tb.entries.iter().enumerate() {
            let a = tb.rom as usize + i * if tb.offsets { 2 } else { 4 };
            let v = if tb.offsets {
                tb.rom.wrapping_add(i16::from_be_bytes(m[a..a + 2].try_into().unwrap()) as u32)
            } else {
                u32::from_be_bytes(m[a..a + 4].try_into().unwrap())
            };
            assert_eq!(v, e, "table {:#x}, entrée {i}", tb.rom);
            assert!(t.by_rom(e).is_some(), "table {:#x} : {e:#x} sans texte", tb.rom);
        }
    }
    // noms des bulles : déplacements (espace programme), chaînes à la suite terminées par '/'
    let p = &t.popups;
    for (i, &o) in p.offsets.iter().enumerate() {
        let a = p.table as usize + 2 * i;
        assert_eq!(i16::from_be_bytes(prog[a..a + 2].try_into().unwrap()), o, "bulle {i}");
    }
    let mut bytes = Vec::new();
    let mut starts = Vec::new();
    for s in &p.strings {
        starts.push(p.strings_rom + bytes.len() as u32);
        bytes.extend(ddsom_assets::texts::decode(s));
        bytes.push(b'/');
    }
    assert_eq!(&rom[p.strings_rom as usize..p.strings_rom as usize + bytes.len()], &bytes[..], "noms des bulles");
    for &o in &p.offsets {
        assert!(starts.contains(&p.table.wrapping_add(o as u32)), "bulle : déplacement {o}");
    }
    // noms terminés par '@'
    for g in &t.names {
        let m = if g.program { &prog } else { &rom };
        for (i, &e) in g.entries.iter().enumerate() {
            let a = g.table as usize + 4 * i;
            assert_eq!(u32::from_be_bytes(m[a..a + 4].try_into().unwrap()), e, "noms {:#x}, entrée {i}", g.table);
        }
        let bytes: Vec<u8> = g.names.iter().flat_map(|n| {
            let mut b = ddsom_assets::texts::decode(n);
            b.push(b'@');
            b
        }).collect();
        assert_eq!(&rom[g.rom as usize..g.rom as usize + bytes.len()], &bytes[..], "noms {:#x}", g.table);
        let starts = g.starts();
        assert!(g.entries.iter().all(|e| starts.contains(e)), "noms {:#x} : entrée hors des noms", g.table);
    }
}

#[test]
fn musique_sans_perte() {
    if !has("sound/music.json") {
        return;
    }
    let m = Music::load(&root().join("assets/sound/music.json")).unwrap();
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
    let sp = Spawns::load(&root().join("assets/data/spawns.json")).unwrap();
    let (rom, _) = program();
    for (s, &a) in sp.scenes.iter().enumerate() {
        let t = sp.table as usize + 4 * s;
        assert_eq!(u32::from_be_bytes(rom[t..t + 4].try_into().unwrap()), a, "scène {s}");
        assert!(sp.of_scene(s).is_some(), "scène {s} : script {a:#x} absent");
    }
    // scripts : réencodés, à la suite les uns des autres
    let mut next = sp.scripts[0].rom;
    for sc in &sp.scripts {
        assert_eq!(sc.rom, next, "script {:#x} : position", sc.rom);
        let b = sc.bytes();
        assert_eq!(&rom[sc.rom as usize..sc.rom as usize + b.len()], &b[..], "script {:#x}", sc.rom);
        next = sc.rom + b.len() as u32;
    }
}

/// Tables de données (`data/*.json`) : chaque table réencodée est identique à la ROM (dans son espace) ;
/// chaque région est couverte exactement par ses tables ; toute adresse d'une colonne `addr` qui tombe
/// dans une région désigne le début d'une ligne d'une table.
fn check_data(rel: &str) {
    if !has(rel) {
        return;
    }
    let file = DataFile::load(&root().join("assets").join(rel)).unwrap();
    let (data, prog) = program();
    let mut names = std::collections::HashSet::new();
    for t in &file.tables {
        assert!(names.insert(&t.name), "{rel} : nom {} en double", t.name);
        let b = t.bytes().unwrap_or_else(|e| panic!("{rel} : {e}"));
        let rom = if t.program { &prog } else { &data };
        let at = t.rom as usize;
        assert_eq!(&rom[at..at + b.len()], &b[..], "{rel} : table {} ({:#x})", t.name, t.rom);
    }
    for t in &file.texts {
        let b = t.bytes();
        let at = t.rom as usize;
        assert_eq!(&data[at..at + b.len()], &b[..], "{rel} : texte {}", t.name);
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
                        assert!(file.row_at(a).is_some(), "{rel} : {} ligne {i} : {a:#x} n'est pas un début de ligne", t.name);
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
    let f = DataFile::load(&root().join("assets/data/players.json")).unwrap();
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
    let f = DataFile::load(&root().join("assets/data/enemies.json")).unwrap();
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
