//! A frame as drawing commands: what the game shows, for any renderer (docs/fin.md, L20).
//!
//! A [`DrawList`] is a view, without copies, on the video state of a frame, decoded from the CPS2 registers into what a
//! renderer needs:
//!
//! * the [`Palette`]: 0xc00 colours in the CPS2 format (brightness, red, green, blue on 4 bits each: [`rgb`]); the
//!   sprites use the colours 0 to 0x1ff, the layers from 0x200 on; fades and flashes are palette changes;
//! * the three scroll [`Layer`]s (tiles of 8, 16 and 32 pixels, 64 x 64 tiles each), their scrolling, the row
//!   scrolling of layer 2 (raster effects) and the order in which they are drawn;
//! * the [`Sprites`] (16 x 16 cells, blocks of cells) and, for the sprites that show a frame of the game's frame table
//!   (`sprites/frames.json`), which frame it is ([`SpriteFrame`]): what a high-definition pack replaces;
//! * the masks that hide a sprite behind the layers ([`DrawList::sprite_masks`]).
//!
//! The tile and cell codes designate the graphics of the [`TileAtlas`](crate::TileAtlas). The CPS2 renderer
//! ([`render`](crate::render)) draws exactly the image of the hardware from a draw list; another renderer reads the same
//! fields. The game builds its draw list from its video state ([`DrawList::from_video`]) and adds the frame identities
//! ([`DrawList::with_frames`]).

use crate::VideoState;

/// Number of colours of the palette.
pub const COLORS: usize = 0xc00;
/// Colours of a palette page (32 palettes of 16 colours).
const PAGE: usize = 0x200;

/// Visible area in MAME's 512x262 bitmap (CPS_HBEND, CPS_VBEND): the screen pixel (0, 0) is at (64, 16).
pub const VISIBLE_X: i32 = 64;
pub const VISIBLE_Y: i32 = 16;

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

/// The frame to draw (see the module header).
#[derive(Clone, Copy)]
pub struct DrawList<'a> {
    pub palette: Palette<'a>,
    /// Layers 1 to 3 (index 0 to 2): tiles of 8, 16 and 32 pixels.
    pub layers: [Layer<'a>; 3],
    /// The layers drawn, from the back to the front: slot `k` holds the index of a layer, or nothing (a disabled
    /// layer or an empty slot). Slot `k` is bit `k` of the layer mask of a pixel ([`DrawList::sprite_masks`]).
    pub order: [Option<usize>; 3],
    /// A sprite of priority `p` (0 to 7) is hidden on a pixel whose set `s` of slots with an opaque layer pixel
    /// (bit `k`: slot `k`) has bit `s` set in `sprite_masks[p]`.
    pub sprite_masks: [u32; 8],
    pub sprites: Sprites<'a>,
    /// The frames of the game's frame table shown by groups of sprites (none for a video state alone).
    pub frames: &'a [SpriteFrame],
}

impl<'a> DrawList<'a> {
    /// The draw list of a CPS2 video state (`cps_state::screen_update_cps1`, `cps2_state::render_layers`).
    pub fn from_video(v: &VideoState<'a>) -> Self {
        let layer_control = v.cps_b[B_LAYER_CONTROL];
        let video_control = v.cps_a[A_VIDEOCONTROL];
        let enabled = [
            layer_control & LAYER_ENABLE_MASK[0] != 0,
            layer_control & LAYER_ENABLE_MASK[1] != 0 && video_control & 4 != 0,
            layer_control & LAYER_ENABLE_MASK[2] != 0 && video_control & 8 != 0,
        ];
        let row_scroll = (video_control & 1 != 0).then(|| RowScroll {
            table: gfx_base(v.cps_a[A_OTHER_BASE], 0x800),
            offset: v.cps_a[A_ROWSCROLL_OFFS] as usize,
        });
        let layers = [0, 1, 2].map(|i| Layer {
            tile_size: 8 << i,
            palette_base: 0x20 * (i as u16 + 1),
            ram: v.gfxram,
            base: gfx_base(v.cps_a[A_SCROLL_BASE[i]], 0x4000),
            scroll_x: v.cps_a[A_SCROLLX[i]] as i32,
            scroll_y: v.cps_a[A_SCROLLY[i]] as i32,
            row_scroll: if i == 1 { row_scroll } else { None },
        });

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
        let slot = |k: usize| {
            let l = order[k] as usize;
            (l != 0 && enabled[l - 1]).then(|| l - 1)
        };

        DrawList {
            palette: Palette::from_video(v),
            layers,
            order: [slot(0), slot(1), slot(2)],
            sprite_masks: sprite_masks(lpri[0], lpri[1], lpri[2]),
            sprites: Sprites::from_video(v),
            frames: &[],
        }
    }

