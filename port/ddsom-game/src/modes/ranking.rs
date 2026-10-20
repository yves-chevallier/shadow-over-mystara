//! Écran de fin de partie (sous-mode 20, `0xfc576`) : le classement défile, puis les scores des
//! joueurs s'affichent. La machine à modes y arrive après la phase 20 (`0x1c80c`), avec le drapeau de
//! démarrage levé (`0x133f6`) ; elle le baisse en partant (`0x133f0`).
//!
//! L'écran a son propre bloc d'état, à `FF2B10` :
//!
//! | Champ | Rôle |
//! |---|---|
//! | `+0` | étape (pas de 2) : 0 préparation, 2 défilement du classement, 4 scores, 6 et 8 décor, 10 sortie |
//! | `+4` | minuterie (mot) |
//! | `+6` | pointeur sur le texte du classement (long, dans la ROM) |
//! | `+10` | ordonnée de la caméra à la dernière ligne ajoutée (mot) |
//!
//! Le défilement vertical est le long `FF2B1C` (16.16) ; un bouton pressé le ralentit.

use super::{Block, Flow, Sequence};
use crate::mem::{rd16, rd32, rd8, set_low_word, wr16, wr32, wr8, A5};
use crate::resume;
use crate::scene::Ctx;
use crate::Native;

const STATE: u32 = 0xff_2b10;
mod f {
    pub const STEP: u32 = 0;
    pub const TIMER: u32 = 4;
    pub const TEXT: u32 = 6;
    pub const MARK: u32 = 0xa;
}
/// Défilement du fond (long, 16.16) et du second plan.
const SCROLL: u32 = 0xff_2b1c;
const SCROLL_2: u32 = 0xff_2b20;
/// Table des quatre joueurs (16 octets chacun : score et état).
const SCORES: u32 = A5 - 0x5b18;
const FRAME_COUNT: u32 = A5 + 0x7e68;
const VIDEO: u32 = A5 + 0x14;
const CPS_REG_4: u32 = 0x400004;

mod ext {
    pub const SOUND_E172C: u32 = 0xe172c;
    pub const SOUND_E0A82: u32 = 0xe0a82;
    pub const SOUND_E0882: u32 = 0xe0882;
    pub const RESET_PLAYERS: u32 = 0x1f184;
    pub const SPRITES_RESET: u32 = 0x1de7c;
    pub const SPRITES: u32 = 0x1de96;
    pub const CLEAR_SPRITES: u32 = 0xc58;
    pub const CLEAR_SCROLL_1: u32 = 0xca8;
    pub const CLEAR_SCROLL_2: u32 = 0xcc8;
    pub const CLEAR_SCROLL_3: u32 = 0xce8;
    pub const POOLS: u32 = 0x2062;
    pub const PEFF: u32 = 0x3996c;
    pub const EFFCT: u32 = 0x39cba;
    pub const PALETTE_PAGE_B: u32 = 0x1a608;
    pub const PALETTE_SET: u32 = 0x1a67e;
    pub const PALETTE_93: u32 = 0x1a366;
    pub const TILE_BLOCK: u32 = 0x26d4;
    pub const BRIGHTNESS_WHITE: u32 = 0x1686;
    pub const FADE_OUT: u32 = 0x1460;
    pub const LINE: u32 = 0x517ba;
    pub const SCORE_OBJECT: u32 = 0xb1a7a;
    pub const BOOTED_CLEAR: u32 = 0x133f0;
    pub const CLEAR_STATUS: u32 = 0xdabba;
}

/// Retour de l'étape à `0xfc584` : A0 revient sur le bloc de la machine à modes.
const RETURN: u32 = 0xfc584;

/// Écran du sous-mode `sub`, s'il est réécrit ici.
pub fn screen(x: &mut Ctx, sub: u8) -> Option<Flow> {
    (sub == 0x14).then(|| post_game(x))
}

