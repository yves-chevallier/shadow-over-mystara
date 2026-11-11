//! High-definition rendering: the CPS2 renderer ([`render`](crate::render)) at a whole scale `S`, where the images of an
//! [`HdPack`] replace the original graphics (docs/pack-hd.md, L22).
//!
//! The pack holds pens, not colours: each pixel of an HD image is a pen (0 to 14, 15 transparent) of the palette the
//! original graphics take at that place, so the colours come from the palette of the frame being drawn, exactly as for
//! the original pixels (palette variants, flashes and fades included). The composition is the hardware's, pixel by
//! pixel at `S` times the resolution: the layers in their order, each marking its opaque pixels, then the sprites from
//! the last to the first, hidden by the layers according to their priority mask and by the sprites already drawn.
//! Where the pack has no image, the original pixel is repeated `S` x `S` times: without any image, the result is the
//! CPS2 frame enlarged `S` times.
//!
//! * A sprite frame of the game's frame table ([`SpriteFrame`]) is replaced as a whole by its [`HdFrame`]: drawn
//!   where its entries would be, at its anchor, mirrored like them, with the priority of its entries; the palette of
//!   each of its pixels is that of the entry its slot designates.
//! * A layer tile is replaced by its HD tile, found by its graphics code ([`tile_key`]): wherever the tile is, with
//!   its palette and its mirrors. The scrolling (row scrolling included) moves the HD pixels by whole original pixels.

use std::collections::HashMap;

use cps2_gfx::draw::{DrawList, SpriteFrame};
use cps2_gfx::render::{build_palette, tile_pens, SCREEN_H, SCREEN_W};
use cps2_gfx::TileAtlas;

use crate::CLEAR;

/// Side of the sprite space, in original pixels (positions wrap around it).
const SPRITE_SPACE: i32 = 1024;


/// HD images at one scale, ready to draw.
#[derive(Default)]
pub struct HdPack {
    /// Scale of all the images (pixels per original pixel, per side).
    pub scale: usize,
    /// Sprite frames, by frame number (`frames.json`).
    pub frames: HashMap<u32, HdFrame>,
    /// Layer tiles of layers 1 to 3, by [`tile_key`]: `(tile_size * scale)²` pens, unmirrored, row after row.
    pub tiles: [HashMap<u32, Vec<u8>>; 3],
}

/// A sprite frame in HD, unmirrored.
pub struct HdFrame {
    pub width: usize,
    pub height: usize,
    /// One byte per pixel, row after row: `16 * slot + pen` (pen 15: transparent).
    pub pixels: Vec<u8>,
    /// Anchor of the frame in the image, in original pixels: the image's top left corner is `anchor` original pixels
    /// left of and above the anchor of the frame on screen ([`SpriteFrame::position`]).
    pub anchor: (i32, i32),
    /// For each slot, the piece of the frame (rank of its entry from the frame's first one) whose palette the slot takes.
    pub slots: Vec<u16>,
}

/// Key of a tile of layer `layer` (0 to 2) in [`HdPack::tiles`]: the graphics code (as the CPS2 renderer reads it)
/// and, for the 8 x 8 tiles, the parity of the column, which chooses the half of the graphics cell.
pub fn tile_key(layer: usize, code: u16, column: usize) -> u32 {
    match layer {
        0 => (code as u32) << 1 | (column & 1) as u32,
        1 => code as u32,
        _ => (code & 0x3fff) as u32,
    }
}

impl HdPack {
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty() && self.tiles.iter().all(HashMap::is_empty)
    }
}

/// Renders a frame at the scale of `pack`: `(384 * S) x (224 * S)` RGB pixels.
pub fn render_hd(list: &DrawList, atlas: &TileAtlas, pack: &HdPack) -> Vec<[u8; 3]> {
    let s = pack.scale.max(1);
    let (w, h) = (SCREEN_W * s, SCREEN_H * s);
    let mut screen = Screen { w, h, s, pens: vec![0xbff; w * h], prio: vec![0; w * h] };
    for (slot, pbit) in [1u8, 2, 4].into_iter().enumerate() {
        if let Some(li) = list.order[slot] {
            draw_layer(list, li, atlas, pack, pbit, &mut screen);
        }
    }
    draw_sprites(list, atlas, pack, &mut screen);
    let pal = build_palette(list);
    screen.pens.iter().map(|&p| pal[(p as usize).min(0xbff)]).collect()
}

