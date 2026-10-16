//! Aperçu en couleur : `preview <assets> <première_cellule> <palette> <sortie.png>`.
//! Rend 32x16 cellules avec la palette de la bibliothèque, comme le matériel (luminosité comprise).

use std::path::Path;

use ddsom_assets::{Cells, Palettes};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let num = |s: &str| usize::from_str_radix(s.trim_start_matches("0x"), if s.starts_with("0x") { 16 } else { 10 }).unwrap();
    let assets = Path::new(&a[1]);
    let cells = Cells::load(&assets.join("gfx")).unwrap();
    let pal = Palettes::load(&assets.join("palettes.png")).unwrap().0[num(&a[3])];
    let (first, cols, rows) = (num(&a[2]), 32, 16);
    let (w, h) = (cols * 16, rows * 16);
    let mut rgb = vec![0x20u8; w * h * 3];
    for c in 0..cols * rows {
        for y in 0..16 {
            for x in 0..16 {
                let p = cells.pixel(first + c, x, y);
                if p == 15 {
                    continue;
                }
                let o = (((c / cols) * 16 + y) * w + (c % cols) * 16 + x) * 3;
                rgb[o..o + 3].copy_from_slice(&color(pal[p as usize]));
            }
        }
    }
    let mut enc = png::Encoder::new(std::fs::File::create(&a[4]).unwrap(), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.write_header().unwrap().write_image_data(&rgb).unwrap();
}

/// Couleur matérielle 0xLRGB -> RGB (formule de MAME cps1_build_palette).
fn color(c: u16) -> [u8; 3] {
    // aperçu : pleine luminosité (le jeu règle la luminosité au chargement et pendant les fondus)
    let bright = 0x0f + (0xf << 1);
    [c >> 8, c >> 4, c].map(|v| ((v & 0xf) as u32 * 0x11 * bright / 0x2d) as u8)
}
