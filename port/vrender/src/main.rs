//! Renders a MAME video snapshot (mame/vsnap.lua) and compares it to MAME's PNG capture.
//! usage: vrender <rom_dir> <snapshot_dir> <output.png> [reference.png] [gfx_mame.bin]

use std::path::Path;

use cps2_gfx::{render, rom, VideoState, SCREEN_H, SCREEN_W};

fn words(path: &Path) -> Vec<u16> {
    std::fs::read(path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .as_chunks::<2>().0.iter()
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .collect()
}

fn read_png(path: &Path) -> (usize, usize, Vec<[u8; 3]>) {
    let dec = png::Decoder::new(std::fs::File::open(path).expect("png"));
    let mut r = dec.read_info().expect("png info");
    let mut buf = vec![0; r.output_buffer_size()];
    let info = r.next_frame(&mut buf).expect("png frame");
    let ch = info.color_type.samples();
    let px = buf[..info.buffer_size()].chunks_exact(ch).map(|c| [c[0], c[1], c[2]]).collect();
    (info.width as usize, info.height as usize, px)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 4 {
        eprintln!("usage: {} <roms> <snapshot> <output.png> [reference.png] [gfx_mame.bin]", a[0]);
        std::process::exit(2);
    }
    let snap = Path::new(&a[2]);
    let gfx = rom::load_gfx(Path::new(&a[1]), rom::DDSOM_GFX, rom::DDSOM_GFX_SIZE).expect("roms gfx");
    let atlas = cps2_gfx::TileAtlas::from_planar(&gfx);

    if let Some(mame_gfx) = a.get(5) {
        let m = std::fs::read(mame_gfx).expect("gfx mame");
        let diff = gfx.iter().zip(&m).filter(|(x, y)| x != y).count();
        println!("gfx region: {} bytes differ from MAME (out of {})", diff, m.len());
    }

    let gfxram = words(&snap.join("gfxram.bin"));
    let cps_a: [u16; 32] = words(&snap.join("cps_a.bin")).try_into().unwrap();
    let cps_b: [u16; 32] = words(&snap.join("cps_b.bin")).try_into().unwrap();
    // Source of the object table: OBJ=prev_ (previous frame) or empty; OBJBANKXOR=1 inverts the bank.
    let pre = std::env::var("OBJ").unwrap_or_default();
    let xor: u32 = std::env::var("OBJBANKXOR").ok().and_then(|s| s.parse().ok()).unwrap_or(0);
    let output: [u16; 6] = words(&snap.join(format!("{pre}output.bin"))).try_into().unwrap();
    let bank: u32 = std::fs::read_to_string(snap.join(format!("{pre}objbank.txt"))).unwrap().trim().parse().unwrap();
    let objram = words(&snap.join(if (bank ^ xor) & 1 == 0 { format!("{pre}objram1.bin") } else { format!("{pre}objram2.bin") }));

    let latch = snap.join("palette_latch.bin");
    let palette = latch.exists().then(|| words(&latch));
    let v = VideoState {
        gfxram: &gfxram, cps_a: &cps_a, cps_b: &cps_b, objram: &objram, output: &output,
        palette_ram: palette.as_deref(),
    };
    let img = render(&v, &atlas);

    let f = std::fs::File::create(&a[3]).expect("output");
    let mut enc = png::Encoder::new(f, SCREEN_W as u32, SCREEN_H as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header().unwrap().write_image_data(&img.concat()).unwrap();

    if let Some(reference) = a.get(4) {
        let (w, h, refpx) = read_png(Path::new(reference));
        assert_eq!((w, h), (SCREEN_W, SCREEN_H), "reference size");
        let diff = img.iter().zip(&refpx).filter(|(x, y)| x != y).count();
        println!("{} pixels differ out of {} ({:.2} %)", diff, w * h, 100.0 * diff as f64 / (w * h) as f64);
    }
}
