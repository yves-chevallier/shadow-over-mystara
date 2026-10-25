//! Chargement des ressources produites par `extract` (voir docs/assets.md).

pub mod data;
pub mod json;
pub mod levels;
pub mod music;
pub mod scene_palettes;
pub mod spawns;
pub mod sprites;
pub mod tables;
pub mod terrain;
pub mod texts;

pub use data::DataFile;
pub use levels::Levels;
pub use music::Music;
pub use scene_palettes::ScenePalettes;
pub use spawns::Spawns;
pub use sprites::{Anims, Frames};
pub use tables::SpriteTables;
pub use terrain::Terrain;
pub use texts::Texts;

use std::io;
use std::path::Path;

/// Toutes les cellules graphiques 16x16 : un indice de couleur (0..15) par pixel.
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

    /// Pixel (x, y) de la cellule `c`.
    pub fn pixel(&self, c: usize, x: usize, y: usize) -> u8 {
        self.pixels[(c * Self::SIZE + y) * Self::SIZE + x]
    }
}

/// Bibliothèque de palettes : 16 couleurs au format matériel 0xLRGB par palette.
pub struct Palettes(pub Vec<[u16; 16]>);

impl Palettes {
    pub fn load(path: &Path) -> io::Result<Self> {
        let mut r = png::Decoder::new(std::fs::File::open(path)?).read_info()?;
        let mut buf = vec![0; r.output_buffer_size()];
        r.next_frame(&mut buf)?;
        let colors: Vec<u16> = buf
            .chunks_exact(4)
            .map(|p| [p[3], p[0], p[1], p[2]].iter().fold(0, |c, &v| c << 4 | (v / 17) as u16))
            .collect();
        Ok(Palettes(colors.chunks_exact(16).map(|p| p.try_into().unwrap()).collect()))
    }
}

/// Échantillon : PCM 8 bits signé, boucle éventuelle (indice de début), note de référence.
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
