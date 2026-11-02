//! Video primitives (V1) and tilemaps (V2): layers, CPS-A/B registers, Object RAM, tile blocks.
//!
//! See `docs/video.md` for the formats. The registers `0x804100-0x80411f` (CPS-A) contain the
//! addresses of the tables in GFX RAM (in units of 256 bytes): objects at `0x804100`, layers 1 to 3 at
//! `0x804102/04/06`; `0x804166` (CPS-B) enables the layers.

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::mem::{rd16, rd32, rd8, rts, set_low_byte, wr16, wr32, wr8, A5};
use crate::{regs, Native};

/// Work RAM variables (offsets from A5 = FF8000) specific to the video subsystem.
pub mod ram {
    /// Settings (bit 10: no loading gauge).
    pub const DIP: u32 = 0x7fd0;
    /// Image of the layer enable register (written at `0x804166`).
    pub const LAYER_ENABLE: u32 = 0x14;
    /// Address of the object table (value of `0x804100`).
    pub const OBJ_TABLE: u32 = 0x5ba;
    /// Object RAM bank toggle (byte, inverted at each clearing).
    pub const OBJ_BANK: u32 = 0x5bc;
    /// Coin locks: two images of the output (`+0x10`, `+0x17`) and the forced bits (`+0x18`).
    pub const LOCKOUT: [u32; 2] = [0x10, 0x17];
    pub const LOCKOUT_FORCED: u32 = 0x18;
    /// Non-zero byte: the display registers are no longer written (frozen screen), `A5 - 0x7155`.
    pub const FROZEN: u32 = 0xff0eab;
    /// Current scene.
    pub const STAGE: u32 = 0x34;
    /// Camera 1 (`+0x4fe`) and 2 (`+0x54e`): scroll at `+0x08` (x) and `+0x0c` (y).
    pub const CAMERAS: [u32; 2] = [0x4fe, 0x54e];
    /// Non-zero byte: camera 2 is the current camera.
    pub const USE_CAMERA_2: u32 = 0x36;
    /// Current scroll positions (x, y) of camera 1 and camera 2.
    pub const SCROLL: [u32; 2] = [0x506, 0x50a];
    pub const SCROLL_2: [u32; 2] = [0x556, 0x55a];
}

/// Graphics memory and registers.
pub mod hw {
    pub const TEXT_LAYER: u32 = 0x912000;
    pub const SCROLL1: u32 = 0x910000;
    pub const SCROLL2: u32 = 0x914000;
    pub const SCROLL3: u32 = 0x918000;
    pub const PALETTES: u32 = 0x91c000;
    pub const OBJECTS: u32 = 0x708000;
    pub const CPS_A_OBJECTS: u32 = 0x804100;
    pub const CPS_A_SCROLL: [u32; 3] = [0x804102, 0x804104, 0x804106];
    pub const LAYER_ENABLE: u32 = 0x804166;
    pub const OBJ_BANK: u32 = 0x8040e1;
}

fn set_low_word(r: &mut u32, v: u16) {
    *r = *r & 0xffff_0000 | v as u32;
}

// ---------------------------------------------------------------------------------------------
// V1: loading gauge, locks, layers, clearings
// ---------------------------------------------------------------------------------------------

/// Fill value of the loading gauge: tile 0x20 (space), attribute 3.
const GAUGE: u32 = 0x0020_0003;
/// The gauge occupies the first 32 longs of the character layer (the first column).
const GAUGE_LONGS: u32 = 32;

/// 0xb40: called at each wait (0xa40); sets D1 then draws the gauge (0xb46).
fn gauge_with_value(c: &mut Cpu, b: &mut Cps2Bus) {
    c.d[1] = GAUGE;
    gauge(c, b);
}