/// Pens and layer marks of the HD screen (as in `render`: bits 0-2 the layer slots, 31 a sprite).
struct Screen {
    w: usize,
    h: usize,
    s: usize,
    pens: Vec<u16>,
    prio: Vec<u8>,
}

impl Screen {
    /// A sprite pixel at `o`: drawn unless hidden by the layers (`pmask`) or by a sprite drawn before.
    #[inline]
    fn sprite(&mut self, o: usize, pen: u16, pmask: u32) {
        if (1u32 << (self.prio[o] & 0x1f)) & (pmask | 1 << 31) == 0 {
            self.pens[o] = pen;
        }
        self.prio[o] = 31;
    }
}

/// Draws layer `li` (marked `pbit`), line by line of the original screen: each original line gives `S` HD lines.
fn draw_layer(list: &DrawList, li: usize, atlas: &TileAtlas, pack: &HdPack, pbit: u8, out: &mut Screen) {
    let layer = &list.layers[li];
    let t = layer.tile_size;
    let size = layer.size();
    let s = out.s;
    let hd_tiles = &pack.tiles[li];
    for y in 0..SCREEN_H {
        let (tx0, ty) = layer.origin(y);
        let (row, oy) = (ty / t, ty % t);
        let mut x = 0;
        let mut tx = tx0;
        // one run per tile crossed by the line
        while x < SCREEN_W {
            let (column, u) = (tx / t, tx % t);
            let n = (t - u).min(SCREEN_W - x);
            let tile = layer.tile(column, row);
            let pen_base = (tile.palette() + layer.palette_base) * 16;
            let hd = hd_tiles.get(&tile_key(li, tile.code, column));
            let py = if tile.flip_y() { t - 1 - oy } else { oy };
            let original = if hd.is_none() { original_row(atlas.pens(), t, tile.code, column, py) } else { [CLEAR; 32] };
            for sub in 0..s {
                let line = (y * s + sub) * out.w + x * s;
                for k in 0..n * s {
                    // HD column in the tile, on screen; then in the stored (unmirrored) tile
                    let hx = u * s + k;
                    let pen = match hd {
                        Some(pens) => {
                            let ts = t * s;
                            let sx = if tile.flip_x() { ts - 1 - hx } else { hx };
                            let sy = if tile.flip_y() { ts - 1 - (oy * s + sub) } else { oy * s + sub };
                            pens[sy * ts + sx]
                        }
                        None => {
                            let px = hx / s;
                            original[if tile.flip_x() { t - 1 - px } else { px }]
                        }
                    };
                    if pen != CLEAR {
                        out.pens[line + k] = pen_base + pen as u16;
                        out.prio[line + k] |= pbit;
                    }
                }
            }
            x += n;
            tx = (tx + n) % size;
        }
    }
}

/// The `t` pens of row `py` of an original layer tile (the first `t` of the array).
pub(crate) fn original_row(atlas: &[u8], t: usize, code: u16, column: usize, py: usize) -> [u8; 32] {
    let mut row = [CLEAR; 32];
    match t {
        8 => row[..8].copy_from_slice(&tile_pens::<8>(atlas, code, column, py)),
        16 => row[..16].copy_from_slice(&tile_pens::<16>(atlas, code, column, py)),
        _ => row = tile_pens::<32>(atlas, code, column, py),
    }
    row
}

