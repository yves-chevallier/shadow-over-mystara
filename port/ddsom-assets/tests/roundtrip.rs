//! Les ressources extraites redonnent exactement les données des ROMs.
//! Ignoré si ../../assets ou les ROMs manquent (lancer `extract <roms> assets` à la racine). Les ROMs
//! sont lues dans `DDSOM_ROMS`, sinon dans ../../rom.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use ddsom_assets::{Anims, Cells, Frames, Levels, Palettes, Sample};

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
