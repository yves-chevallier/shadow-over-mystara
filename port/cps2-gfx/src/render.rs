//! The CPS2 renderer: draws a [`DrawList`] exactly as the hardware shows it.
//! Translation of cps_state::screen_update_cps1 / cps2_state::render_layers / cps2_render_sprites.

use crate::draw::{rgb, DrawList, Layer, Sprite, COLORS};
use crate::TileAtlas;

pub const SCREEN_W: usize = 384;
pub const SCREEN_H: usize = 224;

/// Base of the scenery tiles in the hardware graphics region (mapper_cps2: bank 1, 0x20000 units of 64 bytes).
const SCROLL_TILE_BASE: usize = 0x20000 * 64;

/// The video state of a CPS2 frame: GFX RAM, CPS-A/CPS-B registers, Object RAM ([`DrawList::from_video`]).
pub struct VideoState<'a> {
    /// GFX RAM 0x900000-0x92FFFF, in words.
    pub gfxram: &'a [u16],
    pub cps_a: &'a [u16; 32],
    pub cps_b: &'a [u16; 32],
    /// Object table latched at vblank (0x1000 words).
    pub objram: &'a [u16],
    pub output: &'a [u16; 6],
    /// Copy of the GFX RAM made when the palette base register is written (6 x 0x200 words).
    /// This is what the hardware displays; None = read the current GFX RAM.
    pub palette_ram: Option<&'a [u16]>,
}

/// The RGB colours of the palette (`cps1_build_palette`).
pub fn build_palette(list: &DrawList) -> Box<[[u8; 3]; COLORS]> {
    let mut pal = Box::new([[0u8; 3]; COLORS]);
    for (i, c) in pal.iter_mut().enumerate() {
        *c = rgb(list.palette.color(i));
    }
    pal
}

/// Row `ty % T` of the tile at tilemap (column, row) = (`tx / T`, `ty / T`) of a layer of tiles of side `T`: its `T`
/// pens in screen order and its colour base.
#[inline]
fn tile_row<const T: usize>(layer: &Layer, atlas: &[u8], tx: usize, ty: usize) -> ([u8; T], u16) {
    let tile = layer.tile_of::<T>(tx / T, ty / T);
    let code = tile.code as usize;
    let mut py = ty % T;
    if tile.flip_y() {
        py = T - 1 - py;
    }
    // hardware offset of the tile's pixel row (8x8 tiles: the half of the 16-pixel cell row is chosen by the
    // column); the row is contiguous in the atlas, at twice that offset (`TileAtlas`)
    let row = match T {
        8 => SCROLL_TILE_BASE + code * 64 + py * 8 + ((tx / T) & 1) * 4,
        16 => SCROLL_TILE_BASE + code * 128 + py * 8,
        _ => SCROLL_TILE_BASE + (code & 0x3fff) * 512 + py * 16,
    };
    let mut line = atlas_row::<T>(atlas, 2 * row);
    if tile.flip_x() {
        line.reverse();
    }
    (line, (tile.palette() + layer.palette_base) * 16)
}

/// Draws onto a screen line (pens, prio) the tiles of side `T` covering tilemap abscissas `tx0..` at tilemap
/// ordinate `ty`: each tile row is read once from the atlas; whole tiles are drawn as fixed-size blocks.
fn draw_row<const T: usize>(layer: &Layer, atlas: &[u8], tx0: usize, ty: usize, pbit: u8,
                            pens: &mut [u16; SCREEN_W], prio: &mut [u8; SCREEN_W]) {
    let size = 64 * T;
    let mut x = 0;
    let mut tx = tx0;
    while x < SCREEN_W {
        let u = tx % T;
        let n = (T - u).min(SCREEN_W - x);
        let (line, pen_base) = tile_row::<T>(layer, atlas, tx, ty);
        let dest = x..x + n;
        if transparent(&line) {
            // nothing to draw (frequent: the 8x8 layer is mostly empty)
        } else if n == T {
            draw_pens::<T>(&line, pen_base, pbit, (&mut pens[dest.clone()]).try_into().unwrap(),
                           (&mut prio[dest]).try_into().unwrap());
        } else {
            draw_some_pens(&line[u..u + n], pen_base, pbit, &mut pens[dest.clone()], &mut prio[dest]);
        }
        x += n;
        tx = (tx + n) % size;
    }
}

/// Are all the pens of a tile row transparent (15)?
#[inline]
fn transparent<const T: usize>(line: &[u8; T]) -> bool {
    line.as_chunks::<8>().0.iter().all(|&g| u64::from_ne_bytes(g) == 0x0f0f_0f0f_0f0f_0f0f)
}

/// The `T` pens at atlas offset `at`: a group of 8 pixels outside the graphics stays transparent.
#[inline]
fn atlas_row<const T: usize>(atlas: &[u8], at: usize) -> [u8; T] {
    if let Some(src) = atlas.get(at..at + T) {
        return src.try_into().unwrap();
    }
    let mut line = [15u8; T];
    for g in 0..T / 8 {
        if let Some(src) = atlas.get(at + 8 * g..at + 8 * g + 8) {
            line[8 * g..8 * g + 8].copy_from_slice(src);
        }
    }
    line
}

