//! Tools: the natives of `crate::palette` for the original code that calls them (arguments and results in the registers
//! of the original).

use cps2::Cps2Bus;
use m68k::decode::Size;
use m68k::Cpu;

use crate::palette::*;
use crate::tools::frames::*;
use crate::{regs, Native};
use crate::mem::{rd16, rd32, rd8, wr32};
use crate::video::hw;

macro_rules! page_native {
    ($name:ident, $ident:literal, $k:literal, $entry:expr) => {
        pub(crate) const $name: Native = native(
            $entry,
            $ident,
            |c, b| page_run(c, b, $k, $entry),
        );
    };
}

// Natives (calls by address): D0 is the argument; 0x1686 and 0x168c leave it set.
fn or_brightness_native(c: &mut Cpu, b: &mut Cps2Bus) {
    or_brightness(b, c.d[0] as u16);
    rts(c, b);
}

fn full_brightness_native(c: &mut Cpu, b: &mut Cps2Bus) {
    c.d[0] = (c.d[0] & 0xffff_0000) | 0xf000;
    or_brightness_native(c, b);
}

fn no_brightness_native(c: &mut Cpu, b: &mut Cps2Bus) {
    c.d[0] = 0;
    or_brightness_native(c, b);
}

fn set_brightness_native(c: &mut Cpu, b: &mut Cps2Bus) {
    set_brightness(b, c.d[0] as u16);
    rts(c, b);
}

const fn native(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus)) -> Native {
    Native { entry, name, run: crate::Body::WithCpu(run), touches_sound: false, outputs: 0 }
}

/// Registers that 0x1a36c leaves: D0 = current scene x 4, D1-D6/A1/A2 = last palette copied
/// (D1 = 0 if there are no extra palettes), A3, A4, A6, D7 = 0xffff.
fn stage_regs(c: &mut Cpu, b: &mut Cps2Bus, end: &StageEnd) {
    let current = rd8(b, c.a[5] + STAGE_VAR) as u32;
    c.d[0] = (current << 2) & 0xffff;
    if let Some(at) = end.last {
        let l = palette_longs(b, at);
        c.d[1..7].copy_from_slice(&l[..6]);
        c.a[1] = l[6];
        c.a[2] = l[7];
    }
    if end.extra == 0 {
        c.d[1] = 0;
        c.a[3] = STAGE_EXTRA;
    } else {
        c.a[3] = end.extra + EXTRA_PALETTES * PALETTE_BYTES;
    }
    c.d[7] = 0xffff;
    c.a[4] = end.dst;
    c.a[6] = end.list;
}

/// Arguments of 0x1a350 / 0x1a360 / 0x1a366: scene number loaded (D0) and destination.
/// 0x1a350 and 0x1a360 read the current scene; 0x1a366 takes D0 as is. 0x1a350 writes into the
/// scroll layer 1 (scenes 0x2f and 0x42, where page 0 serves as the fade source).
fn stage_args(b: &mut Cps2Bus, c: &Cpu, entry: u32) -> (u32, u32) {
    let stage = rd8(b, c.a[5] + STAGE_VAR) as u32;
    match entry {
        0x1a350 => (stage, hw::SCROLL1),
        0x1a360 => (stage, PALETTES),
        _ => (c.d[0], PALETTES),
    }
}

fn stage_run(c: &mut Cpu, b: &mut Cps2Bus, entry: u32) {
    let (d0, dst) = stage_args(b, c, entry);
    let end = load_stage_palettes(b, c.a[5], d0, dst);
    stage_regs(c, b, &end);
    rts(c, b);
}

pub(crate) const STAGE_FREE: Native = native(0x1a366, "palette_scene_d0", |c, b| stage_run(c, b, 0x1a366));

pub(crate) const STAGE_PAGE0: Native = native(0x1a360, "palette_scene", |c, b| stage_run(c, b, 0x1a360));

pub(crate) const STAGE_SCROLL1: Native = native(0x1a350, "palette_scene_scroll1", |c, b| stage_run(c, b, 0x1a350));

