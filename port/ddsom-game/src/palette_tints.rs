//! Teintes de la palette (U2) : sauvegarde de la palette, teinte en niveaux d'une couleur, restitution.
//!
//! Les effets de scène (éclair, nuit, brume...) sauvent la palette (0x1ac24), la remplacent par une
//! teinte monochrome où chaque couleur devient une gradation de sa clarté, puis la restituent
//! (0x1abe0). Deux zones de palette sont concernées : 1024 couleurs depuis 0x91c800 et 48 depuis
//! 0x91c3a0. Voir `docs/video.md`.

use cps2::Cps2Bus;
use m68k::{Cpu, SR_C, SR_N, SR_V, SR_Z};

use crate::mem::{rd16, rd32, rts, wr16, wr32};
use crate::Native;

/// Zones de palette (adresse GFX RAM, nombre de couleurs) touchées par les teintes.
const ZONES: [(u32, u32); 2] = [(0x91c800, 1024), (0x91c3a0, 48)];
/// Copie de la palette en RAM de travail.
const BACKUP: u32 = 0xff1190;

/// Taille de la copie, en longs.
fn backup_longs() -> u32 {
    ZONES.iter().map(|&(_, n)| n / 2).sum()
}

/// 0x1ac24 (sauvegarde) et 0x1abe0 (restitution) : copie la palette vers / depuis la RAM de travail.
fn copy_palette(c: &mut Cpu, b: &mut Cps2Bus, restore: bool) {
    let mut ram = BACKUP;
    for &(gfx, colors) in &ZONES {
        for i in 0..colors / 2 {
            let (from, to) = if restore { (ram, gfx + 4 * i) } else { (gfx + 4 * i, ram) };
            let v = rd32(b, from);
            wr32(b, to, v);
            ram += 4;
        }
    }
    // registres que le code d'origine laisse : A1 = fin de la source, A2 = fin de la destination, D6 = -1
    let (gfx_end, ram_end) = (ZONES[1].0 + 2 * ZONES[1].1, BACKUP + 4 * backup_longs());
    (c.a[1], c.a[2]) = if restore { (ram_end, gfx_end) } else { (gfx_end, ram_end) };
    c.d[6] |= 0xffff;
    c.sr &= !(SR_N | SR_Z | SR_V | SR_C);
}

/// Clarté d'une couleur 0xLRGB : (r + g + b + 4) / 4, de 0 à 11.
fn lightness(color: u16) -> u16 {
    ((color >> 8 & 0xf) + (color >> 4 & 0xf) + (color & 0xf) + 4) >> 2
}

/// Teinte : chaque couleur devient `bias + k x 0x111` (k = sa clarté), masquée par `mask`.
fn tint_color(color: u16, bias: u16, mask: u16) -> u16 {
    let k = lightness(color);
    bias.wrapping_add(k).wrapping_add(k << 4).wrapping_add(k << 8) & mask
}

/// Sauve la palette, puis la teinte. Rend les registres du code d'origine (dernière couleur traitée).
fn tint(c: &mut Cpu, b: &mut Cps2Bus, bias: u16, mask: u16) {
    copy_palette(c, b, false);
    let mut last = 0;
    for &(gfx, colors) in &ZONES {
        for i in 0..colors {
            let color = rd16(b, gfx + 2 * i);
            last = color;
            wr16(b, gfx + 2 * i, tint_color(color, bias, mask));
        }
    }
    let out = tint_color(last, bias, mask);
    let (r, g) = (last >> 8 & 0xf, last >> 4 & 0xf);
    let k = lightness(last);
    crate::mem::set_low_word(&mut c.d[0], out);
    crate::mem::set_low_word(&mut c.d[1], r);
    crate::mem::set_low_word(&mut c.d[2], r + g);
    crate::mem::set_low_word(&mut c.d[3], k << 8);
    crate::mem::set_low_word(&mut c.d[4], mask);
    crate::mem::set_low_word(&mut c.d[5], bias);
    c.d[6] |= 0xffff;
    c.a[1] = ZONES[1].0 + 2 * ZONES[1].1;
    c.sr &= !(SR_N | SR_Z | SR_V | SR_C);
    if out == 0 {
        c.sr |= SR_Z;
    }
    if out & 0x8000 != 0 {
        c.sr |= SR_N;
    }
}

const fn palette(entry: u32, name: &'static str, run: fn(&mut Cpu, &mut Cps2Bus)) -> Native {
    Native { entry, name, cost: |_, _| 0, run, touches_sound: false, outputs: 0, exact_cost: false }
}

macro_rules! tint {
    ($entry:literal, $name:literal, $bias:literal, $mask:literal) => {
        palette($entry, $name, |c, b| {
            tint(c, b, $bias, $mask);
            rts(c, b);
        })
    };
}

/// Routines natives de ce module.
pub static NATIVES: [Native; 10] = [
    tint!(0x1aaa0, "teinte_bleutee", 0xf002, 0xffff),
    tint!(0x1aaba, "teinte_sombre", 0xf000, 0xf00f),
    tint!(0x1aad4, "teinte_chaude", 0xf320, 0xffff),
    tint!(0x1aaee, "teinte_verte", 0xf033, 0xffff),
    tint!(0x1ab08, "teinte_cyan", 0xf030, 0xffff),
    tint!(0x1ab22, "teinte_bleue", 0xf004, 0xffff),
    tint!(0x1ab3c, "teinte_pourpre", 0xf040, 0xf0ff),
    tint!(0x1ab56, "teinte_sepia", 0xf300, 0xff00),
    palette(0x1ac24, "palette_sauve", |c, b| {
        copy_palette(c, b, false);
        rts(c, b);
    }),
    palette(0x1abe0, "palette_restitue", |c, b| {
        copy_palette(c, b, true);
        rts(c, b);
    }),
];
