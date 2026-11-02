//! Rendering of a CPS2 frame from the video state (GFX RAM, CPS-A/CPS-B registers, Object RAM).
//! Translation of cps_state::screen_update_cps1 / cps2_state::render_layers / cps2_render_sprites.

use crate::TileAtlas;

pub const SCREEN_W: usize = 384;
pub const SCREEN_H: usize = 224;
/// Visible area in MAME's 512x262 bitmap (CPS_HBEND, CPS_VBEND).
const VIS_X0: i32 = 64;
const VIS_Y0: i32 = 16;

/// Word index of the register at byte offset `byte`.
const fn word(byte: usize) -> usize {
    byte / 2
}

// CPS-A (word index)
const A_SCROLL_BASE: [usize; 3] = [word(0x02), word(0x04), word(0x06)];
const A_OTHER_BASE: usize = word(0x08);
const A_PALETTE_BASE: usize = word(0x0a);
const A_SCROLLX: [usize; 3] = [word(0x0c), word(0x10), word(0x14)];
const A_SCROLLY: [usize; 3] = [word(0x0e), word(0x12), word(0x16)];
const A_ROWSCROLL_OFFS: usize = word(0x20);
const A_VIDEOCONTROL: usize = word(0x22);
// CPS-B, CPS_B_21_DEF configuration (common to CPS2 games)
const B_LAYER_CONTROL: usize = word(0x26);
const B_PALETTE_CONTROL: usize = word(0x30);
const LAYER_ENABLE_MASK: [u16; 3] = [0x02, 0x04, 0x08];
// CPS2 object output (0x400000)
const OUT_PRI: usize = word(0x04);
const OUT_XOFFS: usize = word(0x08);
const OUT_YOFFS: usize = word(0x0a);

/// Base of the scenery tiles in the hardware graphics region (mapper_cps2: bank 1, 0x20000 units of 64 bytes).
const SCROLL_TILE_BASE: usize = 0x20000 * 64;

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

/// 0xC00-color RGB palette, built like cps1_build_palette.
pub fn build_palette(v: &VideoState) -> Box<[[u8; 3]; 0xc00]> {
    let mut pal = Box::new([[0u8; 3]; 0xc00]);
    let (ram, base) = match v.palette_ram {
        Some(p) => (p, 0),
        None => (v.gfxram, gfx_base(v.cps_a[A_PALETTE_BASE], 0x400)),
    };
    let ctrl = v.cps_b[B_PALETTE_CONTROL];
    let mut src = base;
    for page in 0..6 {
        if ctrl >> page & 1 != 0 {
            for off in 0..0x200 {
                let p = ram[src % ram.len()];
                src += 1;
                let bright = 0x0f + ((p >> 12) << 1) as u32;
                let c = |n: u16| ((n & 0x0f) as u32 * 0x11 * bright / 0x2d) as u8;
                pal[0x200 * page + off] = [c(p >> 8), c(p >> 4), c(p)];
            }
        } else if src != base {
            src += 0x200;
        }
    }
    pal
}

/// cps1_base: address (word index in the GFX RAM) designated by a CPS-A register.
fn gfx_base(reg: u16, boundary: usize) -> usize {
    let base = (reg as usize * 256) & !(boundary - 1);
    (base & 0x3ffff) / 2
}

struct Layer {
    base: usize,     // word index in the GFX RAM
    color_base: u16, // 0x20, 0x40, 0x60
}

impl Layer {
    /// Mappers tilemap0_scan / tilemap1_scan / tilemap2_scan for tiles of side `T`: (col, row) -> tile index.
    fn tile_index<const T: usize>(col: usize, row: usize) -> usize {
        match T {
            8 => (row & 0x1f) + ((col & 0x3f) << 5) + ((row & 0x20) << 6),
            16 => (row & 0x0f) + ((col & 0x3f) << 4) + ((row & 0x30) << 6),
            _ => (row & 0x07) + ((col & 0x3f) << 3) + ((row & 0x38) << 6),
        }
    }

    /// Row `ty % T` of the tile at tilemap (column, row) = (`tx / T`, `ty / T`): its `T` pens in screen order and
    /// its colour base.
    #[inline]
    fn tile_row<const T: usize>(&self, ram: &[u16], atlas: &[u8], tx: usize, ty: usize) -> ([u8; T], u16) {
        let idx = Self::tile_index::<T>(tx / T, ty / T);
        let code = ram[wrap(self.base + 2 * idx, ram.len())] as usize;
        let attr = ram[wrap(self.base + 2 * idx + 1, ram.len())];
        let mut py = ty % T;
        if attr & 0x40 != 0 {
            py = T - 1 - py;
        }
        // hardware offset of the tile's pixel row (8x8 tiles: the half of the 16-pixel cell row is chosen by the
        // column); the row is contiguous in the atlas, at twice that offset (`TileAtlas`)
        let row = match T {
            8 => SCROLL_TILE_BASE + code * 64 + py * 8 + ((idx >> 5) & 1) * 4,
            16 => SCROLL_TILE_BASE + code * 128 + py * 8,
            _ => SCROLL_TILE_BASE + (code & 0x3fff) * 512 + py * 16,
        };
        let mut line = atlas_row::<T>(atlas, 2 * row);
        if attr & 0x20 != 0 {
            line.reverse();
        }
        (line, ((attr & 0x1f) + self.color_base) * 16)
    }

