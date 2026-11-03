//! Video primitives (V1) and tilemaps (V2): layers, CPS-A/B registers, Object RAM, tile blocks.
//!
//! See `docs/video.md` for the formats. The registers `0x804100-0x80411f` (CPS-A) contain the
//! addresses of the tables in GFX RAM (in units of 256 bytes): objects at `0x804100`, layers 1 to 3 at
//! `0x804102/04/06`; `0x804166` (CPS-B) enables the layers.

use cps2::Cps2Bus;

use crate::data::data;
use crate::hw::video::{self as gfx, regs, Cell, Vram};
use crate::ram::{Byte, Word};
use crate::tables::engine;
use crate::Native;

/// Copy of the video control register (`regs::VIDEO_CONTROL`; bit 0: per-line scroll of layer 2).
pub const VIDEO_CONTROL: Word = Word::a5(0x12);
/// Image of the layer enable register (written at `regs::LAYER_CONTROL`).
pub const LAYER_ENABLE: Word = Word::a5(0x14);
/// Base of the object table (value of `regs::OBJECT_BASE`).
const OBJECT_TABLE: Word = Word::a5(0x5ba);
/// Object RAM bank toggle (byte, inverted at each clearing and each sprite list built).
pub(crate) const OBJECT_BANK: Byte = Byte::a5(0x5bc);
/// Coin locks: two images of the output (`+0x10`, `+0x17`) and the forced bits (`+0x18`).
const LOCKOUTS: [Byte; 2] = [Byte::a5(0x10), Byte::a5(0x17)];
const LOCKOUT_FORCED: Byte = Byte::a5(0x18);
/// Non-zero: the display registers are no longer written (frozen screen), `A5-0x7155`.
const FROZEN: Byte = Byte(0xff0eab);

/// Addresses of the graphics memory, for the code that computes places in it (`crate::hw::video` for the typed
/// places).
pub mod hw {
    use crate::hw::video as gfx;

    pub const TEXT_LAYER: u32 = gfx::TEXT.base;
    pub const SCROLL1: u32 = gfx::SCROLL1.base;
    pub const SCROLL2: u32 = gfx::SCROLL2.0;
    pub const SCROLL3: u32 = gfx::SCROLL3.0;
    pub const PALETTES: u32 = gfx::PALETTES.0;
    pub const OBJECTS: u32 = gfx::OBJECTS.0;
}

pub(crate) fn set_low_word(r: &mut u32, v: u16) {
    *r = *r & 0xffff_0000 | v as u32;
}

// ---------------------------------------------------------------------------------------------
// V1: loading gauge, locks, layers, clearings
// ---------------------------------------------------------------------------------------------

/// The gauge occupies the first 32 longs of the character layer (the first column).
pub(crate) const GAUGE_LONGS: u32 = 32;

/// 0xb46: fills the first column of the character layer with `marker` (the loading gauge), unless
/// the settings hide it (bit 0x400 of DSW: always, in this game). Returns true if the gauge is drawn.
pub fn update_gauge(b: &mut Cps2Bus, marker: u32) -> bool {
    let hidden = crate::ram::DSW.get(b) & 0x400 != 0;
    if !hidden {
        for i in 0..GAUGE_LONGS {
            gfx::TEXT.cell(0, i).set_long(b, marker);
        }
    }
    !hidden
}

/// `update_gauge` called by the world update (class marker `marker`, D1 of the original). Original glitch: the D2.w
/// that 0xb46 leaves (0x400 of its DSW test when the gauge is hidden, always in this game; 0xFFFF when it is drawn) is
/// reread by a following handler, which writes it into an object field (`DDSOM_POISON=call_b3e`: long_nain_57_86,
/// frame 13398, ram ffd692): it is the leftover D2 of the game (`dispatch::Leftovers`). A0 and the flags are reread by
/// nobody.
pub fn gauge_in_world(b: &mut Cps2Bus, marker: u32) {
    let drawn = update_gauge(b, marker);
    let leftovers = &mut crate::tasks::table::table(b).leftovers;
    leftovers.d2 = if drawn { leftovers.d2 | 0xffff } else { leftovers.d2 & 0xffff_0000 | 0x400 };
}

/// 0xb96: locks the coin counters of bits `bits` (in addition to the forced bits). Returns the forced bits (D1.b).
pub fn lock_counters(b: &mut Cps2Bus, bits: u8) -> u8 {
    let forced = LOCKOUT_FORCED.get(b);
    for lock in LOCKOUTS {
        let v = lock.get(b) | bits | forced;
        lock.set(b, v);
    }
    forced
}

