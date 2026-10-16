//! Non-régression du rendu : chaque instantané de testdata/vsnap (capturé par mame/vsnap.lua)
//! doit donner exactement la capture PNG de MAME. Ignoré si les ROMs ou les instantanés manquent.

use std::path::{Path, PathBuf};

use cps2_gfx::{render, rom, VideoState};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn words(p: &Path) -> Vec<u16> {
    std::fs::read(p).unwrap().chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect()
}

fn read_png(p: &Path) -> Vec<[u8; 3]> {
    let mut r = png::Decoder::new(std::fs::File::open(p).unwrap()).read_info().unwrap();
    let mut buf = vec![0; r.output_buffer_size()];
    let info = r.next_frame(&mut buf).unwrap();
    let ch = info.color_type.samples();
    buf[..info.buffer_size()].chunks_exact(ch).map(|c| [c[0], c[1], c[2]]).collect()
}

#[test]
fn rendu_identique_a_mame() {
    let romdir = root().join("rom");
    let snaps = root().join("testdata/vsnap");
    if !romdir.join("dd2.13m").exists() || !snaps.exists() {
        eprintln!("ROMs ou instantanés absents, test ignoré");
        return;
    }
    let gfx = rom::load_gfx(&romdir, rom::DDSOM_GFX, rom::DDSOM_GFX_SIZE).unwrap();

    let mut frames: Vec<u32> = std::fs::read_dir(&snaps).unwrap()
        .filter_map(|e| e.ok()?.file_name().to_str()?.parse().ok())
        .collect();
    frames.sort();
    assert!(!frames.is_empty());
    // MAME numérote ses captures dans l'ordre des images
    for (n, f) in frames.iter().enumerate() {
        let d = snaps.join(f.to_string());
        let gfxram = words(&d.join("gfxram.bin"));
        let cps_a: [u16; 32] = words(&d.join("cps_a.bin")).try_into().unwrap();
        let cps_b: [u16; 32] = words(&d.join("cps_b.bin")).try_into().unwrap();
        // MAME affiche la table d'objets verrouillée au vblank précédent
        let output: [u16; 6] = words(&d.join("prev_output.bin")).try_into().unwrap();
        let bank = std::fs::read_to_string(d.join("prev_objbank.txt")).unwrap();
        let objram = words(&d.join(if bank.trim() == "0" { "prev_objram1.bin" } else { "prev_objram2.bin" }));
        let latch = d.join("palette_latch.bin");
        let pal = latch.exists().then(|| words(&latch));
        let v = VideoState {
            gfxram: &gfxram, cps_a: &cps_a, cps_b: &cps_b, objram: &objram, output: &output,
            palette_ram: pal.as_deref(),
        };
        let img = render(&v, &gfx);
        let reference = read_png(&snaps.join(format!("png/ddsom/{n:04}.png")));
        let diff = img.iter().zip(&reference).filter(|(a, b)| a != b).count();
        assert_eq!(diff, 0, "image {f} : {diff} pixels différents de MAME");
    }
}
