//! Les ressources extraites redonnent exactement les données des ROMs.
//! Ignoré si ../../assets ou ../../rom manquent (lancer `extract rom assets` à la racine).

use std::path::{Path, PathBuf};

use ddsom_assets::{Cells, Palettes, Sample};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn available() -> bool {
    root().join("assets/palettes.png").exists() && root().join("rom/dd2.13m").exists()
}

#[test]
fn graphismes_sans_perte() {
    if !available() {
        return;
    }
    let cells = Cells::load(&root().join("assets/gfx")).unwrap();
    let rom = cps2_gfx::rom::load_gfx(&root().join("rom"), cps2_gfx::rom::DDSOM_GFX, cps2_gfx::rom::DDSOM_GFX_SIZE).unwrap();
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
    let (rom, _) = cps2::load_program(&root().join("rom")).unwrap();
    let flat: Vec<u16> = pals.0.iter().flatten().copied().collect();
    assert_eq!(flat, rom[0x253000 / 2..0x293000 / 2]);
}

#[test]
fn sons_sans_perte() {
    if !available() {
        return;
    }
    let rom = root().join("rom");
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