    /// The same list, whose sprites show `frames`.
    pub fn with_frames(self, frames: &'a [SpriteFrame]) -> Self {
        DrawList { frames, ..self }
    }

    /// The frames whose sprites are all in the sprite list (an entry beyond its end is not drawn).
    pub fn shown_frames(&self) -> impl Iterator<Item = &'a SpriteFrame> + '_ {
        let n = self.sprites.len();
        self.frames.iter().filter(move |f| f.first as usize + f.count as usize <= n)
    }
}

/// Masks of the sprites of each priority, from the priorities of the three layer slots (`cps2_state::cps2_set_sprite_priorities`).
fn sprite_masks(l0: u32, l1: u32, l2: u32) -> [u32; 8] {
    let mut masks = [0u32; 8];
    let mut mask0 = 0xaau32;
    let mut mask1 = 0xccu32;
    if l0 > l1 { mask0 &= !0x88; }
    if l0 > l2 { mask0 &= !0xa0; }
    if l1 > l2 { mask1 &= !0xc0; }
    masks[0] = 0xff;
    for i in 1..8u32 {
        if i <= l0 && i <= l1 && i <= l2 {
            masks[i as usize] = 0xfe;
            continue;
        }
        let mut m = 0;
        if i <= l0 { m |= mask0; }
        if i <= l1 { m |= mask1; }
        if i <= l2 { m |= 0xf0; }
        masks[i as usize] = m;
    }
    masks
}

/// cps1_base: address (word index in the GFX RAM) designated by a CPS-A register.
fn gfx_base(reg: u16, boundary: usize) -> usize {
    let base = (reg as usize * 256) & !(boundary - 1);
    (base & 0x3ffff) / 2
}

/// `i % len`, without the division in the usual case (`i` already below `len`).
#[inline]
fn wrap(i: usize, len: usize) -> usize {
    if i < len { i } else { i % len }
}

/// RGB of a CPS2 colour word (`cps1_build_palette`): bits 12-15 brightness, then red, green and blue on 4 bits.
pub fn rgb(color: u16) -> [u8; 3] {
    let bright = 0x0f + ((color >> 12) << 1) as u32;
    let c = |n: u16| ((n & 0x0f) as u32 * 0x11 * bright / 0x2d) as u8;
    [c(color >> 8), c(color >> 4), c(color)]
}

/// The palette: [`COLORS`] colours in six pages of 0x200, read from the palette memory. A page that the palette
/// control leaves off is black.
#[derive(Clone, Copy)]
pub struct Palette<'a> {
    ram: &'a [u16],
    /// Where each page starts in `ram`, if it is on.
    pages: [Option<usize>; 6],
}

impl<'a> Palette<'a> {
    fn from_video(v: &VideoState<'a>) -> Self {
        let (ram, base) = match v.palette_ram {
            Some(p) => (p, 0),
            None => (v.gfxram, gfx_base(v.cps_a[A_PALETTE_BASE], 0x400)),
        };
        let ctrl = v.cps_b[B_PALETTE_CONTROL];
        let mut pages = [None; 6];
        let mut src = base;
        for (page, start) in pages.iter_mut().enumerate() {
            if ctrl >> page & 1 != 0 {
                *start = Some(src);
                src += PAGE;
            } else if src != base {
                // a page off skips its words, except before the first page on
                src += PAGE;
            }
        }
        Palette { ram, pages }
    }

    /// Colour `i` (below [`COLORS`]), as a CPS2 word ([`rgb`]).
    #[inline]
    pub fn color(&self, i: usize) -> u16 {
        match self.pages[i / PAGE] {
            Some(start) => self.ram[wrap(start + i % PAGE, self.ram.len())],
            None => 0,
        }
    }
}

