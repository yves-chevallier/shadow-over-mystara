//! Une partie démarre-t-elle avec les graphismes et les échantillons reconstruits depuis les ressources ?
//!
//!   partie_assets <roms> <eeprom> <assets> <scénario> <images> <sortie.png>
//!
//! Deux machines identiques (jeu réécrit en Rust, 68000 surcadencé), l'une avec les ROMs graphiques et
//! sonores, l'autre avec `ddsom_assets::rebuild` ; le scénario est rejoué sur les deux. Les images
//! (RGB) et les échantillons audio doivent être identiques à chaque image ; la dernière image est écrite
//! en PNG.

use std::path::Path;

use cps2::Machine;
use ddsom_assets::rebuild;

fn machine(roms: &Path, eeprom: Option<&[u8]>, assets: Option<&Path>) -> Machine {
    let mut m = match assets {
        Some(dir) => {
            let gfx = rebuild::gfx_region(&dir.join("gfx")).expect("graphismes");
            let pcm = rebuild::qsound_samples(dir).expect("échantillons").bytes;
            Machine::load_ddsom_with(roms, eeprom, Some(gfx), Some(pcm))
        }
        None => Machine::load_ddsom(roms, eeprom),
    }
    .expect("chargement");
    ddsom_game::install(&mut m);
    m.set_overclock(16.0);
    m
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, assets, scenario, frames, out] = &a[..] else {
        eprintln!("usage : partie_assets <roms> <eeprom> <assets> <scénario> <images> <sortie.png>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let mut rom = machine(Path::new(roms), ee.as_deref(), None);
    let mut asset = machine(Path::new(roms), ee.as_deref(), Some(Path::new(assets)));
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    let frames: usize = frames.parse().unwrap();
    let (mut image, mut non_blank, mut audio) = (Vec::new(), 0, 0usize);
    for f in 0..frames {
        script.apply(f, &mut rom);
        script.apply(f, &mut asset);
        let (a, b) = (rom.run_frame(), asset.run_frame());
        if a != b {
            println!("DIFFÉRENT : image {f} (graphismes)");
            std::process::exit(1);
        }
        let (sa, sb): (Vec<_>, Vec<_>) = (rom.sound.audio.drain(..).collect(), asset.sound.audio.drain(..).collect());
        if sa != sb {
            println!("DIFFÉRENT : image {f} (échantillons)");
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
    println!("IDENTIQUE : {frames} images ({non_blank} non unies), {audio} échantillons audio comparés, dernière image dans {out}");
}
