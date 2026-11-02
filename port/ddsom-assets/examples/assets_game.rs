//! Does a game start with the graphics and samples rebuilt from the resources?
//!
//!   assets_game <roms> <eeprom> <assets> <scenario> <frames> <output.png>
//!
//! Two identical machines (game rewritten in Rust, overclocked 68000), one with the graphics and
//! sound ROMs, the other with `ddsom_assets::rebuild`; the scenario is replayed on both. The frames
//! (RGB) and the audio samples must be identical at every frame; the last frame is written
//! as a PNG.

use std::path::Path;

use cps2::Machine;
use ddsom_assets::rebuild;

fn machine(roms: &Path, eeprom: Option<&[u8]>, assets: Option<&Path>) -> Machine {
    let mut m = match assets {
        Some(dir) => {
            let gfx = rebuild::gfx_region(&dir.join("gfx")).expect("graphics");
            let pcm = rebuild::qsound_samples(dir).expect("samples").bytes;
            Machine::load_ddsom_with(roms, eeprom, Some(gfx), Some(pcm))
        }
        None => Machine::load_ddsom(roms, eeprom),
    }
    .expect("loading");
    ddsom_game::install(&mut m);
    m.set_overclock(16.0);
    m
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, assets, scenario, frames, out] = &a[..] else {
        eprintln!("usage: assets_game <roms> <eeprom> <assets> <scenario> <frames> <output.png>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let mut rom = machine(Path::new(roms), ee.as_deref(), None);
    let mut asset = machine(Path::new(roms), ee.as_deref(), Some(Path::new(assets)));
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scenario");
    let frames: usize = frames.parse().unwrap();
    let (mut image, mut non_blank, mut audio) = (Vec::new(), 0, 0usize);
    for f in 0..frames {
        script.apply(f, &mut rom);
        script.apply(f, &mut asset);
        let (a, b) = (rom.run_frame(), asset.run_frame());
        if a != b {
            println!("DIFFÉRENT: frame {f} (graphics)");
            std::process::exit(1);
        }
        let (sa, sb): (Vec<_>, Vec<_>) = (rom.sound.audio.drain(..).collect(), asset.sound.audio.drain(..).collect());
        if sa != sb {
            println!("DIFFÉRENT: frame {f} (samples)");
            std::process::exit(1);
        }
        audio += sa.len();
        non_blank += (a.iter().any(|p| p != &a[0])) as usize;
        image = a;
    }
    let mut enc = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(out).unwrap()), 384, 224);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let rgb: Vec<u8> = image.iter().flatten().copied().collect();
    enc.write_header().unwrap().write_image_data(&rgb).unwrap();
    println!("IDENTIQUE: {frames} frames ({non_blank} not uniform), {audio} audio samples compared, last frame in {out}");
}