/// 0xb46: fills the first column of the character layer with `marker` (the loading gauge), unless
/// the settings hide it (bit 0x400 of DSW: always, in this game). Returns true if the gauge is drawn.
pub fn update_gauge(b: &mut Cps2Bus, marker: u32) -> bool {
    let hidden = rd16(b, A5 + ram::DIP) & 0x400 != 0;
    if !hidden {
        for i in 0..GAUGE_LONGS {
            wr32(b, hw::TEXT_LAYER + 4 * i, marker);
        }
    }
    !hidden
}

/// `update_gauge` (D1 = marker) with the registers of the original: D2 and the flags of the last computation, A0
/// after the last write (the world update leaves them to the next handler).
pub fn gauge_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    let flags_clear = !(m68k::SR_N | m68k::SR_Z | m68k::SR_V | m68k::SR_C);
    if !update_gauge(b, c.d[1]) {
        c.d[2] = c.d[2] & 0xffff_0000 | 0x400;
        c.sr &= flags_clear;
    } else {
        c.d[2] |= 0xffff;
        c.a[0] = hw::TEXT_LAYER + 4 * GAUGE_LONGS;
        c.sr = c.sr & flags_clear
            | if c.d[1] == 0 { m68k::SR_Z } else { 0 }
            | if c.d[1] & 0x8000_0000 != 0 { m68k::SR_N } else { 0 };
    }
}

fn gauge(c: &mut Cpu, b: &mut Cps2Bus) {
    gauge_68k(c, b);
    rts(c, b);
}

/// 0xb96: locks the coin counters of bits `bits` (in addition to the forced bits). Returns the forced bits (D1.b).
pub fn lock_counters(b: &mut Cps2Bus, bits: u8) -> u8 {
    let forced = rd8(b, A5 + ram::LOCKOUT_FORCED);
    for off in ram::LOCKOUT {
        let v = rd8(b, A5 + off) | bits | forced;
        wr8(b, A5 + off, v);
    }
    forced
}

/// 0xbac: unlocks bits `bits` (except the forced bits). Returns the forced bits (D1.b).
pub fn unlock_counters(b: &mut Cps2Bus, bits: u8) -> u8 {
    let forced = rd8(b, A5 + ram::LOCKOUT_FORCED);
    for off in ram::LOCKOUT {
        let v = rd8(b, A5 + off) & !bits | forced;
        wr8(b, A5 + off, v);
    }
    forced
}

fn lock(c: &mut Cpu, b: &mut Cps2Bus) {
    let forced = lock_counters(b, c.d[0] as u8);
    set_low_byte(&mut c.d[1], forced);
    rts(c, b);
}

/// D0 is returned unchanged.
fn unlock(c: &mut Cpu, b: &mut Cps2Bus) {
    let forced = unlock_counters(b, c.d[0] as u8);
    set_low_byte(&mut c.d[1], forced);
    rts(c, b);
}

/// 0xbc6: enables the layers `bits` (added to the register image) and programs the display registers, unless
/// the screen is frozen. No register or flag changes.
pub fn turn_layers_on(b: &mut Cps2Bus, bits: u16) {
    let enable = rd16(b, A5 + ram::LAYER_ENABLE) | bits;
    wr16(b, A5 + ram::LAYER_ENABLE, enable);
    if rd8(b, ram::FROZEN) == 0 {
        wr16(b, hw::LAYER_ENABLE, enable);
        let objects = rd16(b, A5 + ram::OBJ_TABLE);
        wr16(b, hw::CPS_A_OBJECTS, objects);
        for (reg, base) in hw::CPS_A_SCROLL.into_iter().zip([hw::SCROLL1, hw::SCROLL2, hw::SCROLL3]) {
            wr16(b, reg, (base >> 8) as u16);
        }
    }
}

fn enable_layers(c: &mut Cpu, b: &mut Cps2Bus) {
    turn_layers_on(b, c.d[0] as u16);
    rts(c, b);
}

