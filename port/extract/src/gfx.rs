//! Graphismes : la ROM graphique en planches de cellules 16x16, PNG indexé 4 bits.

use std::io;
use std::path::Path;

use cps2_gfx::rom::{load_gfx, DDSOM_GFX, DDSOM_GFX_SIZE};

const CELL: usize = 16;
const GRID: usize = 64; // cellules par côté de planche
const CELL_BYTES: usize = 128;

/// Pixel (0..15) d'une cellule : chaque ligne de 16 pixels tient sur 8 octets, deux groupes de 4
/// plans (un octet par plan, bit 7 = pixel de gauche).
fn pixel(cell: &[u8], x: usize, y: usize) -> u8 {
    let plane = &cell[y * 8 + (x / 8) * 4..][..4];
    let bit = 7 - x % 8;
    (0..4).fold(0, |p, i| p | ((plane[i] >> bit) & 1) << i)
}

pub fn extract(roms: &Path, dir: &Path) -> io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let rom = load_gfx(roms, DDSOM_GFX, DDSOM_GFX_SIZE)?;
    let side = GRID * CELL;
    for (n, sheet) in rom.chunks(GRID * GRID * CELL_BYTES).enumerate() {
        let mut img = vec![0u8; side * side];
        for (c, cell) in sheet.chunks(CELL_BYTES).enumerate() {
            let (cx, cy) = ((c % GRID) * CELL, (c / GRID) * CELL);
            for y in 0..CELL {
                for x in 0..CELL {
                    img[(cy + y) * side + cx + x] = pixel(cell, x, y);
                }
            }
        }
        write_indexed(&dir.join(format!("cells_{n:02}.png")), side, &img)?;
    }
    Ok(())
}

/// PNG 4 bits avec une palette de gris (15 transparent), lisible par n'importe quel visualiseur.
fn write_indexed(path: &Path, side: usize, pixels: &[u8]) -> io::Result<()> {
    let mut enc = png::Encoder::new(std::fs::File::create(path)?, side as u32, side as u32);
    enc.set_color(png::ColorType::Indexed);
    enc.set_depth(png::BitDepth::Four);
    enc.set_palette((0..16u8).flat_map(|i| [i * 17; 3]).collect::<Vec<_>>());
    let mut trns = [255u8; 16];
    trns[15] = 0;
    enc.set_trns(trns.to_vec());
    let packed: Vec<u8> = pixels.chunks(2).map(|p| p[0] << 4 | p[1]).collect();
    enc.write_header()?.write_image_data(&packed)?;
    Ok(())
}
