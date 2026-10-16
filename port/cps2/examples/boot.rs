//! Démarre ddsom et écrit des captures PNG aux images demandées.
//! cargo run --release -p cps2 --example boot -- <roms> <eeprom> <dossier_sortie> f1 f2 ...

use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(&a[1]), ee.as_deref()).expect("roms");
    let out = Path::new(&a[3]);
    std::fs::create_dir_all(out).unwrap();
    let frames: Vec<u64> = a[4..].iter().map(|s| s.parse().unwrap()).collect();
    let last = *frames.iter().max().unwrap();
    let t0 = std::time::Instant::now();
    for f in 1..=last {
        let img = m.run_frame();
        if frames.contains(&f) {
            let file = std::fs::File::create(out.join(format!("{f}.png"))).unwrap();
            let mut enc = png::Encoder::new(file, 384, 224);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header().unwrap().write_image_data(&img.concat()).unwrap();
            println!("image {f} : pc={:06x} sr={:04x}", m.cpu.pc, m.cpu.sr);
        }
    }
    println!("{last} images en {:.2?}", t0.elapsed());
}
