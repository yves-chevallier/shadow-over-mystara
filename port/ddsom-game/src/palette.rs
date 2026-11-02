//! Palettes in GFX RAM: brightness, loading from the ROM library, fades.
//!
//! A hardware colour is 0xLRGB (L = brightness, one nibble per component). The four palette pages
//! (`0x91c000 + 0x400 k`, 32 palettes of 16 colours) are designated by bits 0 to 3 of
//! `PAGES`. See `docs/video.md`.

use cps2::Cps2Bus;

use crate::mem::{rd16, rd32, rd8, wr16, wr32};
use crate::video::hw;
use crate::Native;

/// Palette pages concerned (bit k: page 0x91c000 + 0x400 k).
const PAGES: u32 = 0x48e;
pub(crate) const PALETTES: u32 = 0x91c000;
pub(crate) const PAGE_SIZE: u32 = 0x400;

fn for_each_color(b: &mut Cps2Bus, mut f: impl FnMut(u16) -> u16) {
    let pages = rd8(b, crate::mem::A5 + PAGES);
    for k in 0..4 {
        if pages & (1 << k) != 0 {
            let page = PALETTES + PAGE_SIZE * k;
            for i in 0..PAGE_SIZE / 2 {
                let a = page + 2 * i;
                let v = f(rd16(b, a));
                wr16(b, a, v);
            }
        }
    }
}

/// 0x168e: brightness `level` added (OR) to the colours of the checked pages (`A5+0x48e`), or cleared if `level`
/// is zero.
pub fn or_brightness(b: &mut Cps2Bus, level: u16) {
    for_each_color(b, |col| if level == 0 { col & 0x0fff } else { col | level });
}

/// 0x1686: maximum brightness.
pub fn full_brightness(b: &mut Cps2Bus) {
    or_brightness(b, 0xf000);
}

/// 0x168c: zero brightness.
pub fn no_brightness(b: &mut Cps2Bus) {
    or_brightness(b, 0);
}

/// 0x173c: brightness of the colours of the checked pages replaced by `level`.
pub fn set_brightness(b: &mut Cps2Bus, level: u16) {
    for_each_color(b, |col| (col & 0x0fff) | level);
}

// ---------------------------------------------------------------------------------------------
// Brightness fades (V3): one step, repeated by the tasks and subroutines of `tasks::fading`
// ---------------------------------------------------------------------------------------------

/// State of a fade (A5 + 0x48e): byte of the pages concerned (bit k: page k), number of steps remaining,
/// delay between two steps (word) and addresses of the four pages to process.
mod fade {
    pub const PAGES: u32 = 0;
    pub const PAGE_PTR: u32 = 4;
}

/// Number of colours in a page.
const COLORS: u32 = 512;
/// Colours of the first page processed by the partial fade (0x15c8): the last 48 palettes
/// `0x91c3a0` to `0x91c400`.
const PARTIAL_WORDS: u32 = 48;
const BRIGHTNESS: u16 = 0xf000;

/// 0x1602: adds the step (±0x1000) to the brightness of the page's 512 colours, unless the first
/// colour is already at the end (0 going down, 0xf000 going up) or it would reach 0.
fn fade_page(b: &mut Cps2Bus, page: u32, delta: u32) {
    let first = rd16(b, page) & BRIGHTNESS;
    let at_end = if (delta as i32) < 0 { first == 0 } else { first == BRIGHTNESS };
    if at_end || first.wrapping_add(delta as u16) & BRIGHTNESS == 0 {
        return;
    }
    add_to_colors(b, page, COLORS, delta as u16);
}

/// 0x164e: like `fade_page` on the 48 colours at the end of the first page, without a stop test; going
/// down, the brightness of the first colour is tested before the step.
fn fade_partial_page(b: &mut Cps2Bus, page: u32, delta: u32) {
    let mut first = rd16(b, page);
    if (delta as i32) >= 0 {
        first = first.wrapping_add(delta as u16);
    }
    if first & BRIGHTNESS == 0 {
        return;
    }
    add_to_colors(b, page, PARTIAL_WORDS, delta as u16);
}

/// Adds `delta` (modulo 2^16) to the `n` colours starting at `page`.
fn add_to_colors(b: &mut Cps2Bus, page: u32, n: u32, delta: u16) {
    for i in 0..n {
        let a = page + 2 * i;
        let color = rd16(b, a).wrapping_add(delta);
        wr16(b, a, color);
    }
}

