//! Rendu d'une image CPS2 à partir de l'état vidéo (GFX RAM, registres CPS-A/CPS-B, Object RAM).
//! Traduction de cps_state::screen_update_cps1 / cps2_state::render_layers / cps2_render_sprites.

pub const SCREEN_W: usize = 384;
pub const SCREEN_H: usize = 224;
/// Zone visible dans la bitmap 512x262 de MAME (CPS_HBEND, CPS_VBEND).
const VIS_X0: i32 = 64;
const VIS_Y0: i32 = 16;

// CPS-A (index de mots)
const A_SCROLL_BASE: [usize; 3] = [0x02 / 2, 0x04 / 2, 0x06 / 2];
const A_OTHER_BASE: usize = 0x08 / 2;
const A_PALETTE_BASE: usize = 0x0a / 2;
const A_SCROLLX: [usize; 3] = [0x0c / 2, 0x10 / 2, 0x14 / 2];
const A_SCROLLY: [usize; 3] = [0x0e / 2, 0x12 / 2, 0x16 / 2];
const A_ROWSCROLL_OFFS: usize = 0x20 / 2;
const A_VIDEOCONTROL: usize = 0x22 / 2;
// CPS-B, configuration CPS_B_21_DEF (commune aux jeux CPS2)
const B_LAYER_CONTROL: usize = 0x26 / 2;
const B_PALETTE_CONTROL: usize = 0x30 / 2;
const LAYER_ENABLE_MASK: [u16; 3] = [0x02, 0x04, 0x08];
// Sortie objets CPS2 (0x400000)
const OUT_PRI: usize = 0x04 / 2;
const OUT_XOFFS: usize = 0x08 / 2;
const OUT_YOFFS: usize = 0x0a / 2;

/// Base des tuiles de décor dans la région gfx (mapper_cps2 : banque 1, 0x20000 unités de 64 octets).
const SCROLL_TILE_BASE: usize = 0x20000 * 64;

pub struct VideoState<'a> {
    /// GFX RAM 0x900000-0x92FFFF, en mots.
    pub gfxram: &'a [u16],
    pub cps_a: &'a [u16; 32],
    pub cps_b: &'a [u16; 32],
    /// Table d'objets verrouillée au vblank (0x1000 mots).
    pub objram: &'a [u16],
    pub output: &'a [u16; 6],
    /// Copie de la GFX RAM faite à l'écriture du registre de base de palette (6 x 0x200 mots).
    /// C'est ce que le matériel affiche ; None = lire la GFX RAM courante.
    pub palette_ram: Option<&'a [u16]>,
}