/// Draws the sprites from the last entry to the first; the entries of a frame of the pack are drawn as one HD image,
/// when the loop reaches the frame's last entry.
fn draw_sprites(list: &DrawList, atlas: &TileAtlas, pack: &HdPack, out: &mut Screen) {
    let n = list.sprites.len();
    // entry -> frame of the pack that covers it
    let mut covered: Vec<Option<(&SpriteFrame, &HdFrame)>> = vec![None; n];
    for f in list.shown_frames() {
        if let Some(hd) = pack.frames.get(&f.frame) {
            for c in &mut covered[f.first as usize..(f.first + f.count) as usize] {
                *c = Some((f, hd));
            }
        }
    }
    let cells = atlas.pens();
    let elements = cells.len() / TileAtlas::CELL_PENS;
    for i in (0..n).rev() {
        match covered[i] {
            Some((f, hd)) if i == (f.first + f.count - 1) as usize => draw_frame(list, f, hd, out),
            Some(_) => {}
            None => {
                let sprite = list.sprites.get(i);
                let pmask = list.sprite_masks[sprite.priority];
                for row in 0..sprite.rows {
                    for column in 0..sprite.columns {
                        let (code, x, y) = sprite.cell(column, row);
                        let cell = &cells[(code % elements) * TileAtlas::CELL_PENS..][..TileAtlas::CELL_PENS];
                        draw_cell(cell, sprite.palette * 16, sprite.flip_x, sprite.flip_y, x, y, pmask, out);
                    }
                }
            }
        }
    }
}

/// An original 16 x 16 cell whose top left pixel is at screen (`x`, `y`), each pixel `S` x `S`.
fn draw_cell(cell: &[u8], pen_base: u16, flip_x: bool, flip_y: bool, x: i32, y: i32, pmask: u32, out: &mut Screen) {
    const CELL: usize = TileAtlas::CELL;
    let s = out.s as i32;
    for hy in 0..CELL as i32 * s {
        let by = y * s + hy;
        if by < 0 || by >= out.h as i32 {
            continue;
        }
        let py = (hy / s) as usize;
        let src = &cell[if flip_y { CELL - 1 - py } else { py } * CELL..][..CELL];
        for hx in 0..CELL as i32 * s {
            let bx = x * s + hx;
            if bx < 0 || bx >= out.w as i32 {
                continue;
            }
            let px = (hx / s) as usize;
            let p = src[if flip_x { CELL - 1 - px } else { px }];
            if p != CLEAR {
                out.sprite(by as usize * out.w + bx as usize, pen_base + p as u16, pmask);
            }
        }
    }
}

/// The HD image of frame `f`, at its anchor, mirrored like it.
fn draw_frame(list: &DrawList, f: &SpriteFrame, hd: &HdFrame, out: &mut Screen) {
    let s = out.s as i32;
    let last = list.sprites.get((f.first + f.count - 1) as usize);
    let pmask = list.sprite_masks[last.priority];
    let palettes: Vec<u16> = hd
        .slots
        .iter()
        .map(|&piece| list.sprites.get(f.first as usize + (piece as usize).min(f.count as usize - 1)).palette)
        .collect();
    let (ax, ay) = f.position(list);
    let (w, h) = (hd.width as i32, hd.height as i32);
    // a mirrored frame is mirrored around its anchor (pixel offset `d` becomes `-d - 1`)
    let left = if f.flip { ax * s - (w - hd.anchor.0 * s) } else { ax * s - hd.anchor.0 * s };
    let top = ay * s - hd.anchor.1 * s;
    // the pieces wrap around the 1024 x 1024 sprite space: a frame across its edge shows on both sides
    let wrap = SPRITE_SPACE * s;
    for (left, top) in [(left, top), (left - wrap, top), (left, top - wrap), (left - wrap, top - wrap)] {
        let (x0, x1) = (left.max(0), (left + w).min(out.w as i32));
        let (y0, y1) = (top.max(0), (top + h).min(out.h as i32));
        for by in y0..y1 {
            let src = &hd.pixels[((by - top) * w) as usize..][..w as usize];
            for bx in x0..x1 {
                let ix = bx - left;
                let v = src[if f.flip { w - 1 - ix } else { ix } as usize];
                let pen = v & 15;
                if pen == CLEAR {
                    continue;
                }
                let palette = palettes.get((v >> 4) as usize).copied().unwrap_or(palettes[0]);
                out.sprite(by as usize * out.w + bx as usize, palette * 16 + pen as u16, pmask);
            }
        }
    }
}