/// Pages checked in the fade state, with their address.
fn selected_pages(b: &mut Cps2Bus, state: u32) -> Vec<(usize, u32)> {
    let flags = rd8(b, state + fade::PAGES);
    (0..4).filter(|k| flags >> k & 1 != 0).map(|k| (k, rd32(b, state + fade::PAGE_PTR + 4 * k as u32))).collect()
}

/// One brightness fade step (`0x158e`, or `0x15c8` if `partial`) on the pages of the fade state
/// (`A5 + 0x48e`), with step `delta` (±0x1000).
pub fn brightness_step(b: &mut Cps2Bus, delta: u32, partial: bool) {
    for (k, page) in selected_pages(b, crate::mem::A5 + PAGES) {
        if partial && k == 0 {
            fade_partial_page(b, page, delta);
        } else {
            fade_page(b, page, delta);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Loading the scene palettes from the ROM library (V3)
// ---------------------------------------------------------------------------------------------

/// Palette library (0x253000-0x293000): palettes of 16 colours, 32 bytes each.
const LIBRARY: u32 = 0x253000;
/// Per scene: 32 displacements (words) in the library, a negative displacement leaves the palette.
const STAGE_PALETTES: u32 = 0x2ae81e;
/// Palettes added after those of the scene: address (long, 0 = none) per scene, in code.
pub(crate) const STAGE_EXTRA: u32 = 0x1a41c;
pub(crate) const EXTRA_PALETTES: u32 = 3;
/// Scenes whose 32 palettes are loaded (the others: 29).
const FULL_RANGE: std::ops::Range<u8> = 0x45..0x5a;
pub(crate) const STAGE_VAR: u32 = 0x34;
pub(crate) const PALETTE_BYTES: u32 = 32;

fn copy_palette(b: &mut Cps2Bus, from: u32, to: u32) {
    for i in 0..PALETTE_BYTES / 4 {
        let long = rd32(b, from + 4 * i);
        wr32(b, to + 4 * i, long);
    }
}

fn prog_long(b: &mut Cps2Bus, a: u32) -> u32 {
    (crate::mem::prog16(b, a) as u32) << 16 | crate::mem::prog16(b, a + 2) as u32
}

/// Address of a code table indexed by `n * 4` (16-bit computation, signed word). The tables read through
/// a PC-relative access (pages 1 to 3) are in the program space; those whose address goes through
/// a register (`lea (d,PC),A3` then indexing) in the data space.
fn table_entry(b: &mut Cps2Bus, table: u32, n: u32, program: bool) -> u32 {
    let at = table.wrapping_add((n as u16).wrapping_shl(2) as i16 as u32);
    if program {
        prog_long(b, at)
    } else {
        rd32(b, at)
    }
}

/// Number of palettes in the list of scene `stage`: 32 for the scenes of `FULL_RANGE`, 29 for the others (the lower
/// bound is compared signed, the upper one unsigned, like the original).
pub(crate) fn stage_count(stage: u8) -> u32 {
    if (stage as i8) < FULL_RANGE.start as i8 || stage >= FULL_RANGE.end {
        29
    } else {
        32
    }
}

/// Displacement list (words, 64 bytes per scene) of the scene in the low word of `d0`.
pub(crate) fn stage_list(d0: u32) -> u32 {
    STAGE_PALETTES + ((d0 as u16) << 6) as i16 as u32
}

/// Address of the extra palettes of the current scene (`A5 + 0x34`), 0 if it has none.
pub(crate) fn stage_extra(b: &mut Cps2Bus, a5: u32) -> u32 {
    let current = rd8(b, a5 + STAGE_VAR) as u32;
    table_entry(b, STAGE_EXTRA, current, false)
}

/// 0x1a36c: palettes of scene `d0` (low byte) starting at `dst`, then the extra palettes
/// of the current scene (`A5 + 0x34`).
pub(crate) fn load_stage_palettes(b: &mut Cps2Bus, a5: u32, d0: u32, mut dst: u32) {
    let mut list = stage_list(d0);
    for _ in 0..stage_count(d0 as u8) {
        let offset = rd16(b, list);
        list += 2;
        if offset & 0x8000 == 0 {
            copy_palette(b, LIBRARY + offset as i16 as u32, dst);
        }
        dst += PALETTE_BYTES;
    }
    let extra = stage_extra(b, a5);
    if extra != 0 {
        for i in 0..EXTRA_PALETTES {
            copy_palette(b, extra + i * PALETTE_BYTES, dst);
            dst += PALETTE_BYTES;
        }
    }
}

/// The eight longs of a palette, as `movem.l (A),{D1-D6,A1,A2}` loads them.
pub(crate) fn palette_longs(b: &mut Cps2Bus, at: u32) -> [u32; 8] {
    std::array::from_fn(|i| rd32(b, at + 4 * i as u32))
}

/// 0x1a360: palettes of the current scene in page 0.
pub fn load_scene_palettes(b: &mut Cps2Bus) {
    let stage = rd8(b, crate::mem::A5 + STAGE_VAR) as u32;
    load_stage_palettes(b, crate::mem::A5, stage, PALETTES);
}

/// 0x1a366: palettes of scene `scene` in page 0 (then the extras of the current scene).
pub fn load_scene_palettes_of(b: &mut Cps2Bus, scene: u8) {
    load_stage_palettes(b, crate::mem::A5, scene as u32, PALETTES);
}

/// 0x1a3d4: the 32 palettes of set D0 (address in code at `0x1a3fc + 4 D0`) in page 0.
pub(crate) const SETS: u32 = 0x1a3fc;
pub(crate) const PAGE_PALETTES: u32 = 32;

/// Returns the source address (0: set absent).
pub(crate) fn load_set(b: &mut Cps2Bus, d0: u32) -> u32 {
    let source = table_entry(b, SETS, d0, false);
    if !(source == 0) {
        for i in 0..PAGE_PALETTES {
            copy_palette(b, source + i * PALETTE_BYTES, PALETTES + i * PALETTE_BYTES);
        }
    }
    source
}

/// 0x1a3d4: the 32 palettes of set `n` in page 0 (nothing if the set is absent).
pub fn load_palette_set(b: &mut Cps2Bus, n: u16) {
    load_set(b, n as u32);
}

/// Pages 1 to 3: code table (address per scene), "current scene" entry, "given D0" entry.
pub(crate) const PAGE_TABLES: [(u32, u32, u32); 3] = [(0x1a62c, 0x1a608, 0x1a60a), (0x1a6a0, 0x1a678, 0x1a67e), (0x1a8d4, 0x1a8ac, 0x1a8b2)];

/// Source of page `k + 1`: entry `d0` of the page table.
pub(crate) fn page_source(b: &mut Cps2Bus, k: usize, d0: u32) -> u32 {
    table_entry(b, PAGE_TABLES[k].0, d0, true)
}

/// The 32 palettes of page `k + 1` according to the page table, entry `d0`.
pub(crate) fn load_page(b: &mut Cps2Bus, k: usize, d0: u32) -> u32 {
    let source = page_source(b, k, d0);
    let page = PALETTES + PAGE_SIZE * (k as u32 + 1);
    for i in 0..PAGE_PALETTES {
        copy_palette(b, source + i * PALETTE_BYTES, page + i * PALETTE_BYTES);
    }
    source
}

/// Palette pages 1 to 3 (0x1a608/0x1a60a, 0x1a678/0x1a67e, 0x1a8ac/0x1a8b2): the 32 palettes of entry `n`
/// of the page table.
pub fn load_palette_page(b: &mut Cps2Bus, page: usize, n: u16) {
    assert!((1..=3).contains(&page), "palette page {page}");
    load_page(b, page - 1, n as u32);
}

/// 0x1a678, 0x1a8ac: page 2 or 3 according to the current scene (0x1a608: page 1 always takes entry 0).
pub fn load_scene_page(b: &mut Cps2Bus, page: usize) {
    let n = if page == 1 { 0 } else { rd8(b, crate::mem::A5 + STAGE_VAR) as u16 };
    load_palette_page(b, page, n);
}

/// The D2-D5 that a page loader (0x1a608...) leaves: longs 2 to 5 of the page's last palette (`page`: 1 to 3).
pub fn page_leftovers(b: &mut Cps2Bus, page: usize) -> [u32; 4] {
    let l = palette_longs(b, PALETTES + PAGE_SIZE * page as u32 + (PAGE_PALETTES - 1) * PALETTE_BYTES);
    [l[2], l[3], l[4], l[5]]
}

/// 0x1a296: all the palettes of the current scene (pages 0 to 3), sets the word `0x91d7fe` to zero;
/// for scenes 0x2f and 0x42 page 0 is loaded into scroll layer 1 and the four
/// fade sources (`A5 + 0x492`) are initialised.
const FADE_SOURCES: u32 = 0x492;
const CLEARED_WORD: u32 = 0x91d7fe;

/// Fade sources of scenes 0x2f and 0x42: (layer 1, work buffer, two palettes in ROM).
fn special_sources(stage: u8) -> Option<[u32; 4]> {
    match stage {
        0x2f => Some([hw::SCROLL1, 0xff1190, 0x275360, 0x285680]),
        0x42 => Some([hw::SCROLL1, 0xff1190, 0x277780, 0x287ae0]),
        _ => None,
    }
}

/// 0x1a296: all the palettes of the current scene. Returns the D0 that the original leaves: the first long of the
/// last palette of page 3 (the configuration menu rereads it).
pub fn load_all_palettes(b: &mut Cps2Bus) -> u32 {
    let a5 = crate::mem::A5;
    let stage = rd8(b, a5 + STAGE_VAR);
    let special = special_sources(stage);
    let dst = if special.is_some() { hw::SCROLL1 } else { PALETTES };
    load_stage_palettes(b, a5, stage as u32, dst);
    for (k, d0) in [(0, 0), (1, stage as u32), (2, stage as u32)] {
        load_page(b, k, d0);
    }
    wr16(b, CLEARED_WORD, 0);
    if let Some(sources) = special {
        for (k, source) in sources.into_iter().enumerate() {
            wr32(b, a5 + FADE_SOURCES + 4 * k as u32, source);
        }
    }
    rd32(b, PALETTES + 3 * PAGE_SIZE + (PAGE_PALETTES - 1) * PALETTE_BYTES)
}

// ---------------------------------------------------------------------------------------------
// Per-component fades (V3): each colour approaches a target by one step per component
// ---------------------------------------------------------------------------------------------

/// Fade sources (`A5 + 0x492`, 4 long words: a full palette of 512 colours per page
/// for 0x1cd7c, one colour (word) per page for 0x1cdd6).
pub(crate) const SOURCES: u32 = 0x492;
const PAGES_FLAG: u32 = 0x48e;

/// Advances each RGB component (nibbles 0 to 2) of `color` by one step towards `target`. The
/// brightness (nibble 3) is preserved.
fn approach(color: u16, target: u16) -> u16 {
    let mut out = color;
    for n in 0..3 {
        let shift = 4 * n as u32;
        let (cur, goal) = (color >> shift & 0xf, target >> shift & 0xf);
        let next = match goal.cmp(&cur) {
            std::cmp::Ordering::Equal => cur,
            std::cmp::Ordering::Greater => cur + 1,
            std::cmp::Ordering::Less => cur - 1,
        };
        out = out & !(0xf << shift) | next << shift;
    }
    out
}

/// Advances each RGB component by one step towards 0xf (white).
fn brighten(color: u16) -> u16 {
    let mut out = color;
    for n in 0..3 {
        let shift = 4 * n as u32;
        if color >> shift & 0xf != 0xf {
            out += 1 << shift;
        }
    }
    out
}

fn colors_of(b: &mut Cps2Bus, page: u32, f: impl Fn(&mut Cps2Bus, u32, u32) -> u16) -> Vec<u16> {
    (0..COLORS).map(|i| f(b, page, i)).collect()
}

fn rewrite(b: &mut Cps2Bus, page: u32, results: &[u16]) {
    for (i, color) in results.iter().enumerate() {
        wr16(b, page + 2 * i as u32, *color);
    }
}

/// 0x1cfc8: approaches the 512 colours of page A0 to the target colours at A1.
fn approach_page_results(b: &mut Cps2Bus, page: u32, target: u32) -> Vec<u16> {
    colors_of(b, page, |b, page, i| approach(rd16(b, page + 2 * i), rd16(b, target + 2 * i)))
}

pub(crate) fn approach_page(b: &mut Cps2Bus, page: u32, target: u32) {
    let results = approach_page_results(b, page, target);
    rewrite(b, page, &results);
}

/// 0x1d042: approaches the 512 colours of page A0 to the single colour D1.
fn approach_constant_results(b: &mut Cps2Bus, page: u32, goal: u16) -> Vec<u16> {
    colors_of(b, page, |b, page, i| approach(rd16(b, page + 2 * i), goal))
}

pub(crate) fn approach_constant(b: &mut Cps2Bus, page: u32, goal: u16) {
    let results = approach_constant_results(b, page, goal);
    rewrite(b, page, &results);
}

/// 0x1d00e: the 512 colours of page A0 gain one step towards white.
pub(crate) fn brighten_page(b: &mut Cps2Bus, page: u32) {
    let results = colors_of(b, page, |b, page, i| brighten(rd16(b, page + 2 * i)));
    rewrite(b, page, &results);
}

/// Checked pages (A5 + 0x48e) with their GFX RAM address.
pub(crate) fn flagged_pages(b: &mut Cps2Bus, a5: u32) -> Vec<(usize, u32)> {
    let flags = rd8(b, a5 + PAGES_FLAG);
    (0..4).filter(|k| flags >> k & 1 != 0).map(|k| (k, PALETTES + PAGE_SIZE * k as u32)).collect()
}

/// Target of a per-component fade.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    /// White (`0x1cd32`).
    White,
    /// One colour per page, the word `A5 + 0x492 + 4 k` (`0x1cdd6`).
    Colours,
    /// A full palette per page, at address `A5 + 0x492 + 4 k` (`0x1cd7c`).
    Palettes,
}

/// One per-component fade step of the checked pages (`A5 + 0x48e`) towards `target`.
pub fn component_step(b: &mut Cps2Bus, target: Target) {
    for (k, page) in flagged_pages(b, crate::mem::A5) {
        let source = crate::mem::A5 + SOURCES + 4 * k as u32;
        match target {
            Target::White => brighten_page(b, page),
            Target::Colours => {
                let goal = rd16(b, source);
                approach_constant(b, page, goal);
            }
            Target::Palettes => {
                let target = rd32(b, source);
                approach_page(b, page, target);
            }
        }
    }
}

/// `0x1cf3c`: the checked pages all white.
pub fn white_out(b: &mut Cps2Bus) {
    for (_, page) in flagged_pages(b, crate::mem::A5) {
        fill_page(b, page, [u32::MAX; 8]);
    }
}

/// Copies 8 longs (D0 to D6 and A1) over the whole page (32 palettes): 0x1cfb8.
pub(crate) fn fill_page(b: &mut Cps2Bus, page: u32, values: [u32; 8]) {
    for row in 0..32 {
        for (i, value) in values.iter().enumerate() {
            wr32(b, page + 32 * row + 4 * i as u32, *value);
        }
    }
}

pub const WHITE_PAGES: Native =
    Native { entry: 0x1cf3c, name: "palette_white_pages", run: crate::Body::Memory(white_out), touches_sound: false, outputs: 0 };

// ---------------------------------------------------------------------------------------------
// Lightning: greyed palettes (U2)
// ---------------------------------------------------------------------------------------------

/// Palettes set aside by the lightning: a page of 0x400 colours and 48 colours of page 0.
pub(crate) mod flash {
    pub const PAGE: u32 = 0x91c800;
    pub const PAGE_COLORS: u32 = 0x400;
    pub const SPARE: u32 = 0x91c3a0;
    pub const SPARE_COLORS: u32 = 0x30;
    /// Backup copy (work RAM): the page, then the 48 colours.
    pub const BACKUP: u32 = 0xff1190;
    pub const BACKUP_SPARE: u32 = BACKUP + 2 * PAGE_COLORS;
}

fn copy_words(b: &mut Cps2Bus, from: u32, to: u32, words: u32) {
    for i in 0..words {
        let v = rd16(b, from + 2 * i);
        wr16(b, to + 2 * i, v);
    }
}

/// Sets the lightning palettes aside (0x1ac24) or puts them back (0x1abe0).
fn copy_flash_palettes(b: &mut Cps2Bus, restore: bool) {
    for (live, backup, colors) in
        [(flash::PAGE, flash::BACKUP, flash::PAGE_COLORS), (flash::SPARE, flash::BACKUP_SPARE, flash::SPARE_COLORS)]
    {
        if restore {
            copy_words(b, backup, live, colors);
        } else {
            copy_words(b, live, backup, colors);
        }
    }
}

/// Grey level of a colour: the rounded average of its three components (0 to 12).
pub(crate) fn flash_level(color: u16) -> u16 {
    ((color >> 8 & 0xf) + (color >> 4 & 0xf) + (color & 0xf) + 4) >> 2
}

/// Colour of a level: `base` + level at each nibble, then mask. The offset of `base` makes the brightness
/// nibble overflow (see the eight variants below).
pub(crate) fn flash_color(level: u16, base: u16, mask: u16) -> u16 {
    base.wrapping_add(level).wrapping_add(level << 4).wrapping_add(level << 8) & mask
}

/// Lightning tint: (`BASE`, `MASK`) in D5 and D4 of the original (`0x1ab6c`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flash {
    /// 0x1ab56.
    Gray,
    /// 0x1aaa0, 0x1aaba, 0x1aad4, 0x1aaee, 0x1ab08, 0x1ab22, 0x1ab3c.
    A,
    B,
    C,
    D,
    E,
    F,
    G,
}

impl Flash {
    pub(crate) const fn tint(self) -> (u16, u16) {
        match self {
            Flash::Gray => (0xf300, 0xff00),
            Flash::A => (0xf002, 0xffff),
            Flash::B => (0xf000, 0xf00f),
            Flash::C => (0xf320, 0xffff),
            Flash::D => (0xf033, 0xffff),
            Flash::E => (0xf030, 0xffff),
            Flash::F => (0xf004, 0xffff),
            Flash::G => (0xf040, 0xf0ff),
        }
    }
}

/// 0x1ac24: sets the lightning palettes aside.
pub fn save_flash(b: &mut Cps2Bus) {
    copy_flash_palettes(b, false);
}

/// 0x1abe0: puts the lightning palettes back.
pub fn restore_flash(b: &mut Cps2Bus) {
    copy_flash_palettes(b, true);
}

/// Lightning: sets the palettes aside then replaces them with tinted greys. Returns the last colour processed
/// (from which the original derives its work registers).
pub fn flash(b: &mut Cps2Bus, tint: Flash) -> u16 {
    let (base, mask) = tint.tint();
    copy_flash_palettes(b, false);
    let mut last = 0;
    for (start, colors) in [(flash::PAGE, flash::PAGE_COLORS), (flash::SPARE, flash::SPARE_COLORS)] {
        for i in 0..colors {
            last = rd16(b, start + 2 * i);
            let tinted = flash_color(flash_level(last), base, mask);
            wr16(b, start + 2 * i, tinted);
        }
    }
    last
}

/// Colour of the screen flash (colour 14 of page 1 of the interface, `0x91c41c`): 0x1ac68 sets it to
/// 0xff00 (`FLASH_COLOR_ON`), 0x1ac86 and 0x35288 to 0xffff (white).
pub fn set_flash_color(b: &mut Cps2Bus, color: u16) {
    wr16(b, FLASH_COLOR, color);
}

const FLASH_COLOR: u32 = 0x91c41c;
pub const FLASH_COLOR_RED: u16 = 0xff00;
pub const FLASH_COLOR_WHITE_VALUE: u16 = 0xffff;

/// 0x37882: palette of the character of object `o` (`+3`): 8 longs copied from the plain table 0x390c2 to
/// page 0, at the same offset (offset table 0x378aa).
pub fn character_palette(b: &mut Cps2Bus, o: crate::objects::Obj) {
    let (_, from, to) = character_palette_at(b, o.0);
    copy_palette(b, from, to);
}

/// (offset, source, destination) of the palette of the character of object `o`.
pub(crate) fn character_palette_at(b: &mut Cps2Bus, o: u32) -> (u32, u32, u32) {
    let character = rd8(b, o + 3) as u32;
    let offset = crate::mem::prog16(b, 0x378aa + 2 * character) as i16 as i32 as u32;
    (offset, 0x390c2u32.wrapping_add(offset), 0x91c000u32.wrapping_add(offset))
}

/// Native routines of this module (the brightness settings are declared in `lib.rs`).
pub const NATIVES: &[Native] = &[
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    WHITE_PAGES,
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
    
];
