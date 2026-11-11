//! Export of the original graphics as images to retouch: the sprite frames of the game's frame table, assembled, and
//! the scenery maps of the levels (scroll 2 and 3), one continuous image per scene and layer.

use cps2_gfx::draw::rgb;
use cps2_gfx::TileAtlas;
use ddsom_data::levels::Layer;
use ddsom_data::sprites::Frames;
use ddsom_data::GameData;

use crate::render::original_row;
use crate::{Colors, Image, LevelMeta, CLEAR};

/// A frame assembled from its pieces, unmirrored: `16 * slot + pen` per pixel (pen 15: transparent).
pub struct FrameArt {
    pub image: Image<u8>,
    /// Position of the anchor in the image.
    pub anchor: [i32; 2],
    /// For each slot: the first piece with that palette, and the palette (0 to 31) of that piece in the frame table.
    pub slots: Vec<(u16, u16)>,
}

/// Number of frames (the table, then the frames outside it).
pub fn frame_count(frames: &Frames) -> u32 {
    (frames.frames.len() + frames.extra.len()) as u32
}

/// For each piece of frame `n`: offsets, first cell code, attributes (bits 0-4 palette, 5-6 mirrors, 8-11 columns - 1,
/// 12-15 rows - 1), as the game emits them unmirrored (`services::sprites::emit_frame`).
fn pieces(frames: &Frames, n: u32) -> Vec<(i32, i32, usize, u16)> {
    let fr = frames.number(n);
    let bank = (fr.bank as usize) << 16;
    fr.pieces
        .iter()
        .map(|p| (p.dx() as i32, p.dy() as i32, bank | p.code() as usize, if fr.per_piece() { p.attr() } else { fr.attr }))
        .collect()
}

/// The slot (palette rank in the frame) of each piece, and the slots: first piece and palette.
pub fn slots(frames: &Frames, n: u32) -> (Vec<usize>, Vec<(u16, u16)>) {
    let mut slots: Vec<(u16, u16)> = Vec::new();
    let mut of_piece = Vec::new();
    for (i, &(_, _, _, attr)) in pieces(frames, n).iter().enumerate() {
        let palette = attr & 0x1f;
        let slot = slots.iter().position(|s| s.1 == palette).unwrap_or_else(|| {
            slots.push((i as u16, palette));
            slots.len() - 1
        });
        of_piece.push(slot);
    }
    (of_piece, slots)
}

/// Frame `n` assembled as the hardware draws it, unmirrored (the last piece in front), or nothing if it has no piece.
pub fn frame_art(frames: &Frames, n: u32, atlas: &TileAtlas) -> Option<FrameArt> {
    let pieces = pieces(frames, n);
    let size = |attr: u16| ((((attr >> 8) & 0xf) + 1) as i32, (((attr >> 12) & 0xf) + 1) as i32);
    let x0 = pieces.iter().map(|p| p.0).min()?;
    let y0 = pieces.iter().map(|p| p.1).min()?;
    let x1 = pieces.iter().map(|p| p.0 + 16 * size(p.3).0).max()?;
    let y1 = pieces.iter().map(|p| p.1 + 16 * size(p.3).1).max()?;
    let (w, h) = ((x1 - x0) as usize, (y1 - y0) as usize);
    let mut pixels = vec![CLEAR; w * h];
    let (of_piece, slots) = slots(frames, n);
    let cells = atlas.cells();
    for (&(dx, dy, code, attr), slot) in pieces.iter().zip(of_piece) {
        let (columns, rows) = size(attr);
        let (flip_x, flip_y) = (attr & 0x20 != 0, attr & 0x40 != 0);
        for row in 0..rows as usize {
            for column in 0..columns as usize {
                // as `cps2_gfx::draw::Sprite::cell`
                let cx = if flip_x { columns as usize - 1 - column } else { column };
                let cy = if flip_y { rows as usize - 1 - row } else { row };
                let cell = if !flip_x && !flip_y {
                    (code & !0xf) + ((code + column) & 0xf) + 0x10 * row
                } else {
                    code + cx + 0x10 * cy
                } % cells;
                for py in 0..16 {
                    for px in 0..16 {
                        let pen = atlas.pen(cell, if flip_x { 15 - px } else { px }, if flip_y { 15 - py } else { py });
                        if pen != CLEAR {
                            let x = (dx - x0) as usize + 16 * column + px;
                            let y = (dy - y0) as usize + 16 * row + py;
                            pixels[y * w + x] = 16 * slot as u8 + pen;
                        }
                    }
                }
            }
        }
    }
    Some(FrameArt { image: Image { width: w, height: h, pixels }, anchor: [-x0, -y0], slots })
}

