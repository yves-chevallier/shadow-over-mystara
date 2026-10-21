//! Écrans 4. COLOR, 5. DOT CROSS HATCH et 6. GAME DATA du menu TEST : deux étapes de préparation (ou une),
//! puis l'attente du retour au menu (départ 1P + tir 1).

use super::{call_d0, decimal, exit_check, ext, f, get, layers_off, layers_on, next_step, print, var};
use crate::mem::wr32;
use crate::modes::Flow;
use crate::scene::Ctx;

/// Aide commune aux mires : « EXIT = 1P START & SHOT1 ».
const HELP: u32 = 0x2a076;

// ---------------------------------------------------------------------------------------------
// 4. COLOR (0x29366)

mod color {
    /// Palettes des barres : 4 × 16 couleurs (rouge, vert, bleu, blanc) copiées dans les palettes 0x1c-0x1f.
    pub const RAMPS: u32 = 0x293a6;
    pub const PALETTES: u32 = 0x91c780;
    pub const TITLE: u32 = 0x2a224;
    /// Barres : (case de départ, palette).
    pub const BARS: [(u32, u16); 4] = [(0x9127a0, 0x1c), (0x9127b4, 0x1d), (0x9127c8, 0x1e), (0x9127dc, 0x1f)];
    /// Tuile de la première colonne (les suivantes : +1).
    pub const FIRST_TILE: u16 = 0x11;
}

pub fn color(x: &mut Ctx) -> Flow {
    match get(x, f::STEP) {
        0 => {
            next_step(x);
            layers_off(x);
            for k in 0..0x40 {
                let v = x.rd16(color::RAMPS + 2 * k);
                x.wr16(color::PALETTES + 2 * k, v);
            }
            Flow::Done
        }
        2 => {
            next_step(x);
            print(x, color::TITLE);
            print(x, HELP);
            for (cell, palette) in color::BARS {
                bar(x, cell, palette);
            }
            layers_on(x);
            Flow::Done
        }
        _ => exit_check(x),
    }
}

/// `0x29470` : une barre de 15 colonnes de deux cases de large et quatre de haut, une couleur par colonne.
fn bar(x: &mut Ctx, cell: u32, palette: u16) {
    for column in 0..15 {
        let tile = color::FIRST_TILE + column as u16;
        let top = cell + 0x100 * column;
        for row in 0..4 {
            let at = top + 4 * row;
            for c in [at, at + 0x80] {
                x.wr16(c, tile);
                x.wr16(c + 2, palette);
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 5. DOT CROSS HATCH (0x2949c)

mod hatch {
    pub const TITLE: u32 = 0x2a298;
    /// Palette 0x40 (0x91c800) : blanc, puis 0xff00 en alternance.
    pub const PALETTE: u32 = 0x91c800;
    /// Plan 2 (cases de 32 bits, colonnes de 0x40 octets).
    pub const LAYER: u32 = 0x917000;
}

pub fn cross_hatch(x: &mut Ctx) -> Flow {
    match get(x, f::STEP) {
        0 => {
            next_step(x);
            x.set_v16(var::LAYERS, super::ALL_LAYERS);
            // D0 vaut l'étape (0) : aucune couche coupée
            call_d0(x, ext::LAYERS_OFF, 0);
            print(x, hatch::TITLE);
            print(x, HELP);
            for k in 0..8 {
                wr32(x.b, hatch::PALETTE + 4 * k, 0xffff_ffff);
                wr32(x.b, hatch::PALETTE + 0x20 + 4 * k, 0xff00_ff00);
            }
            Flow::Done
        }
        2 => {
            next_step(x);
            grid(x);
            x.set_v16(var::LAYERS, super::ALL_LAYERS);
            // D0 vaut l'étape (2)
            x.c.d[0] = 2;
            x.call(ext::LAYERS_ON);
            Flow::Done
        }
        _ => exit_check(x),
    }
}

/// `0x29578` : `n` cases de valeur `v`, espacées de `stride` octets. Rend la case qui suit.
fn fill(x: &mut Ctx, mut cell: u32, v: u32, stride: u32, n: u32) -> u32 {
    for _ in 0..n {
        wr32(x.b, cell, v);
        cell += stride;
    }
    cell
}

/// `0x2950e` : la grille (tuiles 0x1891-0x1899 du plan 2).
fn grid(x: &mut Ctx) {
    let cell = fill(x, hatch::LAYER + 4, 0x1897_0001, 0x40, 23);
    wr32(x.b, cell, 0x1891_0001);
    fill(x, cell + 4, 0x1898_0041, 4, 13);
    let cell = fill(x, hatch::LAYER + 8, 0x1899_0001, 4, 12);
    fill(x, cell, 0x1899_0001, 0x40, 23);
    for column in 0..22 {
        fill(x, hatch::LAYER + 0x48 + 0x40 * column, 0x1899_0000, 4, 12);
    }
}

// ---------------------------------------------------------------------------------------------
// 6. GAME DATA (0x2b808)

mod data {
    pub const TITLE: u32 = 0x2b92a;
    pub const HELP: u32 = 0x2b94a;
    pub const COUNTERS: u32 = 0x2b984;
    pub const FREEPLAY: u32 = 0x2b9b0;
    pub const CARDS: u32 = 0x2baf0;
    /// Suite de l'écran avec la RAM de la carte de développement (temps de jeu, compteurs gardés en RAM).
    pub const DEV_BOARD: u32 = 0x2b842;
    /// Cases des trois compteurs (8 chiffres).
    pub const CELLS: [u32; 3] = [0x912fa4, 0x912fac, 0x912fb4];
    /// Compteurs de l'EEPROM (A5+) : pièces, services, parties gratuites / cartes.
    pub const COINS: u32 = 0x76;
    pub const SERVICES: u32 = 0x7a;
    pub const FREEPLAYS: u32 = 0x7e;
    pub const CARD_COUNT: u32 = 0x7fd4;
}

pub fn game_data(x: &mut Ctx) -> Flow {
    if get(x, f::STEP) != 0 {
        return exit_check(x);
    }
    next_step(x);
    layers_off(x);
    print(x, data::TITLE);
    print(x, data::HELP);
    if x.v8(var::DEV_BOARD) != 0 {
        // jamais sur une borne : le reste de l'écran en code d'origine
        x.call(data::DEV_BOARD);
        return Flow::Done;
    }
    print(x, data::COUNTERS);
    for (cell, counter) in [(data::CELLS[0], data::COINS), (data::CELLS[1], data::SERVICES)] {
        let v = x.rd32(crate::mem::A5 + counter);
        decimal(x, cell, v, 8, 0);
    }
    let (label, counter) = if matches!(x.v16(var::VARIANT), 4 | 10) {
        (data::CARDS, data::CARD_COUNT)
    } else {
        (data::FREEPLAY, data::FREEPLAYS)
    };
    print(x, label);
    let v = x.rd32(crate::mem::A5 + counter);
    decimal(x, data::CELLS[2], v, 8, 0);
    layers_on(x);
    Flow::Done
}