/// 0xbfc: disables the layers `bits`. The processor receives D0.w complemented.
pub fn turn_layers_off(b: &mut Cps2Bus, bits: u16) {
    let enable = rd16(b, A5 + ram::LAYER_ENABLE) & !bits;
    wr16(b, A5 + ram::LAYER_ENABLE, enable);
    wr16(b, hw::LAYER_ENABLE, enable);
}

fn disable_layers(c: &mut Cpu, b: &mut Cps2Bus) {
    let bits = c.d[0] as u16;
    set_low_word(&mut c.d[0], !bits);
    turn_layers_off(b, bits);
    rts(c, b);
}

/// 0xc58: writes the end-of-list entry (0x8000, 0x18000000) at the head of the Object RAM, twice,
/// toggling the bank between the two: the list is empty in both banks.
pub fn clear_objects(b: &mut Cps2Bus) {
    for _ in 0..2 {
        wr32(b, hw::OBJECTS, 0x8000);
        wr32(b, hw::OBJECTS + 4, 0x1800_0000);
        let bank = rd8(b, A5 + ram::OBJ_BANK) ^ 0xff;
        wr8(b, A5 + ram::OBJ_BANK, bank);
        wr8(b, hw::OBJ_BANK, bank);
    }
}


/// Black screen, first half (0xc0c, 0xc32): all layers off (`0xbfc` with D0 = 0x12ce, after
/// resetting `A5+0x14` to zero).
fn switch_layers_off(b: &mut Cps2Bus) {
    wr16(b, A5 + ram::LAYER_ENABLE, 0);
    wr16(b, hw::LAYER_ENABLE, 0);
}

/// Black screen, second half: empty object list and the three layers cleared.
fn clear_screen(b: &mut Cps2Bus) {
    clear_objects(b);
    for layer in [Layer::One, Layer::Two, Layer::Three] {
        clear_layer(b, layer);
    }
}

/// `switch_layers_off`, D0.w = !0x12ce (the mask of 0xbfc).
fn blank_layers(c: &mut Cpu, b: &mut Cps2Bus) {
    switch_layers_off(b);
    set_low_word(&mut c.d[0], !0x12ce);
}

/// `clear_screen`, D0.w = 0x1f00 and D1 = 0 as after `0xce8`.
fn blank_planes(c: &mut Cpu, b: &mut Cps2Bus) {
    clear_screen(b);
    set_low_word(&mut c.d[0], 0x1f00);
    c.d[1] = 0;
}

/// 0xc0c: black screen in one go (layers off, objects and layers cleared). The processor receives D0.w = 0x1f00 and
/// D1 = 0.
pub fn black_screen(b: &mut Cps2Bus) {
    switch_layers_off(b);
    clear_screen(b);
}