/// Palette 0xC00 couleurs en RGB, construite comme cps1_build_palette.
pub fn build_palette(v: &VideoState) -> Vec<[u8; 3]> {
    let mut pal = vec![[0u8; 3]; 0xc00];
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

/// cps1_base : adresse (index de mot dans la GFX RAM) désignée par un registre CPS-A.
fn gfx_base(reg: u16, boundary: usize) -> usize {
    let base = (reg as usize * 256) & !(boundary - 1);
    (base & 0x3ffff) / 2
}

/// Pixel (0..15) d'une ligne graphique : `row` pointe sur 4 octets (un plan par octet, MSB = pixel 0).
#[inline]
fn pixel(gfx: &[u8], row: usize, x: usize) -> u8 {
    let bit = 7 - x;
    ((gfx[row + 3] >> bit) & 1) << 3
        | ((gfx[row + 2] >> bit) & 1) << 2
        | ((gfx[row + 1] >> bit) & 1) << 1
        | ((gfx[row] >> bit) & 1)
}

struct Layer {
    tile: usize,     // 8, 16, 32
    base: usize,     // index de mot dans la GFX RAM
    color_base: u16, // 0x20, 0x40, 0x60
}

impl Layer {
    /// Mappers tilemap0_scan / tilemap1_scan / tilemap2_scan : (col, row) -> index de tuile.
    fn tile_index(&self, col: usize, row: usize) -> usize {
        match self.tile {
            8 => (row & 0x1f) + ((col & 0x3f) << 5) + ((row & 0x20) << 6),
            16 => (row & 0x0f) + ((col & 0x3f) << 4) + ((row & 0x30) << 6),
            _ => (row & 0x07) + ((col & 0x3f) << 3) + ((row & 0x38) << 6),
        }
    }

    /// Pen (couleur*16 + pixel) au point (tx, ty) du tilemap, ou None si transparent.
    fn sample(&self, v: &VideoState, gfx: &[u8], tx: usize, ty: usize) -> Option<u16> {
        let t = self.tile;
        let idx = self.tile_index(tx / t, ty / t);
        let code = v.gfxram[(self.base + 2 * idx) % v.gfxram.len()] as usize;
        let attr = v.gfxram[(self.base + 2 * idx + 1) % v.gfxram.len()];
        let mut px = tx % t;
        let mut py = ty % t;
        if attr & 0x20 != 0 {
            px = t - 1 - px;
        }
        if attr & 0x40 != 0 {
            py = t - 1 - py;
        }
        let row = match t {
            8 => {
                // 8x8 : moitié gauche/droite d'une ligne de 16 pixels selon la colonne (gfxset)
                let half = (idx >> 5) & 1;
                SCROLL_TILE_BASE + code * 64 + py * 8 + half * 4
            }
            16 => SCROLL_TILE_BASE + code * 128 + py * 8 + (px / 8) * 4,
            _ => SCROLL_TILE_BASE + (code & 0x3fff) * 512 + py * 16 + (px / 8) * 4,
        };
        if row + 4 > gfx.len() {
            return None;
        }
        let p = pixel(gfx, row, px % 8);
        if p == 15 {
            None
        } else {
            Some(((attr & 0x1f) + self.color_base) * 16 + p as u16)
        }
    }
}

/// Rend une image 384x224 en RGB.
pub fn render(v: &VideoState, gfx: &[u8]) -> Vec<[u8; 3]> {
    let pal = build_palette(v);
    const W: usize = SCREEN_W;
    const H: usize = SCREEN_H;
    let mut pens = vec![0xbffu16; W * H];
    let mut prio = vec![0u8; W * H];

    let layer_control = v.cps_b[B_LAYER_CONTROL];
    let video_control = v.cps_a[A_VIDEOCONTROL];
    let layers = [
        Layer { tile: 8, base: gfx_base(v.cps_a[A_SCROLL_BASE[0]], 0x4000), color_base: 0x20 },
        Layer { tile: 16, base: gfx_base(v.cps_a[A_SCROLL_BASE[1]], 0x4000), color_base: 0x40 },
        Layer { tile: 32, base: gfx_base(v.cps_a[A_SCROLL_BASE[2]], 0x4000), color_base: 0x60 },
    ];
    let enabled = [
        layer_control & LAYER_ENABLE_MASK[0] != 0,
        layer_control & LAYER_ENABLE_MASK[1] != 0 && video_control & 4 != 0,
        layer_control & LAYER_ENABLE_MASK[2] != 0 && video_control & 8 != 0,
    ];
    let other = gfx_base(v.cps_a[A_OTHER_BASE], 0x800);
    let rowscroll = video_control & 1 != 0;

    // Ordre des couches : 4 groupes de 2 bits, 0 = sprites, 1..3 = scroll1..3.
    let mut order = [
        (layer_control >> 6) & 3,
        (layer_control >> 8) & 3,
        (layer_control >> 10) & 3,
        (layer_control >> 12) & 3,
    ];
    let pri_ctrl = v.output[OUT_PRI];
    let mut lpri = order.map(|l| ((pri_ctrl >> (4 * l)) & 0x0f) as u32);
    // retire la couche sprites CPS1 de l'ordre
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
        let size = 64 * layer.tile;
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
            for x in 0..W {
                let tx = ((x as i32 + VIS_X0 + scrollx).rem_euclid(size as i32)) as usize;
                if let Some(pen) = layer.sample(v, gfx, tx, ty) {
                    pens[y * W + x] = pen;
                    prio[y * W + x] |= pbit;
                }
            }
        }
    }

    draw_sprites(v, gfx, &primasks, &mut pens, &mut prio);

    pens.iter().map(|&p| pal[p as usize % pal.len()]).collect()
}

fn draw_sprites(v: &VideoState, gfx: &[u8], primasks: &[u32; 8], pens: &mut [u16], prio: &mut [u8]) {
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
    let elements = gfx.len() / 128;

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
                    draw_tile16(gfx, c % elements, col, flipx, flipy, sx, sy, pmask, pens, prio);
                }
            }
        } else {
            draw_tile16(gfx, code % elements, col, flipx, flipy, (x + xoffs) & 0x3ff, (y + yoffs) & 0x3ff, pmask, pens, prio);
        }
        i -= 4;
    }
}

/// drawgfx prio_transpen : pixel dessiné si ((1 << prio) & pmask) == 0 ; prio passe à 31.
/// MAME force le bit 31 du masque : un pixel déjà couvert par un sprite (prio 31) bloque les
/// sprites dessinés ensuite. La table étant parcourue de la fin vers le début, c'est l'entrée
/// d'indice le plus élevé qui reste visible.
#[allow(clippy::too_many_arguments)]
fn draw_tile16(gfx: &[u8], code: usize, col: u16, flipx: bool, flipy: bool, sx: i32, sy: i32,
               pmask: u32, pens: &mut [u16], prio: &mut [u8]) {
    let base = code * 128;
    let pmask = pmask | 1 << 31;
    for py in 0..16 {
        let by = sy + py as i32 - VIS_Y0;
        if by < 0 || by >= SCREEN_H as i32 {
            continue;
        }
        let ry = if flipy { 15 - py } else { py };
        for px in 0..16 {
            let bx = sx + px as i32 - VIS_X0;
            if bx < 0 || bx >= SCREEN_W as i32 {
                continue;
            }
            let rx = if flipx { 15 - px } else { px };
            let p = pixel(gfx, base + ry * 8 + (rx / 8) * 4, rx % 8);
            if p == 15 {
                continue;
            }
            let o = by as usize * SCREEN_W + bx as usize;
            if (1u32 << (prio[o] & 0x1f)) & pmask == 0 {
                pens[o] = col * 16 + p as u16;
            }
            prio[o] = 31;
        }
    }
}