/// 0xbac: unlocks bits `bits` (except the forced bits). Returns the forced bits (D1.b).
pub fn unlock_counters(b: &mut Cps2Bus, bits: u8) -> u8 {
    let forced = LOCKOUT_FORCED.get(b);
    for lock in LOCKOUTS {
        let v = lock.get(b) & !bits | forced;
        lock.set(b, v);
    }
    forced
}

/// 0xbc6: enables the layers `bits` (added to the register image) and programs the display registers, unless
/// the screen is frozen. No register or flag changes.
pub fn turn_layers_on(b: &mut Cps2Bus, bits: u16) {
    let enable = LAYER_ENABLE.get(b) | bits;
    LAYER_ENABLE.set(b, enable);
    if !FROZEN.is_set(b) {
        regs::LAYER_CONTROL.set(b, enable);
        let objects = OBJECT_TABLE.get(b);
        regs::OBJECT_BASE.set(b, objects);
        for (k, base) in [hw::SCROLL1, hw::SCROLL2, hw::SCROLL3].into_iter().enumerate() {
            regs::SCROLL_BASES.nth(k as u32).set(b, (base >> 8) as u16);
        }
    }
}

/// 0xbfc: disables the layers `bits`. The processor receives D0.w complemented.
pub fn turn_layers_off(b: &mut Cps2Bus, bits: u16) {
    let enable = LAYER_ENABLE.get(b) & !bits;
    LAYER_ENABLE.set(b, enable);
    regs::LAYER_CONTROL.set(b, enable);
}

/// 0xc58: writes the end-of-list entry (0x8000, 0x18000000) at the head of the Object RAM, twice,
/// toggling the bank between the two: the list is empty in both banks.
pub fn clear_objects(b: &mut Cps2Bus) {
    for _ in 0..2 {
        gfx::OBJECTS.set_long(b, 0x8000);
        gfx::OBJECTS.offset(4).set_long(b, 0x1800_0000);
        let bank = OBJECT_BANK.get(b) ^ 0xff;
        OBJECT_BANK.set(b, bank);
        regs::OBJECT_BANK.set(b, bank);
    }
}

/// Black screen, first half (0xc0c, 0xc32): all layers off (`0xbfc` with D0 = 0x12ce, after
/// resetting `A5+0x14` to zero).
fn switch_layers_off(b: &mut Cps2Bus) {
    LAYER_ENABLE.set(b, 0);
    regs::LAYER_CONTROL.set(b, 0);
}

/// Black screen, second half: empty object list and the three layers cleared.
fn clear_screen(b: &mut Cps2Bus) {
    clear_objects(b);
    for layer in [Layer::One, Layer::Two, Layer::Three] {
        clear_layer(b, layer);
    }
}

/// 0xc0c: black screen in one go (layers off, objects and layers cleared). The registers that the original leaves
/// (D0.w = 0x1f00, D1 = 0) are reread by nobody.
pub fn black_screen(b: &mut Cps2Bus) {
    switch_layers_off(b);
    clear_screen(b);
}

/// 0xc32: black screen in three frames (subroutine that cedes, `0xb64`, before and after the clearing): a
/// sub-machine ([`black_screen::Step`]) that the Rust steps call directly (`Sequence::subroutine`); nothing
/// calls it by address any more.
pub mod black_screen {
    use crate::modes::Flow;
    use crate::scene::Ctx;
    use crate::tasks::one_frame;

    /// Layers off, then one frame.
    pub fn start(x: &mut Ctx) -> Flow {
        super::switch_layers_off(x.b);
        one_frame(x, Step::Clear)
    }

    crate::steps! {
        /// Steps of the black screen.
        pub enum Step {
            /// Objects and layers cleared, then one frame.
            Clear = 0xc42 => |x| {
                super::clear_screen(x.b);
                one_frame(x, Step::End)
            },
            /// Return to the caller.
            End = 0xc56 => |_| Flow::Done,
        }
    }
}

/// Fills [start, end) with cells (tile, 0): 0xca8 (scroll 1), 0xcc8 (scroll 2), 0xce8 (scroll 3).
fn fill_layer(b: &mut Cps2Bus, from: u32, to: u32, tile: u16) {
    for a in (from..to).step_by(4) {
        Cell(a).put(b, tile, 0);
    }
}

/// The three scroll layers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Layer {
    One,
    Two,
    Three,
}