/// A tile of a layer: graphics code and attributes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Tile {
    pub code: u16,
    /// Bits 0-4: palette (from the layer's [`Layer::palette_base`]); bit 5: horizontal mirror; bit 6: vertical
    /// mirror.
    pub attr: u16,
}

impl Tile {
    pub fn palette(self) -> u16 {
        self.attr & 0x1f
    }
    pub fn flip_x(self) -> bool {
        self.attr & 0x20 != 0
    }
    pub fn flip_y(self) -> bool {
        self.attr & 0x40 != 0
    }
}

/// Row scrolling of layer 2: one horizontal offset per line, read from a table of the GFX RAM.
#[derive(Clone, Copy)]
struct RowScroll {
    table: usize,
    offset: usize,
}

/// A scroll layer: a tilemap of 64 x 64 tiles that wraps around, and its scrolling.
#[derive(Clone, Copy)]
pub struct Layer<'a> {
    /// Side of the tiles: 8, 16 or 32 pixels.
    pub tile_size: usize,
    /// First palette of the layer: the colours of a tile are `16 * (palette_base + tile.palette())` and the 15
    /// following ones (pen 15 is transparent).
    pub palette_base: u16,
    ram: &'a [u16],
    /// Word index of the tilemap in `ram`.
    base: usize,
    scroll_x: i32,
    scroll_y: i32,
    row_scroll: Option<RowScroll>,
}

impl Layer<'_> {
    /// Side of the tilemap in pixels.
    pub fn size(&self) -> usize {
        64 * self.tile_size
    }

    /// Mappers tilemap0_scan / tilemap1_scan / tilemap2_scan for tiles of side `T`: (col, row) -> tile index.
    #[inline]
    fn index<const T: usize>(col: usize, row: usize) -> usize {
        match T {
            8 => (row & 0x1f) + ((col & 0x3f) << 5) + ((row & 0x20) << 6),
            16 => (row & 0x0f) + ((col & 0x3f) << 4) + ((row & 0x30) << 6),
            _ => (row & 0x07) + ((col & 0x3f) << 3) + ((row & 0x38) << 6),
        }
    }

    /// The tile at (`col`, `row`) of a layer of tiles of side `T` (`T` is [`Layer::tile_size`]).
    #[inline]
    pub fn tile_of<const T: usize>(&self, col: usize, row: usize) -> Tile {
        let i = self.base + 2 * Self::index::<T>(col, row);
        Tile { code: self.ram[wrap(i, self.ram.len())], attr: self.ram[wrap(i + 1, self.ram.len())] }
    }

    /// The tile at (`col`, `row`) of the tilemap.
    pub fn tile(&self, col: usize, row: usize) -> Tile {
        match self.tile_size {
            8 => self.tile_of::<8>(col, row),
            16 => self.tile_of::<16>(col, row),
            _ => self.tile_of::<32>(col, row),
        }
    }

    /// Tilemap coordinates (x, y), in pixels, of the leftmost pixel of screen line `line`: the tilemap pixel shown at
    /// (`x0`, `line`) is (`(x + x0) % size`, `y`).
    #[inline]
    pub fn origin(&self, line: usize) -> (usize, usize) {
        let by = line as i32 + VISIBLE_Y;
        let mut scroll_x = self.scroll_x;
        if let Some(r) = self.row_scroll {
            scroll_x += self.ram[wrap(r.table + ((by as usize + r.offset) & 0x3ff), self.ram.len())] as i32;
        }
        let size = self.size() as i32;
        (((VISIBLE_X + scroll_x).rem_euclid(size)) as usize, ((by + self.scroll_y).rem_euclid(size)) as usize)
    }
}

/// The sprite list: the object table latched at the last vblank, up to its end marker.
#[derive(Clone, Copy)]
pub struct Sprites<'a> {
    entries: &'a [u16],
    len: usize,
    /// Object output offsets (x, y), added to the sprites that ask for them.
    offset: (i32, i32),
}

impl<'a> Sprites<'a> {
    fn from_video(v: &VideoState<'a>) -> Self {
        let obj = v.objram;
        // find_last_sprite: the first entry with y >= 0x8000 or attributes >= 0xff00 ends the list
        let mut len = obj.len() / 4;
        let mut off = 0;
        while off + 3 < obj.len() {
            if obj[off + 1] >= 0x8000 || obj[off + 3] >= 0xff00 {
                len = off / 4;
                break;
            }
            off += 4;
        }
        Sprites { entries: obj, len, offset: (v.output[OUT_XOFFS] as i32, v.output[OUT_YOFFS] as i32) }
    }

