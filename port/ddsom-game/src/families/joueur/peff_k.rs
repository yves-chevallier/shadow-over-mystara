//! Effets des joueurs (PEFF), onzième série : types 40 (météores) et 45 (voyants de l'écran de choix).
//! Voir `docs/familles/joueur.md`.

use crate::families::cx::{handlers, Cx};
use super::pshell::*;
use crate::mem::{rd32, wr32};
use crate::objects::pools;

const REINSERT: u32 = 0x1ee1a;
/// Fenêtre de palette de la RAM graphique recopiée par les effets (8 longs).
const PALETTE_WINDOW: u32 = 0x91c1e0;
/// Masque ajouté ou combiné aux couleurs copiées (bit de transparence des mots).
const PALETTE_MASK: u32 = 0xf000_f000;

/// Recopie 8 longs de la ROM vers la fenêtre de palette (`movem.l (A2)+,{D1-D6,A3,A4}` puis `movem.l`
/// vers `(A1)`) : les six premiers longs sont combinés au masque par OU, les deux derniers l'additionnent.
fn copy_palette(x: &mut Cx, source: u32) {
    let mut words = [0u32; 8];
    for (i, w) in words.iter_mut().enumerate() {
        *w = rd32(x.b, source + 4 * i as u32);
    }
    for w in words.iter_mut().take(6) {
        *w |= PALETTE_MASK;
    }
    for w in words.iter_mut().skip(6) {
        *w = w.wrapping_add(PALETTE_MASK);
    }
    for (i, w) in words.iter().enumerate() {
        wr32(x.b, PALETTE_WINDOW + 4 * i as u32, *w);
    }
    for (i, w) in words.iter().take(6).enumerate() {
        x.c.d[1 + i] = *w;
    }
    x.c.a[3] = words[6];
    x.c.a[4] = words[7];
    x.c.a[1] = PALETTE_WINDOW;
    x.c.a[2] = source + 32;
}

/// Type 40 (0x50148) : pluie de météores (magie 20, kind 0x28). Variante 0 : le nuage, qui change la
/// palette de l'écran puis lâche des météores (variante 1) en se tenant `+0x168` images ; variante 1 :
/// un météore qui tombe d'un point aléatoire autour du nuage.
mod meteors {
    use super::*;

    const ANIMS: u32 = 0x1bc790;
    const PALETTE: u32 = 0x25a780;
    const SFX: u32 = 0xe9be4;
    const KIND: u8 = 0x28;

    pub fn run(x: &mut Cx) {
        let child = x.u8(0x3) != 0;
        match x.state(0x50156) {
            0 if !child => {
                x.call(SFX);
                copy_palette(x, PALETTE);
                x.add8(0x4, 2);
                follow_owner(x);
                x.add16(0xc, 0);
                x.add16(0x10, 1);
                x.set16(0x58, 0x510);
                x.anim(ANIMS, 0);
                x.call(REINSERT);
            }
            0 => {
                x.add8(0x4, 2);
                let parent = x.link_a1(0x5c);
                let (px, pz) = (parent.u16(x.b, 0x8), parent.u16(x.b, 0xc));
                x.set16(0x8, px);
                x.set16(0xc, pz);
                let y = parent.u32(x.b, 0x10).wrapping_sub(1);
                x.set32(0x10, y);
                let dx = (x.rand_and(0xff) as u16).wrapping_sub(0x80);
                x.add16(0x8, dx);
                x.c.d[2] = dx as u32;
                let dz = (x.rand_and(0x7f) as u16).wrapping_sub(0x3f);
                x.add16(0xc, dz);
                x.anim(ANIMS, 1);
                x.call(REINSERT);
            }
            2 if !child => {
                x.c.d[0] = x.c.d[0] & !0xff | x.u8(0x5) as u32;
                match x.u8(0x5) {
                    0 => cloud(x),
                    _ => {
                        let hidden = x.u8(0x1) & 2 == 0;
                        x.set_bit(0x1, 1, hidden);
                        x.add16(0x58, 0xffff);
                        if (x.u16(0x58) as i16) < 0 {
                            x.free_peff();
                        }
                    }
                }
            }
            2 => {
                x.call(ANIM_STEP);
                if x.c.cond(5) {
                    x.free_peff();
                }
            }
            _ => x.free_peff(),
        }
    }