/// Draws the opaque pens of a whole tile row (all but 15) with colour base `pen_base`, marking them with layer bit
/// `pbit`.
#[inline]
fn draw_pens<const T: usize>(src: &[u8; T], pen_base: u16, pbit: u8, pens: &mut [u16; T], prio: &mut [u8; T]) {
    // branchless (a mask per pixel), so that the block is vectorised
    for i in 0..T {
        let opaque = ((src[i] != 15) as u8).wrapping_neg();
        let opaque16 = opaque as i8 as u16;
        pens[i] = (pens[i] & !opaque16) | ((pen_base + src[i] as u16) & opaque16);
        prio[i] |= pbit & opaque;
    }
}

/// `draw_pens` for part of a tile row (at the edges of the screen).
fn draw_some_pens(src: &[u8], pen_base: u16, pbit: u8, pens: &mut [u16], prio: &mut [u8]) {
    for ((&p, pen), pr) in src.iter().zip(pens).zip(prio) {
        if p != 15 {
            *pen = pen_base + p as u16;
            *pr |= pbit;
        }
    }
}

/// Renders a 384x224 RGB frame.
pub fn render(list: &DrawList, atlas: &TileAtlas) -> Vec<[u8; 3]> {
    let cells = atlas.pens();
    let pal = build_palette(list);
    const W: usize = SCREEN_W;
    const H: usize = SCREEN_H;
    let mut pens = vec![0xbffu16; W * H];
    let mut prio = vec![0u8; W * H];

    for (slot, &pbit) in [1u8, 2, 4].iter().enumerate() {
        let Some(li) = list.order[slot] else { continue };
        let layer = &list.layers[li];
        for y in 0..H {
            let (tx0, ty) = layer.origin(y);
            let line = y * W..(y + 1) * W;
            let (pens, prio) = ((&mut pens[line.clone()]).try_into().unwrap(), (&mut prio[line]).try_into().unwrap());
            match li {
                0 => draw_row::<8>(layer, cells, tx0, ty, pbit, pens, prio),
                1 => draw_row::<16>(layer, cells, tx0, ty, pbit, pens, prio),
                _ => draw_row::<32>(layer, cells, tx0, ty, pbit, pens, prio),
            }
        }
    }

    draw_sprites(list, cells, &mut pens, &mut prio);

    // every pen is below 0xc00 (layers: at most 0x7ff, sprites: 0x1ff)
    pens.iter().map(|&p| pal[(p as usize).min(0xbff)]).collect()
}

fn draw_sprites(list: &DrawList, atlas: &[u8], pens: &mut [u16], prio: &mut [u8]) {
    let elements = atlas.len() / TileAtlas::CELL_PENS;
    for i in (0..list.sprites.len()).rev() {
        let s: Sprite = list.sprites.get(i);
        let pmask = list.sprite_masks[s.priority];
        for row in 0..s.rows {
            for column in 0..s.columns {
                let (code, x, y) = s.cell(column, row);
                draw_tile16(atlas, code % elements, s.palette, s.flip_x, s.flip_y, x, y, pmask, pens, prio);
            }
        }
    }
}

/// drawgfx prio_transpen: pixel drawn if ((1 << prio) & pmask) == 0; prio becomes 31.
/// MAME forces bit 31 of the mask: a pixel already covered by a sprite (prio 31) blocks the
/// sprites drawn afterwards. Since the table is walked from end to start, the entry
/// with the highest index is the one that stays visible.
/// (`x`, `y`): screen position of the top left pixel of the cell.
fn draw_tile16(atlas: &[u8], code: usize, col: u16, flipx: bool, flipy: bool, x: i32, y: i32,
               pmask: u32, pens: &mut [u16], prio: &mut [u8]) {
    const CELL: usize = TileAtlas::CELL;
    let cell = &atlas[code * TileAtlas::CELL_PENS..][..TileAtlas::CELL_PENS];
    let pmask = pmask | 1 << 31;
    let pen_base = col * 16;
    // columns of the cell that fall on the screen
    let left = x;
    let px0 = (-left).clamp(0, CELL as i32) as usize;
    let px1 = (SCREEN_W as i32 - left).clamp(0, CELL as i32) as usize;
    if px0 >= px1 {
        return;
    }
    for py in 0..CELL {
        let by = y + py as i32;
        if by < 0 || by >= SCREEN_H as i32 {
            continue;
        }
        let ry = if flipy { CELL - 1 - py } else { py };
        let src = &cell[ry * CELL..][..CELL];
        let line = by as usize * SCREEN_W;
        for px in px0..px1 {
            let p = src[if flipx { CELL - 1 - px } else { px }];
            if p == 15 {
                continue;
            }
            let o = line + (left + px as i32) as usize;
            if (1u32 << (prio[o] & 0x1f)) & pmask == 0 {
                pens[o] = pen_base + p as u16;
            }
            prio[o] = 31;
        }
    }
}