fn post_game(x: &mut Ctx) -> Flow {
    x.c.a[0] = STATE;
    let step = rd8(x.b, STATE + f::STEP);
    x.c.d[0] = step as u32;
    x.push_return(RETURN);
    match step {
        0 => s0_prepare(x),
        2 => s2_scroll(x),
        4 => s4_scores(x),
        6 => s6_decor(x),
        8 => s8_decor(x),
        10 => sa_leave(x),
        _ => Flow::Done,
    }
}

resume!(R_RETURN, RETURN, |x| {
    x.c.a[0] = super::mode::BASE;
    Flow::Done
});

fn next_step(x: &mut Ctx) {
    let s = rd8(x.b, STATE).wrapping_add(2);
    wr8(x.b, STATE, s);
}

/// Un rendu : PEFF, EFFCT, sprites (`0xfc91c`).
fn render(x: &mut Ctx) {
    for f in [ext::PEFF, ext::EFFCT, ext::SPRITES] {
        x.call(f);
    }
    x.c.a[0] = STATE;
}

/// `0xfc6f8` : vitesse du défilement (1 pixel par image, 1/4 si un bouton est pressé).
fn speed(x: &mut Ctx) -> u32 {
    let mut buttons = 0;
    for k in 0..4 {
        buttons |= rd8(x.b, A5 + k);
    }
    x.c.d[1] = (x.c.d[1] & !0xff) | (buttons & 0xf0) as u32;
    x.c.d[0] = if buttons & 0xf0 == 0 { 0x10000 } else { 0x4000 };
    x.c.d[0]
}

/// Fait avancer le défilement du fond d'une image ; la caméra 2 le suit.
fn scroll(x: &mut Ctx) {
    let d0 = speed(x);
    let v = rd32(x.b, SCROLL).wrapping_sub(d0);
    wr32(x.b, SCROLL, v);
    wr16(x.b, A5 + 0x55a, (v >> 16) as u16);
}

fn s0_prepare(x: &mut Ctx) -> Flow {
    next_step(x);
    wr16(x.b, STATE + f::TIMER, 300);
    x.call(ext::SOUND_E172C);
    for f in [ext::RESET_PLAYERS, ext::SPRITES_RESET, ext::CLEAR_SPRITES, ext::POOLS] {
        x.call(f);
    }
    x.c.a[0] = STATE;
    x.wait(1, S0_VIDEO)
}
const S0_VIDEO: u32 = 0xfc5cc;
resume!(R_S0_VIDEO, S0_VIDEO, |x| {
    wr16(x.b, CPS_REG_4, 0x1355);
    wr16(x.b, VIDEO, 0x24c0);
    x.wait(1, S0_PALETTES)
});
const S0_PALETTES: u32 = 0xfc5e2;
resume!(R_S0_PALETTES, S0_PALETTES, |x| {
    for f in [ext::CLEAR_SCROLL_1, ext::CLEAR_SCROLL_2, ext::CLEAR_SCROLL_3, ext::PALETTE_PAGE_B] {
        x.call(f);
    }
    x.call_d0_low(ext::PALETTE_SET, 83);
    x.call_d0_low(ext::PALETTE_SET, 83);
    wr8(x.b, A5 + 0x34, 0);
    x.call_d0_low(ext::PALETTE_93, 89);
    // quatre blocs de palette, de la ROM vers la table des palettes (D0-D6 et A3 gardent le dernier)
    for (dst, src) in [(0x91c3e0u32, 0x291f00u32), (0x91c2e0, 0xfc6d8), (0x91c000, 0x292260), (0x91c3c0, 0x2537c0)] {
        x.c.a[1] = dst;
        x.c.a[2] = src;
        let words: Vec<u32> = (0..8).map(|i| rd32(x.b, src + 4 * i)).collect();
        for (i, w) in words.iter().enumerate() {
            wr32(x.b, dst + 4 * i as u32, *w);
        }
        x.c.d[..7].copy_from_slice(&words[..7]);
        x.c.a[3] = words[7];
    }
    wr8(x.b, A5 + 0x48e, 0xf);
    x.call(ext::BRIGHTNESS_WHITE);
    for off in [0x506u32, 0x50a, 0x556, 0x4b6, 0x4ba] {
        wr16(x.b, A5 + off, 0);
    }
    wr16(x.b, A5 + 0x55a, 0x701);
    wr32(x.b, SCROLL, 0x0701_0000);
    wr8(x.b, A5 + 0x36, 1);
    x.wait(4, S0_TILES)
});
const S0_TILES: u32 = 0xfc6a8;
resume!(R_S0_TILES, S0_TILES, |x| {
    x.c.a[1] = 0x917000;
    x.call_d0_low(ext::TILE_BLOCK, 197);
    x.call_d0_low(ext::TILE_BLOCK, 198);
    wr16(x.b, VIDEO, 0x24ce);
    wr16(x.b, STATE + f::MARK, 0x728);
    wr32(x.b, STATE + f::TEXT, 0xfc938);
    Flow::Done
});

