//! Palettes : la bibliothèque du jeu, une palette de 16 couleurs par ligne d'un PNG RGBA.

use std::io;
use std::path::Path;

/// Bibliothèque de palettes dans la ROM programme (0x253000-0x293000, 8192 palettes).
const LIBRARY: std::ops::Range<usize> = 0x253000..0x293000;

pub fn extract(roms: &Path, path: &Path) -> io::Result<()> {
    let (rom, _) = cps2::load_program(roms)?;
    let words = &rom[LIBRARY.start / 2..LIBRARY.end / 2];
    // couleur 0xLRGB -> R, G, B, A = composantes 4 bits x 17 (A = luminosité)
    let rgba: Vec<u8> = words
        .iter()
        .flat_map(|&c| [c >> 8, c >> 4, c, c >> 12].map(|v| (v & 0xf) as u8 * 17))
        .collect();
    let mut enc = png::Encoder::new(std::fs::File::create(path)?, 16, (words.len() / 16) as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&rgba)?;
    Ok(())
}
