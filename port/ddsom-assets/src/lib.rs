//! Loading of the resources produced by `extract` (see docs/assets.md): graphics, palettes, samples, music, and the
//! rebuilding of the ROM image (`rebuild`). The typed game data (sprites, scenery, tables...) are in `ddsom-data`.

pub mod music;
pub mod rebuild;
pub mod remainder;

pub use ddsom_data::{data, json, levels, pointers, scene_palettes, spawns, speeds, sprites, tables, terrain, texts};
pub use ddsom_data::{DataFile, Levels, ScenePalettes, Spawns, Speeds, SpriteTables, Terrain, Texts};
pub use ddsom_data::{Anims, EffectTables, Frames};
pub use music::Music;

use std::io;
use std::path::Path;

/// All 16x16 graphic cells: one color index (0..15) per pixel.
pub struct Cells {
    pixels: Vec<u8>,
}

impl Cells {
    pub const SIZE: usize = 16;

    pub fn load(dir: &Path) -> io::Result<Self> {
        let mut pixels = Vec::new();
        for n in 0.. {
            let path = dir.join(format!("cells_{n:02}.png"));
            if !path.exists() {
                break;
            }
            let (side, img) = read_indexed(&path)?;
            let grid = side / Self::SIZE;
            for c in 0..grid * grid {
                let (cx, cy) = ((c % grid) * Self::SIZE, (c / grid) * Self::SIZE);
                for y in 0..Self::SIZE {
                    pixels.extend_from_slice(&img[(cy + y) * side + cx..][..Self::SIZE]);
                }
            }
        }
        Ok(Cells { pixels })
    }

    pub fn len(&self) -> usize {
        self.pixels.len() / (Self::SIZE * Self::SIZE)
    }

    pub fn is_empty(&self) -> bool {
        self.pixels.is_empty()
    }

    /// Pixel (x, y) of cell `c`.
    pub fn pixel(&self, c: usize, x: usize, y: usize) -> u8 {
        self.pixels[(c * Self::SIZE + y) * Self::SIZE + x]
    }
}

/// Palette library: 16 colors per palette in the hardware 0xLRGB format.
pub struct Palettes(pub Vec<[u16; 16]>);

impl Palettes {
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
}

/// Sample: signed 8-bit PCM, optional loop (start index), reference note.
pub struct Sample {
    pub pcm: Vec<i8>,
    pub loop_start: Option<usize>,
    pub root: u8,
}

impl Sample {
    pub fn load(path: &Path) -> io::Result<Self> {
        let b = std::fs::read(path)?;
        let mut s = Sample { pcm: Vec::new(), loop_start: None, root: 60 };
        let mut i = 12;
        while i + 8 <= b.len() {
            let len = u32::from_le_bytes(b[i + 4..i + 8].try_into().unwrap()) as usize;
            let body = &b[i + 8..i + 8 + len];
            match &b[i..i + 4] {
                b"data" => s.pcm = body.iter().map(|&v| (v ^ 0x80) as i8).collect(),
                b"smpl" => {
                    let u = |o: usize| u32::from_le_bytes(body[o..o + 4].try_into().unwrap()) as usize;
                    s.root = u(12) as u8;
                    if u(28) > 0 {
                        s.loop_start = Some(u(36 + 8));
                    }
                }
                _ => {}
            }
            i += 8 + len + len % 2;
        }
        Ok(s)
    }
}

fn read_indexed(path: &Path) -> io::Result<(usize, Vec<u8>)> {
    let mut dec = png::Decoder::new(std::fs::File::open(path)?);
    dec.set_transformations(png::Transformations::IDENTITY);
    let mut r = dec.read_info()?;
    let mut buf = vec![0; r.output_buffer_size()];
    let info = r.next_frame(&mut buf)?;
    let side = info.width as usize;
    let pixels = buf[..info.buffer_size()].iter().flat_map(|b| [b >> 4, b & 15]).collect();
    Ok((side, pixels))
}