    /// Draws onto a screen line (pens, prio) the tiles of side `T` covering tilemap abscissas `tx0..` at tilemap
    /// ordinate `ty`: each tile row is read once from the atlas; whole tiles are drawn as fixed-size blocks.
    fn draw_row<const T: usize>(&self, ram: &[u16], atlas: &[u8], tx0: usize, ty: usize, pbit: u8,
                                pens: &mut [u16; SCREEN_W], prio: &mut [u8; SCREEN_W]) {
        let size = 64 * T;
        let mut x = 0;
        let mut tx = tx0;
        while x < SCREEN_W {
            let u = tx % T;
            let n = (T - u).min(SCREEN_W - x);
            let (line, pen_base) = self.tile_row::<T>(ram, atlas, tx, ty);
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
}

/// Are all the pens of a tile row transparent (15)?
#[inline]
fn transparent<const T: usize>(line: &[u8; T]) -> bool {
    line.as_chunks::<8>().0.iter().all(|&g| u64::from_ne_bytes(g) == 0x0f0f_0f0f_0f0f_0f0f)
}

/// `i % len`, without the division in the usual case (`i` already below `len`).
#[inline]
fn wrap(i: usize, len: usize) -> usize {
    if i < len { i } else { i % len }
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
pub fn render(v: &VideoState, atlas: &TileAtlas) -> Vec<[u8; 3]> {
    let cells = atlas.pens();
    let pal = build_palette(v);
    const W: usize = SCREEN_W;
    const H: usize = SCREEN_H;
    let mut pens = vec![0xbffu16; W * H];
    let mut prio = vec![0u8; W * H];

    let layer_control = v.cps_b[B_LAYER_CONTROL];
    let video_control = v.cps_a[A_VIDEOCONTROL];
    const TILE: [usize; 3] = [8, 16, 32];
    let layers = [
        Layer { base: gfx_base(v.cps_a[A_SCROLL_BASE[0]], 0x4000), color_base: 0x20 },
        Layer { base: gfx_base(v.cps_a[A_SCROLL_BASE[1]], 0x4000), color_base: 0x40 },
        Layer { base: gfx_base(v.cps_a[A_SCROLL_BASE[2]], 0x4000), color_base: 0x60 },
    ];
    let enabled = [
        layer_control & LAYER_ENABLE_MASK[0] != 0,
        layer_control & LAYER_ENABLE_MASK[1] != 0 && video_control & 4 != 0,
        layer_control & LAYER_ENABLE_MASK[2] != 0 && video_control & 8 != 0,
    ];
    let other = gfx_base(v.cps_a[A_OTHER_BASE], 0x800);
    let rowscroll = video_control & 1 != 0;

    // Layer order: 4 groups of 2 bits, 0 = sprites, 1..3 = scroll1..3.
    let mut order = [
        (layer_control >> 6) & 3,
        (layer_control >> 8) & 3,
        (layer_control >> 10) & 3,
        (layer_control >> 12) & 3,
    ];
    let pri_ctrl = v.output[OUT_PRI];
    let mut lpri = order.map(|l| ((pri_ctrl >> (4 * l)) & 0x0f) as u32);
    // removes the CPS1 sprite layer from the order
    if order[0] == 0 { order[0] = order[1]; order[1] = 0; lpri[0] = lpri[1]; }
    if order[1] == 0 { order[1] = order[2]; order[2] = 0; lpri[1] = lpri[2]; }
    if order[2] == 0 { order[2] = order[3]; order[3] = 0; lpri[2] = lpri[3]; }

    let mut primasks = [0u32; 8];
    {
        let (l0, l1, l2) = (lpri[0], lpri[1], lpri[2]);
        let mut mask0 = 0xaau32;
        let mut mask1 = 0xccu32;
        if l0 > l1 { mask0 &= !0x88; }
        if l0 > l2 { mask0 &= !0xa0; }
        if l1 > l2 { mask1 &= !0xc0; }
        primasks[0] = 0xff;
        for i in 1..8u32 {
            if i <= l0 && i <= l1 && i <= l2 {
                primasks[i as usize] = 0xfe;
                continue;
            }
            let mut m = 0;
            if i <= l0 { m |= mask0; }
            if i <= l1 { m |= mask1; }
            if i <= l2 { m |= 0xf0; }
            primasks[i as usize] = m;
        }
    }

    for (slot, &pbit) in [1u8, 2, 4].iter().enumerate() {
        let l = order[slot];
        if l == 0 || !enabled[l as usize - 1] {
            continue;
        }
        let li = l as usize - 1;
        let layer = &layers[li];
        let size = 64 * TILE[li];
        let sx = v.cps_a[A_SCROLLX[li]] as i32;
        let sy = v.cps_a[A_SCROLLY[li]] as i32;
        for y in 0..H {
            let by = y as i32 + VIS_Y0;
            let mut scrollx = sx;
            if li == 1 && rowscroll {
                let offs = v.cps_a[A_ROWSCROLL_OFFS] as usize;
                scrollx += v.gfxram[(other + ((by as usize + offs) & 0x3ff)) % v.gfxram.len()] as i32;
            }
            let ty = ((by + sy).rem_euclid(size as i32)) as usize;
            let tx0 = ((VIS_X0 + scrollx).rem_euclid(size as i32)) as usize;
            let line = y * W..(y + 1) * W;
            let (pens, prio) = ((&mut pens[line.clone()]).try_into().unwrap(), (&mut prio[line]).try_into().unwrap());
            match li {
                0 => layer.draw_row::<8>(v.gfxram, cells, tx0, ty, pbit, pens, prio),
                1 => layer.draw_row::<16>(v.gfxram, cells, tx0, ty, pbit, pens, prio),
                _ => layer.draw_row::<32>(v.gfxram, cells, tx0, ty, pbit, pens, prio),
            }
        }
    }

    draw_sprites(v, cells, &primasks, &mut pens, &mut prio);

    // every pen is below 0xc00 (layers: at most 0x7ff, sprites: 0x1ff)
    pens.iter().map(|&p| pal[(p as usize).min(0xbff)]).collect()
}

fn draw_sprites(v: &VideoState, atlas: &[u8], primasks: &[u32; 8], pens: &mut [u16], prio: &mut [u8]) {
    let obj = v.objram;
    // find_last_sprite
    let mut last: isize = obj.len() as isize - 4;
    let mut off = 0;
    while off + 3 < obj.len() {
        if obj[off + 1] >= 0x8000 || obj[off + 3] >= 0xff00 {
            last = off as isize - 4;
            break;
        }
        off += 4;
    }
    let xoffs = 64 - v.output[OUT_XOFFS] as i32;
    let yoffs = 16 - v.output[OUT_YOFFS] as i32;
    let elements = atlas.len() / TileAtlas::CELL_PENS;

    let mut i = last;
    while i >= 0 {
        let s = &obj[i as usize..i as usize + 4];
        let mut x = s[0] as i32;
        let mut y = s[1] as i32;
        let priority = ((x >> 13) & 7) as usize;
        let code = s[2] as usize + (((y & 0x6000) as usize) << 3);
        let colour = s[3];
        let col = colour & 0x1f;
        let flipx = colour & 0x20 != 0;
        let flipy = colour & 0x40 != 0;
        if colour & 0x80 != 0 {
            x += v.output[OUT_XOFFS] as i32;
            y += v.output[OUT_YOFFS] as i32;
        }
        let pmask = primasks[priority];
        if colour & 0xff00 != 0 {
            let nx = ((colour & 0x0f00) >> 8) as usize + 1;
            let ny = ((colour & 0xf000) >> 12) as usize + 1;
            for nys in 0..ny {
                let sy = (y + nys as i32 * 16 + yoffs) & 0x3ff;
                for nxs in 0..nx {
                    let sx = (x + nxs as i32 * 16 + xoffs) & 0x3ff;
                    let cx = if flipx { nx - 1 - nxs } else { nxs };
                    let cy = if flipy { ny - 1 - nys } else { nys };
                    let c = if !flipx && !flipy {
                        (code & !0xf) + ((code + nxs) & 0xf) + 0x10 * nys
                    } else {
                        code + cx + 0x10 * cy
                    };
                    draw_tile16(atlas, c % elements, col, flipx, flipy, sx, sy, pmask, pens, prio);
                }
            }
        } else {
            draw_tile16(atlas, code % elements, col, flipx, flipy, (x + xoffs) & 0x3ff, (y + yoffs) & 0x3ff, pmask, pens, prio);
        }
        i -= 4;
    }
}

/// drawgfx prio_transpen: pixel drawn if ((1 << prio) & pmask) == 0; prio becomes 31.
/// MAME forces bit 31 of the mask: a pixel already covered by a sprite (prio 31) blocks the
/// sprites drawn afterwards. Since the table is walked from end to start, the entry
/// with the highest index is the one that stays visible.
fn draw_tile16(atlas: &[u8], code: usize, col: u16, flipx: bool, flipy: bool, sx: i32, sy: i32,
               pmask: u32, pens: &mut [u16], prio: &mut [u8]) {
    const CELL: usize = TileAtlas::CELL;
    let cell = &atlas[code * TileAtlas::CELL_PENS..][..TileAtlas::CELL_PENS];
    let pmask = pmask | 1 << 31;
    let pen_base = col * 16;
    // columns of the cell that fall on the screen
    let left = sx - VIS_X0;
    let px0 = (-left).clamp(0, CELL as i32) as usize;
    let px1 = (SCREEN_W as i32 - left).clamp(0, CELL as i32) as usize;
    if px0 >= px1 {
        return;
    }
    for py in 0..CELL {
        let by = sy + py as i32 - VIS_Y0;
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