fn set_run(c: &mut Cpu, b: &mut Cps2Bus) {
    let source = load_set(b, c.d[0]);
    // registers of the original: D0.w shifted, A4 = end of the page, A3 = end of the source
    let shifted = (c.d[0] as u16) << 2;
    set_low_word(&mut c.d[0], shifted);
    if source == 0 {
        c.d[1] = 0;
        c.a[3] = SETS;
        c.a[4] = PALETTES;
    } else {
        let l = palette_longs(b, PALETTES + (PAGE_PALETTES - 1) * PALETTE_BYTES);
        c.d[1..7].copy_from_slice(&l[..6]);
        c.a[1] = l[6];
        c.a[2] = l[7];
        set_low_word(&mut c.d[7], 0xffff);
        c.a[3] = source + PAGE_PALETTES * PALETTE_BYTES;
        c.a[4] = PALETTES + PAGE_PALETTES * PALETTE_BYTES;
    }
    rts(c, b);
}

/// Registers left by a page loader: D0-D6 = first seven longs of the last palette,
/// the eighth in A4 (pages 1 and 2) or A2 (page 3); source and destination (end) in A2/A3
/// (pages 1 and 2) or A4/A1 (page 3); D7.w = -1.
fn page_regs(c: &mut Cpu, b: &mut Cps2Bus, k: usize, source: u32) {
    let page = PALETTES + PAGE_SIZE * (k as u32 + 1);
    let l = palette_longs(b, page + (PAGE_PALETTES - 1) * PALETTE_BYTES);
    c.d[..7].copy_from_slice(&l[..7]);
    let (source_end, page_end) = (source + PAGE_PALETTES * PALETTE_BYTES, page + PAGE_PALETTES * PALETTE_BYTES);
    if k < 2 {
        c.a[4] = l[7];
        c.a[2] = source_end;
        c.a[3] = page_end;
    } else {
        c.a[2] = l[7];
        c.a[4] = source_end;
        c.a[1] = page_end;
    }
    set_low_word(&mut c.d[7], 0xffff);
}

/// Effective D0 of a page loader: the caller's for the "D0" entry, otherwise 0 (page 1)
/// or the current scene.
fn page_d0(b: &mut Cps2Bus, c: &Cpu, k: usize, entry: u32) -> u32 {
    if entry == PAGE_TABLES[k].2 {
        c.d[0]
    } else if k == 0 {
        0
    } else {
        rd8(b, c.a[5] + STAGE_VAR) as u32
    }
}

fn page_run(c: &mut Cpu, b: &mut Cps2Bus, k: usize, entry: u32) {
    let d0 = page_d0(b, c, k, entry);
    let source = load_page(b, k, d0);
    page_regs(c, b, k, source);
    rts(c, b);
}

fn load_all(c: &mut Cpu, b: &mut Cps2Bus) {
    load_all_with(b, |b, step| match step {
        Loaded::Stage(end) => stage_regs(c, b, end),
        Loaded::Page(k, source) => page_regs(c, b, k, source),
        Loaded::Cleared => c.flags_logic(0, Size::Word),
    });
    rts(c, b);
}

/// 0x1cd7c: one fade step of each checked page towards its target palette (`A5 + 0x492 + 4 k`).
fn fade_to_palettes(c: &mut Cpu, b: &mut Cps2Bus) {
    for (k, page) in flagged_pages(b, c.a[5]) {
        let target = rd32(b, c.a[5] + SOURCES + 4 * k as u32);
        approach_page(b, page, target);
    }
    rts(c, b);
}

/// 0x1cdd6: one fade step of each checked page towards the colour (word) `A5 + 0x492 + 4 k`.
fn fade_to_colors(c: &mut Cpu, b: &mut Cps2Bus) {
    for (k, page) in flagged_pages(b, c.a[5]) {
        let goal = rd16(b, c.a[5] + SOURCES + 4 * k as u32);
        approach_constant(b, page, goal);
    }
    rts(c, b);
}

/// 0x1cd32: one fade step towards white of each checked page.
fn fade_to_white(c: &mut Cpu, b: &mut Cps2Bus) {
    for (_, page) in flagged_pages(b, c.a[5]) {
        brighten_page(b, page);
    }
    rts(c, b);
}

fn fill_page_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let values = [c.d[0], c.d[1], c.d[2], c.d[3], c.d[4], c.d[5], c.d[6], c.a[1]];
    fill_page(b, c.a[0], values);
    c.a[0] = c.a[0].wrapping_add(0x400);
    rts(c, b);
}

pub(crate) const APPROACH_PAGE: Native = native(
    0x1cfc8,
    "palette_approach_page",
    |c, b| {
        approach_page(b, c.a[0], c.a[1]);
        rts(c, b);
    },
);