impl Layer {
    /// Start and end of the layer in GFX RAM, and its empty tile.
    fn zone(self) -> (u32, u32, u16) {
        match self {
            Layer::One => (hw::SCROLL1, hw::SCROLL2, 0x20),
            Layer::Two => (hw::SCROLL2, hw::SCROLL3, 0x1800),
            Layer::Three => (hw::SCROLL3, hw::PALETTES, 0x1f00),
        }
    }
}

/// 0xca8, 0xcc8, 0xce8: fills the layer with its empty tile. The registers that the original leaves (D0.w = the
/// tile, D1 = 0; A0 and A1 are restored) are reread by nobody.
pub fn clear_layer(b: &mut Cps2Bus, layer: Layer) {
    let (from, to, tile) = layer.zone();
    fill_layer(b, from, to, tile);
}

fn clear_scroll1(b: &mut Cps2Bus) {
    clear_layer(b, Layer::One);
}
/// Bytes cleared by 0x3525c.
pub(crate) const SCROLL1_START: u32 = 0x2000;

/// 0x3525c: clears the first 8 KB of layer 1 with the empty tile 0x1f.
pub fn clear_layer1_start(b: &mut Cps2Bus) {
    fill_layer(b, hw::SCROLL1, hw::SCROLL1 + SCROLL1_START, 0x1f);
}

fn clear_scroll2(b: &mut Cps2Bus) {
    clear_layer(b, Layer::Two);
}
fn clear_scroll3(b: &mut Cps2Bus) {
    clear_layer(b, Layer::Three);
}

const fn memory(entry: u32, name: &'static str, run: fn(&mut Cps2Bus)) -> Native {
    Native { entry, name, run: crate::Body::Memory(run), touches_sound: false, outputs: 0 }
}

pub const BLANK_SCREEN: Native = memory(0xc0c, "video_black_screen", black_screen);
pub const CLEAR_OBJECTS: Native = memory(0xc58, "video_clear_objects", clear_objects);
pub const CLEAR_SCROLL1: Native = memory(0xca8, "video_clear_scroll1", clear_scroll1);
pub const CLEAR_SCROLL2: Native = memory(0xcc8, "video_clear_scroll2", clear_scroll2);
pub const CLEAR_SCROLL3: Native = memory(0xce8, "video_clear_scroll3", clear_scroll3);

// ---------------------------------------------------------------------------------------------
// V2: terrain attribute under a point
// ---------------------------------------------------------------------------------------------

/// Terrain attribute under a point, and the computation leftovers that the original leaves in its registers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Terrain {
    /// Terrain number: bits 7 to 15 of the cell's attribute word (D2.w; 0: free ground).
    pub number: u16,
    /// Corrected coordinates (D0.w, D1.w): y shifted by -/+ 0x10 on the fixed maps, camera 2 otherwise.
    pub x: u16,
    pub y: u16,
    /// Address of the cell (A1): fixed map in ROM, or scroll layer 2 in GFX RAM.
    pub case: u32,
    /// Index of the cell in the map (D3.w, whole D3 on the fixed maps) and column (D4.w).
    pub index: u16,
    pub column: u16,
    /// Fixed map of scenes 2 and 0x14 (D2 is then only written as a low word; elsewhere D2 = 0 first).
    pub fixed_map: bool,
}

/// Terrain number under point (x, y) (0x24b0). Scenes 2 and 0x14: fixed map in ROM (y shifted by -/+ 0x10);
/// elsewhere: cell of scroll layer 2 in GFX RAM, with the camera correction when camera 2 is
/// current.
pub fn terrain(b: &mut Cps2Bus, x: u16, y: u16) -> Terrain {
    let stage = crate::ram::SCENE.get(b);
    let (mut x, mut y) = (x, y);
    // the fixed maps of `terrain.json` (scenes 2 and 0x14), with their correction of y
    let fixed_map = data().terrain.maps.iter().find(|m| m.stage == stage).map(|m| (m.rom, m.dy as u16));
    let (word, case, index, column) = if let Some((map, dy)) = fixed_map {
        y = y.wrapping_add(dy);
        let row = 0x1e - ((y as u8 >> 3) & 0x1e) as u16;
        let column = x.wrapping_add(x) & 0xffe0;
        let index = row.wrapping_add(column);
        let case = map.wrapping_add(index as i16 as u32);
        (engine().word(b, case), case, index, column)
    } else {
        use crate::ram::{CAMERA_ALT_ACTIVE, CAMERA_ALT_X, CAMERA_ALT_Y, CAMERA_X, CAMERA_Y};
        if CAMERA_ALT_ACTIVE.is_set(b) {
            x = x.wrapping_add(CAMERA_X.get(b).wrapping_sub(CAMERA_ALT_X.get(b)));
            y = y.wrapping_add(CAMERA_Y.get(b).wrapping_sub(CAMERA_ALT_Y.get(b)));
        }
        let page = 0x3000u16.wrapping_sub((y & 0x300) << 4);
        let row = 0x3c - ((y as u8 >> 2) & 0x3c) as u16;
        let column = x.wrapping_shl(2) & 0xfc0;
        let index = page.wrapping_add(row).wrapping_add(column);
        let case = hw::SCROLL2.wrapping_add(index as i16 as u32);
        (Cell(case).attr(b), case, index, column)
    };
    Terrain { number: (word & 0xff80) >> 7, x, y, case, index, column, fixed_map: fixed_map.is_some() }
}