fn blank_screen(c: &mut Cpu, b: &mut Cps2Bus) {
    blank_layers(c, b);
    blank_planes(c, b);
    rts(c, b);
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
        super::blank_layers(x.c, x.b);
        one_frame(x, Step::Clear)
    }

    crate::steps! {
        /// Steps of the black screen.
        pub enum Step {
            /// Objects and layers cleared, then one frame.
            Clear = 0xc42 => |x| {
                super::blank_planes(x.c, x.b);
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
        wr16(b, a, tile);
        wr16(b, a + 2, 0);
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

/// 0xca8, 0xcc8, 0xce8: fills the layer with its empty tile. The processor receives D0.w = the tile and D1 = 0 (A0
/// and A1 are restored).
pub fn clear_layer(b: &mut Cps2Bus, layer: Layer) {
    let (from, to, tile) = layer.zone();
    fill_layer(b, from, to, tile);
}

/// Common body of the three fills: D0 = tile, D1 = 0.
fn fill_run(c: &mut Cpu, b: &mut Cps2Bus, layer: Layer) {
    clear_layer(b, layer);
    set_low_word(&mut c.d[0], layer.zone().2);
    c.d[1] = 0;
    rts(c, b);
}

fn clear_scroll1(c: &mut Cpu, b: &mut Cps2Bus) {
    fill_run(c, b, Layer::One);
}
/// Bytes cleared by 0x3525c.
const SCROLL1_START: u32 = 0x2000;

/// 0x3525c: clears the first 8 KB of layer 1 with the empty tile 0x1f.
pub fn clear_layer1_start(b: &mut Cps2Bus) {
    fill_layer(b, hw::SCROLL1, hw::SCROLL1 + SCROLL1_START, 0x1f);
}

/// `clear_layer1_start` with the registers of the original: the cell in D0-D5, A2 and A3, D6 = -1 and A1 at the end
/// of the zone.
pub fn clear_layer1_start_68k(c: &mut Cpu, b: &mut Cps2Bus) {
    clear_layer1_start(b);
    leftovers_clear_layer1_start(c);
}

/// The registers that 0x3525c leaves.
pub fn leftovers_clear_layer1_start(c: &mut Cpu) {
    const SIZE: u32 = SCROLL1_START;
    for n in 0..6 {
        c.d[n] = 0x1f_0000;
    }
    c.a[2] = 0x1f_0000;
    c.a[3] = 0x1f_0000;
    c.a[1] = hw::SCROLL1 + SIZE;
    set_low_word(&mut c.d[6], 0xffff);
    c.sr &= !(m68k::SR_N | m68k::SR_Z | m68k::SR_V | m68k::SR_C);
}

fn clear_scroll1_start(c: &mut Cpu, b: &mut Cps2Bus) {
    clear_layer1_start_68k(c, b);
    rts(c, b);
}
fn clear_scroll2(c: &mut Cpu, b: &mut Cps2Bus) {
    fill_run(c, b, Layer::Two);
}
fn clear_scroll3(c: &mut Cpu, b: &mut Cps2Bus) {
    fill_run(c, b, Layer::Three);
}

const fn fixed(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus), outputs: u32) -> Native {
    Native { entry, name, run: crate::Body::WithCpu(run), touches_sound: false, outputs }
}

pub const GAUGE_NATIVE: Native = fixed(0xb46, "video_gauge", gauge, regs::d(2) | regs::a(0) | regs::CCR);
/// 0xb3e: `bra.b 0xb46`, called by the world update (D1 = class marker).
pub const GAUGE_THUNK_NATIVE: Native = fixed(0xb3e, "video_gauge_b3e", gauge, regs::d(2) | regs::a(0) | regs::CCR);
pub const GAUGE_VALUE_NATIVE: Native = fixed(0xb40, "video_gauge_d1", gauge_with_value, regs::d(1) | regs::d(2) | regs::a(0) | regs::CCR);
pub const LOCK: Native = fixed(0xb96, "video_lock", lock, regs::d(1));
pub const UNLOCK: Native = fixed(0xbac, "video_deverrou", unlock, regs::d(0) | regs::d(1));
pub const ENABLE_LAYERS: Native = fixed(0xbc6, "video_layers_on", enable_layers, 0);
pub const DISABLE_LAYERS: Native = fixed(0xbfc, "video_layers_off", disable_layers, regs::d(0));
pub const BLANK_SCREEN: Native = Native {
    entry: 0xc0c,
    name: "video_black_screen",
    run: crate::Body::WithCpu(blank_screen),
    touches_sound: false,
    outputs: regs::d(0) | regs::d(1),
};
pub const CLEAR_OBJECTS: Native =
    Native { entry: 0xc58, name: "video_clear_objects", run: crate::Body::Memory(clear_objects), touches_sound: false, outputs: 0 };
pub const CLEAR_SCROLL1: Native = fixed(0xca8, "video_clear_scroll1", clear_scroll1, regs::d(0) | regs::d(1));
pub const CLEAR_SCROLL1_START: Native = fixed(0x3525c, "video_clear_start_scroll1", clear_scroll1_start, regs::ALL);
pub const CLEAR_SCROLL2: Native = fixed(0xcc8, "video_clear_scroll2", clear_scroll2, regs::d(0) | regs::d(1));
pub const CLEAR_SCROLL3: Native = fixed(0xce8, "video_clear_scroll3", clear_scroll3, regs::d(0) | regs::d(1));


// ---------------------------------------------------------------------------------------------
// V2: terrain attribute under a point
// ---------------------------------------------------------------------------------------------

/// Fixed terrain maps of scenes 2 and 0x14 (words in ROM, columns of 0x20 bytes).
const TERRAIN_MAP_STAGE_2: u32 = 0x3f8e9c;
const TERRAIN_MAP_STAGE_14: u32 = 0x3f9c9c;

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
    let stage = rd8(b, crate::mem::A5 + ram::STAGE);
    let (mut x, mut y) = (x, y);
    let fixed_map = if stage == 2 {
        Some((TERRAIN_MAP_STAGE_2, 0x10u16.wrapping_neg()))
    } else if stage == 0x14 {
        Some((TERRAIN_MAP_STAGE_14, 0x10))
    } else {
        None
    };
    let (word, case, index, column) = if let Some((map, dy)) = fixed_map {
        y = y.wrapping_add(dy);
        let row = 0x1e - ((y as u8 >> 3) & 0x1e) as u16;
        let column = x.wrapping_add(x) & 0xffe0;
        let index = row.wrapping_add(column);
        let case = map.wrapping_add(index as i16 as u32);
        (rd16(b, case), case, index, column)
    } else {
        let camera_2 = rd8(b, crate::mem::A5 + ram::USE_CAMERA_2) != 0;
        if camera_2 {
            x = x.wrapping_add(rd16(b, crate::mem::A5 + ram::SCROLL[0]).wrapping_sub(rd16(b, crate::mem::A5 + ram::SCROLL_2[0])));
            y = y.wrapping_add(rd16(b, crate::mem::A5 + ram::SCROLL[1]).wrapping_sub(rd16(b, crate::mem::A5 + ram::SCROLL_2[1])));
        }
        let page = 0x3000u16.wrapping_sub((y & 0x300) << 4);
        let row = 0x3c - ((y as u8 >> 2) & 0x3c) as u16;
        let column = x.wrapping_shl(2) & 0xfc0;
        let index = page.wrapping_add(row).wrapping_add(column);
        let case = hw::SCROLL2.wrapping_add(index as i16 as u32);
        (rd16(b, case + 2), case, index, column)
    };
    Terrain { number: (word & 0xff80) >> 7, x, y, case, index, column, fixed_map: fixed_map.is_some() }
}

/// Terrain number under object `o` (0x24a8: x position at +8, y at +0x10).
pub fn terrain_of(b: &mut Cps2Bus, o: crate::objects::Obj) -> Terrain {
    let (x, y) = (rd16(b, o.0 + 8), rd16(b, o.0 + 0x10));
    terrain(b, x, y)
}

/// Registers left by 0x24b0: D0/D1 (corrected coordinates), D2 (number), D3/D4 (computation leftovers, read by
/// callers that take them as arguments), A1 (cell) and Z (zero number).
pub fn terrain_68k(c: &mut Cpu, t: &Terrain) {
    if t.fixed_map {
        c.d[3] = t.index as u32;
    } else {
        set_low_word(&mut c.d[3], t.index);
        c.d[2] = 0;
    }
    set_low_word(&mut c.d[4], t.column);
    c.a[1] = t.case;
    set_low_word(&mut c.d[0], t.x);
    set_low_word(&mut c.d[1], t.y);
    set_low_word(&mut c.d[2], t.number);
    c.sr = c.sr & !crate::ops::ALL | if t.number == 0 { m68k::SR_Z } else { 0 };
}

/// 0x24b0: terrain number at (D0, D1).
fn terrain_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let t = terrain(b, c.d[0] as u16, c.d[1] as u16);
    terrain_68k(c, &t);
    rts(c, b);
}