pub(crate) const APPROACH_CONSTANT: Native = native(
    0x1d042,
    "palette_approach_colour",
    |c, b| {
        approach_constant(b, c.a[0], c.d[1] as u16);
        rts(c, b);
    },
);

pub(crate) const BRIGHTEN_PAGE: Native = native(
    0x1d00e,
    "palette_eclaircit_page",
    |c, b| {
        brighten_page(b, c.a[0]);
        rts(c, b);
    },
);

fn save_flash_native(c: &mut Cpu, b: &mut Cps2Bus) {
    save_flash(b);
    c.a[1] = flash::SPARE + 2 * flash::SPARE_COLORS;
    c.a[2] = flash::BACKUP_SPARE + 2 * flash::SPARE_COLORS;
    set_low_word(&mut c.d[6], 0xffff);
    rts(c, b);
}

fn restore_flash_native(c: &mut Cpu, b: &mut Cps2Bus) {
    restore_flash(b);
    c.a[1] = flash::BACKUP_SPARE + 2 * flash::SPARE_COLORS;
    c.a[2] = flash::SPARE + 2 * flash::SPARE_COLORS;
    set_low_word(&mut c.d[6], 0xffff);
    rts(c, b);
}

/// Native of a lightning: the work registers as the original leaves them (last colour processed).
fn flash_native_run(c: &mut Cpu, b: &mut Cps2Bus, tint: Flash) {
    let last = flash(b, tint);
    let (base, mask) = tint.tint();
    let level = flash_level(last);
    let (red, green) = (last >> 8 & 0xf, last >> 4 & 0xf);
    c.a[1] = flash::SPARE + 2 * flash::SPARE_COLORS;
    set_low_word(&mut c.d[0], flash_color(level, base, mask));
    set_low_word(&mut c.d[1], red);
    set_low_word(&mut c.d[2], red + green);
    set_low_word(&mut c.d[3], level << 8);
    set_low_word(&mut c.d[4], mask);
    set_low_word(&mut c.d[5], base);
    set_low_word(&mut c.d[6], 0xffff);
    rts(c, b);
}

const fn flash_native(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus)) -> Native {
    Native { entry, name, run: crate::Body::WithCpu(run), touches_sound: false, outputs: regs::ALL }
}

pub(crate) const FLASH_GRAY: Native = flash_native(0x1ab56, "palette_lightning_grey", |c, b| flash_native_run(c, b, Flash::Gray));

pub(crate) const FLASH_A: Native = flash_native(0x1aaa0, "palette_lightning_a", |c, b| flash_native_run(c, b, Flash::A));

pub(crate) const FLASH_B: Native = flash_native(0x1aaba, "palette_lightning_b", |c, b| flash_native_run(c, b, Flash::B));

pub(crate) const FLASH_C: Native = flash_native(0x1aad4, "palette_lightning_c", |c, b| flash_native_run(c, b, Flash::C));

pub(crate) const FLASH_D: Native = flash_native(0x1aaee, "palette_lightning_d", |c, b| flash_native_run(c, b, Flash::D));

pub(crate) const FLASH_E: Native = flash_native(0x1ab08, "palette_lightning_e", |c, b| flash_native_run(c, b, Flash::E));

pub(crate) const FLASH_F: Native = flash_native(0x1ab22, "palette_lightning_f", |c, b| flash_native_run(c, b, Flash::F));

pub(crate) const FLASH_G: Native = flash_native(0x1ab3c, "palette_lightning_g", |c, b| flash_native_run(c, b, Flash::G));

/// The natives leave the flags of the `move.w`.
fn flash_color_native_run(c: &mut Cpu, b: &mut Cps2Bus, color: u16) {
    set_flash_color(b, color);
    c.flags_logic(color as u32, m68k::decode::Size::Word);
    rts(c, b);
}

const fn flash_color_native(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus)) -> Native {
    Native { entry, name, run: crate::Body::WithCpu(run), touches_sound: false, outputs: regs::CCR }
}

pub(crate) const FLASH_COLOR_ON: Native =
    flash_color_native(0x1ac68, "palette_lightning_colour", |c, b| flash_color_native_run(c, b, FLASH_COLOR_RED));

pub(crate) const FLASH_COLOR_WHITE: Native =
    flash_color_native(0x1ac86, "palette_lightning_white", |c, b| flash_color_native_run(c, b, FLASH_COLOR_WHITE_VALUE));