    fn cloud(x: &mut Cx) {
        let owner = x.link_a1(f::OWNER);
        if owner.u16(x.b, 0x168) <= 0x20 {
            x.add8(0x5, 4);
            x.set16(0x58, 0x20);
            return;
        }
        x.add8(0x59, 0xff);
        if (x.u8(0x59) as i8) < 0 {
            x.set8(0x59, 0x20);
            x.add8(0x58, 0xff);
            if (x.u8(0x58) as i8) >= 0 {
                x.call(SFX);
            }
        }
        x.call(ANIM_STEP);
        let signal = x.u8(0x33);
        if signal == 0 {
            return;
        }
        if signal & 0x80 != 0 {
            x.add16(0xc, 1);
            return;
        }
        for _ in 0..2 {
            let Some(o) = x.alloc(&pools::PEFF) else { return };
            o.set_u8(x.b, 0, 1);
            o.set_u8(x.b, 2, KIND);
            o.set_u16(x.b, 0x5c, x.o.0 as u16);
            let owner = x.u16(f::OWNER);
            o.set_u16(x.b, f::OWNER, owner);
            o.set_u8(x.b, 0x3, 1);
        }
    }
}

/// Type 45 (0x518b4) : voyant de l'écran de choix (kind 0x2d) posé sur le joueur ; la variante `+0x38`
/// choisit l'effet de palette lancé quand le script le signale.
mod lamp {
    use super::*;

    const ANIMS: u32 = 0x1be8b8;
    const OFFSETS_X: u32 = 0x51982;
    const OFFSETS_Z: u32 = 0x51996;
    const PALETTE_A: u32 = 0x25b200;
    const PALETTE_B: u32 = 0x25a780;
    const FADE_ON: u32 = 0x1ab56;
    const FADE_OFF: u32 = 0x1abe0;
    const FADE_C: u32 = 0x1aaba;
    const FADE_D: u32 = 0x1aaee;
    const FADE_E: u32 = 0x1aad4;
    const FADE_F: u32 = 0x1ab3c;
    const SFX: u32 = 0xea09c;

    pub fn run(x: &mut Cx) {
        match x.state(0x518c2) {
            0 => {
                x.add8(0x4, 2);
                let owner = x.link_a1(f::OWNER);
                x.copy_pos(owner);
                x.add16(0xc, 0x40);
                x.add16(0x10, 0xffff);
                x.anim(ANIMS, 0);
                x.call(SFX);
                active(x);
            }
            2 => active(x),
            _ => x.free_peff(),
        }
    }

    fn active(x: &mut Cx) {
        let owner = x.link_a1(f::OWNER);
        if x.u8(0x5d) != owner.u8(x.b, 0x7) {
            x.add8(0x4, 2);
            return x.call(FADE_OFF);
        }
        x.copy_pos(owner);
        x.add16(0xc, 0x40);
        x.add16(0x10, 0xffff);
        let row = (x.u8(0x38) as u32) * 2;
        x.c.d[0] = row;
        let dx = x.rom16(OFFSETS_X + row);
        let dz = x.rom16(OFFSETS_Z + row);
        x.c.d[1] = dx as u32;
        x.c.d[2] = dz as u32;
        x.add16(0x8, dx);
        x.add16(0xc, dz);
        if x.u8(0x33) != 0 {
            x.set8(0x33, 0);
            palette_effect(x);
        }
        x.call(ANIM_STEP);
        x.set_bit(0x1, 1, false);
        let blink = x.a5_u8(a5::FRAME) & 1;
        x.c.d[0] = x.c.d[0] & !0xff | blink as u32;
        if blink != 0 {
            x.set_bit(0x1, 1, false);
        }
        x.call(REINSERT);
    }

    /// `0x519b0` : effet de palette de la variante.
    fn palette_effect(x: &mut Cx) {
        let v = x.u8(0x38);
        x.c.d[0] = v as u32 * 2;
        match v {
            0 => x.call(FADE_ON),
            1 => {
                copy_palette(x, PALETTE_A);
                x.call(FADE_C);
            }
            2 => {
                x.call(0x3afe);
                x.call(FADE_D);
            }
            3 => {
                x.call(0x3afe);
                x.call(FADE_E);
            }
            4 => {
                copy_palette(x, PALETTE_B);
                x.call(FADE_F);
            }
            _ => x.call(FADE_OFF),
        }
    }
}

handlers! {
    0x50148 "peff_40" 150 meteors::run,
    0x518b4 "peff_45" 100 lamp::run,
}