/// Étape 2 : le classement défile ; toutes les 40 lignes de pixels, les lignes suivantes du texte
/// sont ajoutées. Quand le texte est fini (octet négatif), étape suivante.
fn s2_scroll(x: &mut Ctx) -> Flow {
    render(x);
    scroll(x);
    let d0 = rd16(x.b, STATE + f::MARK).wrapping_sub(rd16(x.b, A5 + 0x55a));
    if d0 as u8 != 40 {
        return Flow::Done;
    }
    let camera = rd16(x.b, A5 + 0x55a);
    wr16(x.b, STATE + f::MARK, camera);
    let mut a4 = rd32(x.b, STATE + f::TEXT);
    for counter in (0..6u32).rev() {
        x.c.d[0] = counter;
        if rd8(x.b, a4) & 0x80 != 0 {
            next_step(x);
            return Flow::Done;
        }
        x.c.a[4] = a4;
        x.call(ext::LINE);
        a4 = x.c.a[4];
    }
    if rd8(x.b, a4) & 0x80 != 0 {
        next_step(x);
        return Flow::Done;
    }
    wr32(x.b, STATE + f::TEXT, a4);
    Flow::Done
}

/// Étape 4 : les cartouches des scores (un objet par joueur ayant joué), après dix images de jeu.
fn s4_scores(x: &mut Ctx) -> Flow {
    if (rd16(x.b, FRAME_COUNT) as i16) < 10 {
        return Flow::Done;
    }
    render(x);
    scroll(x);
    next_step(x);
    for kind in [4u32, 6] {
        x.call_d0(ext::SCORE_OBJECT, kind);
    }
    x.c.d[0] = 5;
    x.c.d[2] = 3;
    let mut a4 = SCORES;
    for counter in (0..4u32).rev() {
        x.c.d[1] = counter;
        if rd32(x.b, a4) != 0 || rd32(x.b, a4 + 4) != 0 {
            x.c.d[0] = 5;
            x.call(ext::SCORE_OBJECT);
            let o = x.c.a[1];
            wr8(x.b, o + 0x42, x.c.d[2] as u8);
            x.c.d[2] = (x.c.d[2] & !0xff) | (x.c.d[2] as u8).wrapping_sub(1) as u32;
            wr32(x.b, o + 0x4c, a4);
        }
        a4 += 0x10;
    }
    x.c.d[0] = 9;
    x.c.d[2] = 3;
    a4 = SCORES;
    for counter in (0..4u32).rev() {
        x.c.d[1] = counter;
        if rd8(x.b, a4) != 0 {
            if rd8(x.b, a4 + 14) != 0 {
                x.c.d[0] = 9;
                x.call(ext::SCORE_OBJECT);
                let o = x.c.a[1];
                wr8(x.b, o + 0x42, x.c.d[2] as u8);
                wr32(x.b, o + 0x4c, a4);
            }
            x.c.d[2] = (x.c.d[2] & !0xff) | (x.c.d[2] as u8).wrapping_sub(1) as u32;
        }
        a4 += 0x10;
    }
    x.c.a[4] = a4;
    Flow::Done
}