/// 0x24a8: terrain number under object A0 (x position at +8, y at +0x10).
fn terrain_of_object(c: &mut Cpu, b: &mut Cps2Bus) {
    set_low_word(&mut c.d[0], rd16(b, c.a[0] + 8));
    set_low_word(&mut c.d[1], rd16(b, c.a[0] + 0x10));
    terrain_native(c, b);
}

const TERRAIN_OUT: u32 = regs::d(0) | regs::d(1) | regs::d(2) | regs::d(3) | regs::d(4) | regs::a(1) | regs::CCR;

pub const TERRAIN: Native = fixed(0x24b0, "video_terrain", terrain_native, TERRAIN_OUT);
pub const TERRAIN_OF_OBJECT: Native = fixed(0x24a8, "video_terrain_object", terrain_of_object, TERRAIN_OUT);

// ---------------------------------------------------------------------------------------------
// V2: tile block loaders
// ---------------------------------------------------------------------------------------------

/// Geometry of a loader: D0 chooses a grid of `grid` x `grid` block indices (words in ROM);
/// each block is `cells` x `cells` cells, each cell a long (tile, attribute) copied from the ROM
/// with `code_offset` added to the tile. A1 is the destination in GFX RAM.
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
    /// word; address of the table at `list + selector` (read in program space if `program`, PC-relative
    /// access, otherwise in data space, access through an address register).
    Banks { origin: u32, list: u32, mask: u32, shift: u32, program: bool },
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
    source: Source::Banks { origin: 0x1f9ba0, list: 0x275a, mask: 0x0fff_8000, shift: 3, program: false },
};
const LOAD_SMALL: Blocks = Blocks {
    index_base: 0x2459c0,
    index_size: 0x20,
    grid: 4,
    cells: 2,
    row_stride: 0x20,
    code_offset: 0x1f00,
    page: Some(0x800),
    source: Source::Banks { origin: 0x2459c0, list: 0x27e6, mask: 0xffff_e000, shift: 5, program: true },
};

