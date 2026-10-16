//! Luminosité des palettes en GFX RAM.
//!
//! Une couleur matérielle est 0xLRGB (L = luminosité). Les quatre pages de palettes actives
//! (bits 0 à 3 de PAGES) sont réglées d'un coup : luminosité effacée, forcée ou remplacée.

use cps2::Cps2Bus;
use m68k::Cpu;

use crate::mem::{rd16, rd8, rts, wr16};
use crate::Native;

/// Pages de palettes concernées (bit k : page 0x91c000 + 0x400 k).
const PAGES: u32 = 0x48e;
const PALETTES: u32 = 0x91c000;
const PAGE_SIZE: u32 = 0x400;

fn for_each_color(c: &Cpu, b: &mut Cps2Bus, mut f: impl FnMut(u16) -> u16) {
    let pages = rd8(b, c.a[5] + PAGES);
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

/// 0x168e : luminosité D0 ajoutée (OU) aux couleurs, ou effacée si D0 = 0.
fn or_brightness(c: &mut Cpu, b: &mut Cps2Bus) {
    let level = c.d[0] as u16;
    for_each_color(c, b, |col| if level == 0 { col & 0x0fff } else { col | level });
    rts(c, b);
}

/// 0x1686 : luminosité maximale.
fn full_brightness(c: &mut Cpu, b: &mut Cps2Bus) {
    c.d[0] = (c.d[0] & 0xffff_0000) | 0xf000;
    or_brightness(c, b);
}

/// 0x168c : luminosité minimale.
fn no_brightness(c: &mut Cpu, b: &mut Cps2Bus) {
    c.d[0] = 0;
    or_brightness(c, b);
}

/// 0x173c : luminosité remplacée par D0.
fn set_brightness(c: &mut Cpu, b: &mut Cps2Bus) {
    let level = c.d[0] as u16;
    for_each_color(c, b, |col| (col & 0x0fff) | level);
    rts(c, b);
}

/// ~ 8 cycles par couleur et par page active.
fn cost(c: &Cpu, b: &mut Cps2Bus) -> u64 {
    100 + 8 * 512 * rd8(b, c.a[5] + PAGES).count_ones().min(4) as u64
}

const fn native(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus)) -> Native {
    Native { entry, name, cost, run, touches_sound: false, outputs: 0, exact_cost: false }
}

pub const FULL_BRIGHTNESS: Native = native(0x1686, "palette_full_brightness", full_brightness);
pub const NO_BRIGHTNESS: Native = native(0x168c, "palette_no_brightness", no_brightness);
pub const OR_BRIGHTNESS: Native = native(0x168e, "palette_or_brightness", or_brightness);
pub const SET_BRIGHTNESS: Native = native(0x173c, "palette_set_brightness", set_brightness);