pub(crate) const FLASH_COLOR_WHITE_B: Native =
    flash_color_native(0x35288, "palette_lightning_white_b", |c, b| flash_color_native_run(c, b, FLASH_COLOR_WHITE_VALUE));

/// Copies `longs` × 8 longs from `from` to `to` by `movem.l` of eight registers (D0-D5, A3, A4 keep the
/// last block); returns the end addresses.
fn movem_copy(c: &mut Cpu, b: &mut Cps2Bus, from: u32, to: u32, blocks: u32, step: u32) -> (u32, u32) {
    let (mut from, mut to) = (from, to);
    for _ in 0..blocks {
        let v: [u32; 8] = std::array::from_fn(|i| rd32(b, from + 4 * i as u32));
        for (i, &x) in v.iter().enumerate() {
            wr32(b, to + 4 * i as u32, x);
        }
        c.d[..6].copy_from_slice(&v[..6]);
        c.a[3] = v[6];
        c.a[4] = v[7];
        from += 0x20;
        to += step;
    }
    (from, to)
}

/// 0x1ad2a: backup of palette page 1 (0x91c400, 1 KB) to FF1190.
fn save_page1(c: &mut Cpu, b: &mut Cps2Bus) {
    let (from, to) = movem_copy(c, b, 0x91c400, 0xff1190, 32, 0x20);
    c.a[2] = from;
    c.a[1] = to;
    c.d[6] = 0xffff;
    rts(c, b);
}

/// Native of 0x37882: D0 = offset, D1-D5/A3/A4 = the longs copied (`movem`), A1 = end of the source, A2 = the
/// destination.
fn character_palette_native(c: &mut Cpu, b: &mut Cps2Bus) {
    let (offset, from, to) = character_palette_at(b, c.a[0]);
    c.d[0] = offset & 0xffff;
    let (end, _) = movem_copy(c, b, from, to, 1, 0);
    c.a[1] = end;
    c.a[2] = to;
    rts(c, b);
}

pub(crate) const SAVE_PAGE1: Native = Native { entry: 0x1ad2a, name: "palette_saved_page1", run: crate::Body::WithCpu(save_page1), touches_sound: false, outputs: regs::ALL };

pub(crate) const CHARACTER_PALETTE: Native = Native { entry: 0x37882, name: "palette_character", run: crate::Body::WithCpu(character_palette_native), touches_sound: false, outputs: regs::ALL };

pub(crate) const FULL_BRIGHTNESS: Native = native(0x1686, "palette_full_brightness", full_brightness_native);

pub(crate) const NO_BRIGHTNESS: Native = native(0x168c, "palette_no_brightness", no_brightness_native);

pub(crate) const OR_BRIGHTNESS: Native = native(0x168e, "palette_or_brightness", or_brightness_native);

pub(crate) const SET_BRIGHTNESS: Native = native(0x173c, "palette_set_brightness", set_brightness_native);

pub(crate) const LOAD_SET: Native = native(0x1a3d4, "palette_set", set_run);

pub(crate) const LOAD_ALL: Native = native(0x1a296, "palette_chargement_scene", load_all);

pub(crate) const FADE_TO_PALETTES: Native = native(0x1cd7c, "palette_fade_to_palettes", fade_to_palettes);

pub(crate) const FADE_TO_COLORS: Native = native(0x1cdd6, "palette_fade_to_colours", fade_to_colors);

pub(crate) const FADE_TO_WHITE: Native = native(0x1cd32, "palette_fade_to_white", fade_to_white);

pub(crate) const FILL_PAGE: Native = native(0x1cfb8, "palette_page_unie", fill_page_native);

pub(crate) const FLASH_RESTORE: Native = flash_native(0x1abe0, "palette_lightning_restore", restore_flash_native);

pub(crate) const FLASH_SAVE: Native = flash_native(0x1ac24, "palette_lightning_reserve", save_flash_native);

page_native!(PAGE1_STAGE, "palette_page1", 0, 0x1a608);

page_native!(PAGE1_D0, "palette_page1_d0", 0, 0x1a60a);

page_native!(PAGE2_STAGE, "palette_page2", 1, 0x1a678);

page_native!(PAGE2_D0, "palette_page2_d0", 1, 0x1a67e);

page_native!(PAGE3_STAGE, "palette_page3", 2, 0x1a8ac);

page_native!(PAGE3_D0, "palette_page3_d0", 2, 0x1a8b2);

fn set_low_word(r: &mut u32, v: u16) {
    *r = *r & 0xffff_0000 | v as u32;
}