/// Registers left by 0x24b0: D0/D1 (corrected coordinates), D2 (number), D3/D4 (computation leftovers, read by
/// callers that take them as arguments), A1 (cell) and Z (zero number, the other flags cleared).
pub(crate) fn terrain_regs(r: &mut impl crate::ops::Alu, t: &Terrain) {
    if t.fixed_map {
        r.d()[3] = t.index as u32;
    } else {
        set_low_word(&mut r.d()[3], t.index);
        r.d()[2] = 0;
    }
    set_low_word(&mut r.d()[4], t.column);
    r.a()[1] = t.case;
    set_low_word(&mut r.d()[0], t.x);
    set_low_word(&mut r.d()[1], t.y);
    set_low_word(&mut r.d()[2], t.number);
    r.flags(crate::ops::ALL, if t.number == 0 { crate::ops::Z } else { 0 });
}

/// Terrain number under object `o` (0x24a8: x position at +8, y at +0x10).
pub fn terrain_of(b: &mut Cps2Bus, o: crate::objects::Obj) -> Terrain {
    use crate::objects::Obj;
    let (x, y) = (o.u16(b, Obj::X), o.u16(b, Obj::Y));
    terrain(b, x, y)
}

// ---------------------------------------------------------------------------------------------
// V2: tile block loaders
// ---------------------------------------------------------------------------------------------

/// Geometry of a loader: D0 chooses a grid of `grid` x `grid` block indices (words of the scenery maps of
/// `levels.json`, read by address as the original computes them); each block is `cells` x `cells` cells, each cell a
/// long (tile, attribute) copied from the map with `code_offset` added to the tile. A1 is the destination in GFX RAM.
struct Blocks {
    index_base: u32,
    /// Index bytes per value of D0.
    index_size: u32,
    grid: u32,
    cells: u32,
    /// Bytes per row of cells in the destination.
    row_stride: u32,
    code_offset: u16,
    /// If present, the destination goes back to the start of its page (of this size) at the end of a row of blocks.
    page: Option<u32>,
    source: Source,
}

enum Source {
    /// A single block table of 16 cells.
    Fixed(u32),
    /// Several block tables of `cells`² cells, chosen from the position of the index in the
    /// index ROM: selector = `((position & mask) << shift)` swapped by halves (SWAP), signed
    /// word; address of the table at `list + selector` (the table of addresses `list` of `data/engine.json`, read in
    /// program space if `program`, PC-relative access, otherwise in data space, access through an address register).
    Banks { origin: u32, list: &'static str, mask: u32, shift: u32, program: bool },
}

const LOAD_4X4: Blocks = Blocks {
    index_base: 0x1f1b60,
    index_size: 0x80,
    grid: 8,
    cells: 4,
    row_stride: 0x80,
    code_offset: 0xf800,
    page: None,
    source: Source::Fixed(0x1f3b60),
};
const LOAD_SCROLL2: Blocks = Blocks {
    index_base: 0x1f9ba0,
    index_size: 0x80,
    grid: 8,
    cells: 2,
    row_stride: 0x40,
    code_offset: 0x1800,
    page: Some(0x1000),
    source: Source::Banks { origin: 0x1f9ba0, list: "video/scroll2_banks", mask: 0x0fff_8000, shift: 3, program: false },
};
const LOAD_SMALL: Blocks = Blocks {
    index_base: 0x2459c0,
    index_size: 0x20,
    grid: 4,
    cells: 2,
    row_stride: 0x20,
    code_offset: 0x1f00,
    page: Some(0x800),
    source: Source::Banks { origin: 0x2459c0, list: "video/scroll3_banks", mask: 0xffff_e000, shift: 5, program: true },
};

impl Blocks {
    /// ROM address of the cells of the block whose index is read at `at`.
    fn block_source(&self, b: &mut Cps2Bus, at: u32) -> u32 {
        let index = engine().word(b, at) as u32;
        match self.source {
            Source::Fixed(base) => base.wrapping_add(index << 6),
            Source::Banks { origin, list, mask, shift, program } => {
                let selector = ((at.wrapping_sub(origin) & mask) << shift).rotate_left(16) as u16 as i16;
                let at = engine().get(list).rom.wrapping_add(selector as u32);
                let table = if program { engine().pc_long(b, at) } else { engine().long(b, at) };
                table.wrapping_add(index << 4)
            }
        }
    }

