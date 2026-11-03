//! Palettes: the game's library, one 16-color palette per row of an RGBA PNG.

use std::io;
use std::path::Path;

use ddsom_data::Palettes;

use crate::rom::Program;

/// Number of palettes of the library (0x253000-0x293000).
const COUNT: u32 = 8192;

pub fn extract(roms: &Path, path: &Path) -> io::Result<()> {
    let library = palettes(&Program::load(roms)?);
    // color 0xLRGB -> R, G, B, A = 4-bit components x 17 (A = brightness)
    let rgba: Vec<u8> = library
        .0
        .iter()
        .flatten()
        .flat_map(|&c| [c >> 8, c >> 4, c, c >> 12].map(|v| (v & 0xf) as u8 * 17))
        .collect();
    let mut enc = png::Encoder::new(std::fs::File::create(path)?, 16, COUNT);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(&rgba)?;
    Ok(())
}

/// The palette library of the program `rom`.
pub fn palettes(rom: &Program) -> Palettes {
    Palettes((0..COUNT).map(|p| std::array::from_fn(|c| rom.rd16(Palettes::ROM + 32 * p + 2 * c as u32))).collect())
}