fn prog_long(b: &mut Cps2Bus, a: u32) -> u32 {
    (crate::mem::prog16(b, a) as u32) << 16 | crate::mem::prog16(b, a + 2) as u32
}

impl Blocks {
    /// ROM address of the cells of the block whose index is read at `at`.
    fn block_source(&self, b: &mut Cps2Bus, at: u32) -> u32 {
        let index = rd16(b, at) as u32;
        match self.source {
            Source::Fixed(base) => base.wrapping_add(index << 6),
            Source::Banks { origin, list, mask, shift, program } => {
                let selector = ((at.wrapping_sub(origin) & mask) << shift).rotate_left(16) as u16 as i16;
                let at = list.wrapping_add(selector as u32);
                let table = if program { prog_long(b, at) } else { rd32(b, at) };
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
                        let cell = rd32(b, src).wrapping_add((self.code_offset as u32) << 16);
                        src += 4;
                        wr32(b, dst + row * self.row_stride + col * 4, cell);
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

/// `load_grid` (D0 = number, A1 = destination) with the registers of the original: D0 = 0xffff, A1 and A2 at the end
/// of the grid.
pub fn load_grid_68k(c: &mut Cpu, b: &mut Cps2Bus, grid: Grid) {
    let end = load_grid(b, grid, c.d[0] as u16, c.a[1]);
    c.d[0] = 0xffff;
    c.a[1] = end.destination;
    c.a[2] = end.indices;
}

fn load_blocks(c: &mut Cpu, b: &mut Cps2Bus, grid: Grid) {
    load_grid_68k(c, b, grid);
    rts(c, b);
}

const LOAD_OUT: u32 = regs::d(0) | regs::a(1) | regs::a(2);

pub const LOAD_BLOCKS_4X4: Native = fixed(0x260c, "video_blocks_4x4", |c, b| load_blocks(c, b, Grid::Blocks4x4), LOAD_OUT);
pub const LOAD_BLOCKS_SCROLL2: Native = fixed(0x26d4, "video_blocks_scroll2", |c, b| load_blocks(c, b, Grid::Layer2), LOAD_OUT);
pub const LOAD_BLOCKS_SMALL: Native = fixed(0x2766, "video_blocks_small", |c, b| load_blocks(c, b, Grid::Layer3), LOAD_OUT);

// ---------------------------------------------------------------------------------------------
// V2: digits in the character layer
// ---------------------------------------------------------------------------------------------

/// Writes a digit (tile number = value, attribute 0) at `at`; the next cell of the row is
/// `COLUMN` further in the character layer.
fn put_digit(b: &mut Cps2Bus, at: u32, digit: u16) {
    wr16(b, at, digit);
    wr16(b, at + 2, 0);
}

/// Next column of the character layer (see `text.rs`).
const COLUMN: u32 = 0x80;

/// 0x255a: D0 (0 to 99, saturated) as two decimal digits: tens in A1, units one column further.
/// Returns D0 = units, D1 = tens, A1 = units cell + 2.
fn decimal_digits(c: &mut Cpu, b: &mut Cps2Bus) {
    let mut value = c.d[0] as u16;
    if value >= 100 {
        value = 99;
    }
    let (tens, units) = (value / 10, value % 10);
    put_digit(b, c.a[1], tens);
    put_digit(b, c.a[1] + COLUMN, units);
    c.d[1] = tens as u32;
    set_low_word(&mut c.d[0], units);
    c.a[1] += COLUMN + 2;
    crate::ops::logic(c, 0, m68k::decode::Size::Word);
    rts(c, b);
}

/// 0x25e8: D0 as four hexadecimal digits (high nibble first), one per column starting at A1.
/// Returns A1 = A1 + 4 columns.
fn hex_digits(c: &mut Cpu, b: &mut Cps2Bus) {
    for i in 0..4 {
        put_digit(b, c.a[1] + COLUMN * i, (c.d[0] as u16) >> (12 - 4 * i) & 0xf);
    }
    c.a[1] += 4 * COLUMN;
    crate::ops::logic(c, 0, m68k::decode::Size::Word);
    rts(c, b);
}

pub const DECIMAL_DIGITS: Native = fixed(0x255a, "video_digits_decimal", decimal_digits, regs::d(0) | regs::d(1) | regs::a(1) | regs::CCR);
pub const HEX_DIGITS: Native = fixed(
    0x25e8,
    "video_digits_hex",
    hex_digits,
    regs::a(1) | regs::CCR,
);

/// Table of fixed screens (logos and rankings): 10 entries of 12 bytes from the ROM, copied into work
/// RAM at startup (0x19038).
const FIXED_SCREENS_ROM: u32 = 0x19a1a;
const FIXED_SCREENS_RAM: u32 = 0xff111c;
const FIXED_SCREENS_LEN: u32 = 120;

pub fn copy_fixed_screens(b: &mut Cps2Bus) {
    for offset in (0..FIXED_SCREENS_LEN).step_by(2) {
        let word = rd16(b, FIXED_SCREENS_ROM + offset);
        wr16(b, FIXED_SCREENS_RAM + offset, word);
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
    GAUGE_NATIVE,
    GAUGE_THUNK_NATIVE,
    GAUGE_VALUE_NATIVE,
    LOCK,
    UNLOCK,
    ENABLE_LAYERS,
    DISABLE_LAYERS,
    CLEAR_OBJECTS,
    BLANK_SCREEN,
    CLEAR_SCROLL1,
    CLEAR_SCROLL1_START,
    CLEAR_SCROLL2,
    CLEAR_SCROLL3,
    TERRAIN,
    TERRAIN_OF_OBJECT,
    LOAD_BLOCKS_4X4,
    LOAD_BLOCKS_SCROLL2,
    LOAD_BLOCKS_SMALL,
    DECIMAL_DIGITS,
    HEX_DIGITS,
];
