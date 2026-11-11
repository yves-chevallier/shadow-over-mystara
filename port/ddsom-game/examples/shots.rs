//! Screenshots of a scenario: shots <roms> <eeprom> <scenario> <folder> f1 f2 ...
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let m = cps2::Machine::load_ddsom(std::path::Path::new(&a[1]), ee.as_deref()).unwrap();
    let mut m = ddsom_game::install(m);
    let s = ddsom_game::core::scenario::Scenario::load(std::path::Path::new(&a[3])).unwrap();
    let out = std::path::Path::new(&a[4]);
    std::fs::create_dir_all(out).unwrap();
    let frames: Vec<usize> = a[5..].iter().map(|v| v.parse().unwrap()).collect();
    for f in 0..*frames.iter().max().unwrap() {
        s.apply(f, &mut m);
        let img = m.run_frame();
        if frames.contains(&(f + 1)) {
            let file = std::fs::File::create(out.join(format!("{}.png", f + 1))).unwrap();
            let mut e = png::Encoder::new(file, 384, 224);
            e.set_color(png::ColorType::Rgb);
            e.write_header().unwrap().write_image_data(&img.concat()).unwrap();
        }
    }
}
