//! Preview of a sprite frame: `frame <assets> <number> <palette> <output.png>`.
//! Assembles the pieces of the frame (16x16 cells, mirrors) and paints them with a palette of the
//! library (the same for all pieces: in game, the palette depends on the object).

use std::path::Path;

use ddsom_assets::{Cells, Frames, Palettes};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let num = |s: &str| usize::from_str_radix(s.trim_start_matches("0x"), if s.starts_with("0x") { 16 } else { 10 }).unwrap();
    let assets = Path::new(&a[1]);
    let frames = Frames::load(&assets.join("sprites/frames.json")).unwrap();
    let cells = Cells::load(&assets.join("gfx")).unwrap();
    let pal = Palettes::load(&assets.join("palettes.png")).unwrap().0[num(&a[3])];
    let f = &frames.frames[num(&a[2])];
    // placed cells: (x, y, cell, mirror x, mirror y)
    let mut tiles = Vec::new();
    for p in &f.pieces {
        let (nx, ny) = (((p.attr() >> 8) & 0xf) as i32 + 1, (p.attr() >> 12) as i32 + 1);
        let (fx, fy) = (p.attr() & 0x20 != 0, p.attr() & 0x40 != 0);
        let code = p.code() as usize;
        for by in 0..ny {
            for bx in 0..nx {
                let (cx, cy) = (if fx { nx - 1 - bx } else { bx }, if fy { ny - 1 - by } else { by });
                let c = if !fx && !fy {
                    (code & !0xf) + ((code + bx as usize) & 0xf) + 16 * by as usize
                } else {
                    code + cx as usize + 16 * cy as usize
                };
                let cell = ((f.bank as usize) << 16) | c;
                tiles.push((p.dx() as i32 + 16 * bx, p.dy() as i32 + 16 * by, cell, fx, fy));
            }
        }
    }
    let (x0, y0) = (tiles.iter().map(|t| t.0).min().unwrap(), tiles.iter().map(|t| t.1).min().unwrap());
    let (w, h) = ((tiles.iter().map(|t| t.0).max().unwrap() - x0 + 16) as usize, (tiles.iter().map(|t| t.1).max().unwrap() - y0 + 16) as usize);
    let mut rgba = vec![0u8; w * h * 4];
    // the object table is drawn from the end towards the start: the first piece ends up in front
    for &(x, y, cell, fx, fy) in tiles.iter().rev() {
        for py in 0..16 {
            for px in 0..16 {
                let v = cells.pixel(cell, if fx { 15 - px } else { px }, if fy { 15 - py } else { py });
                if v == 15 {
                    continue;
                }
                let o = (((y - y0) as usize + py) * w + (x - x0) as usize + px) * 4;
                let c = pal[v as usize];
                let bright = 0x0f + 2 * (c >> 12) as u32;
                let rgb = [c >> 8, c >> 4, c].map(|k| ((k & 0xf) as u32 * 0x11 * bright / 0x2d) as u8);
                rgba[o..o + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
        }
    }
    let mut enc = png::Encoder::new(std::fs::File::create(&a[4]).unwrap(), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgba);
    enc.write_header().unwrap().write_image_data(&rgba).unwrap();
    println!("frame {} ({:#x}): {} pieces, {w}x{h}, anchor at ({}, {})", a[2], f.rom, f.pieces.len(), -x0, -y0);
}