    /// Number of entries. They are drawn from the last to the first: the first one stays in front.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Entry `i` (below [`Sprites::len`]).
    #[inline]
    pub fn get(&self, i: usize) -> Sprite {
        let s = &self.entries[4 * i..4 * i + 4];
        let (mut x, mut y) = (s[0] as i32, s[1] as i32);
        let colour = s[3];
        let code = s[2] as usize + (((y & 0x6000) as usize) << 3);
        let priority = ((x >> 13) & 7) as usize;
        if colour & 0x80 != 0 {
            x += self.offset.0;
            y += self.offset.1;
        }
        let (columns, rows) = if colour & 0xff00 != 0 {
            (((colour & 0x0f00) >> 8) as usize + 1, ((colour & 0xf000) >> 12) as usize + 1)
        } else {
            (1, 1)
        };
        Sprite {
            x: x + 64 - self.offset.0,
            y: y + 16 - self.offset.1,
            code,
            palette: colour & 0x1f,
            flip_x: colour & 0x20 != 0,
            flip_y: colour & 0x40 != 0,
            priority,
            columns,
            rows,
        }
    }
}

/// A sprite: a block of `columns` x `rows` cells of 16 x 16 pixels.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Sprite {
    /// Position in the 1024 x 1024 sprite space, before wrapping (see [`Sprite::cell`]).
    x: i32,
    y: i32,
    /// Code of the first cell.
    pub code: usize,
    /// Palette (0 to 31): colours `16 * palette` to `16 * palette + 14`, pen 15 is transparent.
    pub palette: u16,
    pub flip_x: bool,
    pub flip_y: bool,
    /// Priority against the layers (index of [`DrawList::sprite_masks`]).
    pub priority: usize,
    pub columns: usize,
    pub rows: usize,
}

impl Sprite {
    /// Cell (`column`, `row`) of the block: its graphics code and the screen position of its top left pixel (the
    /// block wraps around the 1024 x 1024 sprite space cell by cell). The code may exceed the graphics: the renderer
    /// takes it modulo the number of cells.
    #[inline]
    pub fn cell(&self, column: usize, row: usize) -> (usize, i32, i32) {
        let x = ((self.x + 16 * column as i32) & 0x3ff) - VISIBLE_X;
        let y = ((self.y + 16 * row as i32) & 0x3ff) - VISIBLE_Y;
        let cx = if self.flip_x { self.columns - 1 - column } else { column };
        let cy = if self.flip_y { self.rows - 1 - row } else { row };
        let code = if !self.flip_x && !self.flip_y {
            (self.code & !0xf) + ((self.code + column) & 0xf) + 0x10 * row
        } else {
            self.code + cx + 0x10 * cy
        };
        (code, x, y)
    }
}

/// A frame of the game's frame table (`sprites/frames.json`) drawn as consecutive sprite entries: what a
/// high-definition pack replaces by its own image of the frame.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SpriteFrame {
    /// First entry in the sprite list, and number of entries (one per piece of the frame).
    pub first: u16,
    pub count: u16,
    /// Frame number: index in the frame table (numbers past the table: the frames outside it).
    pub frame: u32,
    /// Anchor of the frame (the point its piece offsets start from), in the coordinates of the sprite entries; the
    /// pieces of a mirrored frame are placed leftwards from it.
    pub x: u16,
    pub y: u16,
    /// Mirrored horizontally.
    pub flip: bool,
}

impl SpriteFrame {
    /// Screen position of the anchor in `list` (as [`Sprite::cell`] places a cell).
    pub fn position(&self, list: &DrawList) -> (i32, i32) {
        let (ox, oy) = list.sprites.offset;
        (((self.x as i32 + 64 - ox) & 0x3ff) - VISIBLE_X, ((self.y as i32 + 16 - oy) & 0x3ff) - VISIBLE_Y)
    }

    /// Palette of the frame: that of its first sprite.
    pub fn palette(&self, list: &DrawList) -> u16 {
        list.sprites.get(self.first as usize).palette
    }
}