/// Étape 6 : le fond descend jusqu'à l'ordonnée 1064, puis un second plan se met à défiler.
fn s6_decor(x: &mut Ctx) -> Flow {
    render(x);
    scroll(x);
    if rd16(x.b, A5 + 0x55a) != 1064 {
        return Flow::Done;
    }
    next_step(x);
    wr32(x.b, SCROLL_2, 0x0108_0000);
    wr16(x.b, A5 + 0x4ba, 264);
    Flow::Done
}

/// Étape 8 : les deux plans défilent ensemble jusqu'à l'ordonnée 832.
fn s8_decor(x: &mut Ctx) -> Flow {
    render(x);
    scroll(x);
    let v = rd32(x.b, SCROLL_2).wrapping_sub(x.c.d[0]);
    wr32(x.b, SCROLL_2, v);
    wr16(x.b, A5 + 0x4ba, (v >> 16) as u16);
    if rd16(x.b, A5 + 0x55a) != 832 {
        return Flow::Done;
    }
    next_step(x);
    wr16(x.b, STATE + f::TIMER, 300);
    Flow::Done
}

/// Étape 10 : après 300 images (ou un bouton), fondu au noir et retour à l'attract.
fn sa_leave(x: &mut Ctx) -> Flow {
    let mut buttons = 0;
    for k in 0..4 {
        buttons |= rd8(x.b, A5 + 0xc + k);
    }
    x.c.d[1] = (x.c.d[1] & !0xff) | (buttons & 0xf0) as u32;
    if buttons & 0xf0 != 0 {
        wr16(x.b, STATE + f::TIMER, 1);
    }
    let left = rd16(x.b, STATE + f::TIMER).wrapping_sub(1);
    wr16(x.b, STATE + f::TIMER, left);
    if left != 0 {
        render(x);
        return Flow::Done;
    }
    x.call_d0(ext::SOUND_E0A82, 4);
    wr8(x.b, A5 + 0x48e, 0xf);
    wr16(x.b, A5 + 0x490, 3);
    x.tail(ext::FADE_OUT, SA_FADED)
}
const SA_FADED: u32 = 0xfc8a8;
resume!(R_SA_FADED, SA_FADED, |x| {
    x.call(ext::SOUND_E0882);
    for f in [ext::RESET_PLAYERS, ext::SPRITES_RESET, ext::CLEAR_SPRITES, ext::POOLS] {
        x.call(f);
    }
    x.wait(1, SA_VIDEO)
});
const SA_VIDEO: u32 = 0xfc8ce;
resume!(R_SA_VIDEO, SA_VIDEO, |x| {
    wr16(x.b, VIDEO, 0x12c0);
    x.wait(4, SA_CLEARED)
});
const SA_CLEARED: u32 = 0xfc8dc;
resume!(R_SA_CLEARED, SA_CLEARED, |x| {
    for f in [ext::CLEAR_SCROLL_1, ext::CLEAR_SCROLL_2, ext::CLEAR_SCROLL_3] {
        x.call(f);
    }
    wr8(x.b, super::mode::SUB, 0);
    wr16(x.b, super::mode::STEP, 0);
    wr8(x.b, A5 + 0x36, 0);
    x.c.d[0] = 0;
    for off in [0x4bau32, 0x50a, 0x55a] {
        wr16(x.b, A5 + off, 0);
    }
    x.call(ext::BOOTED_CLEAR);
    x.call(ext::CLEAR_STATUS);
    wr8(x.b, A5 + 0x30, 0);
    Flow::Done
});

/// Charge D0 sur son mot bas (`move.w #n,D0w`) avant l'appel.
trait LowWord {
    fn call_d0_low(&mut self, addr: u32, d0: u16);
}
impl LowWord for Ctx<'_> {
    fn call_d0_low(&mut self, addr: u32, d0: u16) {
        set_low_word(&mut self.c.d[0], d0);
        self.call(addr);
    }
}

pub const NATIVES: &[Native] = &[
    R_RETURN,
    R_S0_VIDEO,
    R_S0_PALETTES,
    R_S0_TILES,
    R_SA_FADED,
    R_SA_VIDEO,
    R_SA_CLEARED,
];