/// Colours of the pens: `16 * slot + pen` takes colour `pen` of `colors[slot]`; pen 15 is transparent.
pub fn colour(image: &Image<u8>, colors: &[Colors]) -> Image<[u8; 4]> {
    let pixels = image
        .pixels
        .iter()
        .map(|&v| match colors.get(v as usize / 16) {
            Some(c) if v % 16 != CLEAR => {
                let [r, g, b] = c[v as usize % 16];
                [r, g, b, 255]
            }
            _ => [0; 4],
        })
        .collect();
    Image { width: image.width, height: image.height, pixels }
}

/// A grey ramp, for a frame never seen in play (its colours are not known).
pub const GREY: Colors = {
    let mut c = [[0u8; 3]; 16];
    let mut i = 0;
    while i < 16 {
        c[i] = [(i * 17) as u8; 3];
        i += 1;
    }
    c
};

/// The colours of a palette in the CPS2 format.
pub fn colors_of(words: &[u16; 16]) -> Colors {
    words.map(rgb)
}

/// The scenery of scene `scene` on layer `li` (1: scroll 2, 2: scroll 3), as a grid of tiles with the palettes the
/// scene loads; nothing if the scene has no map for the layer.
pub fn level(data: &GameData, scene: usize, li: usize) -> Option<LevelMeta> {
    let layer: &Layer = data.levels.layers.iter().find(|l| l.name == ["", "scroll2", "scroll3"][li])?;
    let table = &layer.tables[*layer.scenes.get(scene)?];
    let sc = layer.strip_cases();
    // scroll 2: the last word of each row is a sentinel (0xfe)
    let width = table.width() - (layer.case == 16 && table.rows[0].last() == Some(&0xfe)) as usize;
    let size = [width * sc, table.rows.len() * sc];
    let mut cells = vec![None; size[0] * size[1]];
    for (ky, strips) in table.rows.iter().enumerate() {
        for (kx, &strip) in strips[..width].iter().enumerate() {
            if layer.fillers.contains(&strip) || strip as usize >= layer.strips.len() {
                continue;
            }
            for cy in 0..sc {
                for cx in 0..sc {
                    let (code, attr) = layer.case_at(strip as usize, cx, cy);
                    cells[(ky * sc + cy) * size[0] + kx * sc + cx] = Some([code, attr]);
                }
            }
        }
    }
    // the 32 palettes of the layer's page (page 2 for scroll 2, 3 for scroll 3) for this scene
    let sp = &data.scene_palettes;
    let first = sp.pages[li].entries.get(scene).and_then(|&a| sp.library_index(a).filter(|_| a != 0));
    let palettes = (0..32)
        .map(|p| first.and_then(|f| data.palettes.0.get(f + p)).map_or(GREY, colors_of))
        .collect();
    Some(LevelMeta { scene: scene as u32, layer: li, tile: layer.case as usize, scale: 1, size, cells, palettes })
}

/// The image of a level, coloured (empty tiles transparent), and its pens.
pub fn level_image(meta: &LevelMeta, atlas: &TileAtlas) -> (Image<[u8; 4]>, Image<u8>) {
    let t = meta.tile;
    let (w, h) = (meta.size[0] * t, meta.size[1] * t);
    let mut pixels = vec![[0u8; 4]; w * h];
    let mut pens = vec![CLEAR; w * h];
    for row in 0..meta.size[1] {
        for column in 0..meta.size[0] {
            let Some([code, attr]) = meta.cell(column, row) else { continue };
            let colors = &meta.palettes[(attr & 0x1f) as usize];
            for py in 0..t {
                let sy = if attr & 0x40 != 0 { t - 1 - py } else { py };
                let line = original_row(atlas.pens(), t, code, column, sy);
                for px in 0..t {
                    let pen = line[if attr & 0x20 != 0 { t - 1 - px } else { px }];
                    if pen != CLEAR {
                        let [r, g, b] = colors[pen as usize];
                        let at = (row * t + py) * w + column * t + px;
                        pixels[at] = [r, g, b, 255];
                        pens[at] = pen;
                    }
                }
            }
        }
    }
    (Image { width: w, height: h, pixels }, Image { width: w, height: h, pixels: pens })
}