    /// Loads index grid D0 at `dst`. Returns the final address of the destination and the end of the indices.
    fn load(&self, b: &mut Cps2Bus, d0: u16, mut dst: u32) -> (u32, u32) {
        let page_base = self.page.map_or(0, |p| dst & !(p - 1));
        let mut at = self.index_base.wrapping_add(d0 as u32 * self.index_size);
        let row_advance = (self.cells * self.row_stride).wrapping_sub(self.grid * self.cells * 4);
        for _ in 0..self.grid {
            for _ in 0..self.grid {
                let mut src = self.block_source(b, at);
                at += 2;
                for row in 0..self.cells {
                    for col in 0..self.cells {
                        let cell = engine().long(b, src).wrapping_add((self.code_offset as u32) << 16);
                        src += 4;
                        Vram(dst + row * self.row_stride + col * 4).set_long(b, cell);
                    }
                }
                dst = dst.wrapping_add(self.cells * 4);
            }
            dst = dst.wrapping_add(row_advance);
            if let Some(p) = self.page {
                dst = (dst & (p - 1)) + page_base;
            }
        }
        (dst, at)
    }
}

/// The tile block grid loaders.
#[derive(Clone, Copy)]
pub enum Grid {
    /// 0x260c: blocks of 4 x 4 cells.
    Blocks4x4,
    /// 0x26d4: scroll layer 2 (blocks of 2 x 2 cells, pages of 0x1000 bytes).
    Layer2,
    /// 0x2766: scroll layer 3 (small blocks, pages of 0x800 bytes).
    Layer3,
}

impl Grid {
    fn blocks(self) -> &'static Blocks {
        match self {
            Grid::Blocks4x4 => &LOAD_4X4,
            Grid::Layer2 => &LOAD_SCROLL2,
            Grid::Layer3 => &LOAD_SMALL,
        }
    }
}

/// End of a grid load: the destination after the last block (A1) and the index following the last (A2).
#[derive(Clone, Copy, Debug)]
pub struct GridEnd {
    pub destination: u32,
    pub indices: u32,
}

/// Loads block grid `n` of `grid` to `destination` (GFX RAM).
pub fn load_grid(b: &mut Cps2Bus, grid: Grid, n: u16, destination: u32) -> GridEnd {
    let (destination, indices) = grid.blocks().load(b, n, destination);
    GridEnd { destination, indices }
}

// ---------------------------------------------------------------------------------------------
// V2: digits in the character layer
// ---------------------------------------------------------------------------------------------

/// Table of fixed screens (logos and rankings): 10 entries of 12 bytes from the ROM (`video/fixed_screens`), copied
/// into work RAM at startup (0x19038).
const FIXED_SCREENS_RAM: u32 = 0xff111c;

pub fn copy_fixed_screens(b: &mut Cps2Bus) {
    let screens = crate::tables::engine_table!("video/fixed_screens");
    for i in 0..60 {
        let word = screens.nth16(b, i);
        Word(FIXED_SCREENS_RAM + 2 * i).set(b, word);
    }
}

pub const COPY_FIXED_SCREENS: Native = Native {
    entry: 0x19038,
    name: "video_copy_fixed_screens",
    run: crate::Body::Memory(copy_fixed_screens),
    touches_sound: false,
    outputs: 0,
};

/// Native routines of this module.
pub const NATIVES: &[Native] = &[
    COPY_FIXED_SCREENS,
    
    
    
    
    
    
    
    CLEAR_OBJECTS,
    BLANK_SCREEN,
    CLEAR_SCROLL1,
    
    CLEAR_SCROLL2,
    CLEAR_SCROLL3,
    
    
    
    
    
    
    
];
