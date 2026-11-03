//! Palette library: `assets/palettes.png`, one 16-colour palette per row of an RGBA image (docs/assets.md). The
//! original keeps it at 0x253000-0x293000 (8192 palettes of 32 bytes) and designates a palette by its address; the
//! colours are words in the hardware format 0xLRGB (brightness, red, green, blue).

use std::io;
use std::path::Path;

/// Palette library: 16 colours per palette.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Palettes(pub Vec<[u16; 16]>);

impl Palettes {
    /// Address of the library in the program ROM.
    pub const ROM: u32 = 0x253000;

    /// Loads `palettes.png` (RGBA, each 4-bit component times 17; alpha is the brightness).
    pub fn load(path: &Path) -> io::Result<Self> {
        let mut r = png::Decoder::new(std::fs::File::open(path)?).read_info()?;
        let mut buf = vec![0; r.output_buffer_size()];
        r.next_frame(&mut buf)?;
        let colors: Vec<u16> = buf
            .as_chunks::<4>()
            .0
            .iter()
            .map(|p| [p[3], p[0], p[1], p[2]].iter().fold(0, |c, &v| c << 4 | (v / 17) as u16))
            .collect();
        Ok(Palettes(colors.as_chunks::<16>().0.to_vec()))
    }

    /// The library as the ROM stores it: one big-endian word per colour, from [`Palettes::ROM`].
    pub fn bytes(&self) -> Vec<u8> {
        self.0.iter().flatten().flat_map(|c| c.to_be_bytes()).collect()
    }

    /// Colour word at the ROM address `a` (even), if the library holds it.
    pub fn word_at(&self, a: u32) -> Option<u16> {
        let i = a.checked_sub(Self::ROM)? as usize / 2;
        self.0.get(i / 16).map(|p| p[i % 16])
    }
}
