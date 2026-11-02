//! Écrans 4. COLOR, 5. DOT CROSS HATCH et 6. GAME DATA du menu TEST : deux étapes de préparation (ou une),
//! puis l'attente du retour au menu (départ 1P + tir 1).

use super::{decimal, exit_check, f, get, layers_off, layers_on, next_step, print, var};
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
            crate::video::desactiver_couches(x.b, 0);
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
            crate::video::activer_couches(x.b, 2);
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
    /// Texte du temps de jeu (carte de développement).
    pub const TIME: u32 = 0x2b96c;
    /// Première case du temps de jeu (heures, minutes, secondes, un chiffre par ligne).
    pub const TIME_CELL: u32 = 0x912f9c;
    /// Temps de jeu (A5 - 0x615c) et son indicateur (A5 - 0x6158) : sans indicateur, 00:00:00.
    pub const TIME_VALUE: u32 = 0xff_1ea4;
    pub const TIME_FLAG: u32 = 0xff_1ea8;
    /// Secondes puis minutes (mots, A5 - 0x62a0 et - 0x629e).
    pub const SECONDS: u32 = 0xff_1d60;
    /// Cases des trois compteurs (8 chiffres).
    pub const CELLS: [u32; 3] = [0x912fa4, 0x912fac, 0x912fb4];
    /// Compteurs : pièces, services, parties gratuites, cartes. En EEPROM (A5 + 0x76, 0x7a, 0x7e, 0x7fd4) ;
    /// sur la carte de développement, copies en RAM supplémentaire (A5 - 0x6164, - 0x6160, - 0x6154, - 0x6150).
    pub const EEPROM_COUNTERS: [u32; 4] = [crate::mem::A5 + 0x76, crate::mem::A5 + 0x7a, crate::mem::A5 + 0x7e, crate::mem::A5 + 0x7fd4];
    pub const DEV_COUNTERS: [u32; 4] = [0xff_1e9c, 0xff_1ea0, 0xff_1eac, 0xff_1eb0];
}

/// `0x2b86e`, `0x29ca0`, `0x29cc4` : le temps de jeu (compteur divisé par l'indicateur, qui est la fréquence
/// du compteur) en `hh:mm:ss` ; divisions signées ; heures plafonnées à 99 (minutes alors 0x3b3b, secondes 0).
fn play_time(x: &mut Ctx) {
    let value = x.rd32(data::TIME_VALUE) as i32;
    let rate = x.rd32(data::TIME_FLAG) as i32;
    let (hours, seconds, minutes);
    if rate == 0 {
        (hours, seconds, minutes) = (0, 0, 0);
    } else {
        let divide = |v: i32, d: i32| {
            let q = (v.unsigned_abs() / d.unsigned_abs()) as i32;
            (if (v < 0) != (d < 0) { -q } else { q }, v.unsigned_abs() % d.unsigned_abs())
        };
        let (q, _) = divide(value, rate);
        let (q, s) = divide(q, 60);
        let (q, m) = divide(q, 60);
        (hours, seconds, minutes) = if q as u16 > 0x63 { (0x63, 0, 0x3b3b) } else { (q as u32, s, m) };
    }
    wr32(x.b, data::SECONDS, seconds << 16 | minutes);
    let mut cell = data::TIME_CELL;
    for (k, v) in [hours, minutes, seconds].into_iter().enumerate() {
        cell = decimal(x, cell, v, 2, 0);
        if k < 2 {
            wr32(x.b, cell, 0x003a_0000);
            cell += 0x80;
        }
    }
}

pub fn game_data(x: &mut Ctx) -> Flow {
    if get(x, f::STEP) != 0 {
        return exit_check(x);
    }
    next_step(x);
    layers_off(x);
    print(x, data::TITLE);
    print(x, data::HELP);
    // carte de développement (jamais sur une borne) : temps de jeu, compteurs gardés en RAM
    let dev = x.v8(var::DEV_BOARD) != 0;
    let counters = if dev { data::DEV_COUNTERS } else { data::EEPROM_COUNTERS };
    if dev {
        print(x, data::TIME);
        play_time(x);
    }
    print(x, data::COUNTERS);
    for (cell, counter) in [(data::CELLS[0], counters[0]), (data::CELLS[1], counters[1])] {
        let v = x.rd32(counter);
        decimal(x, cell, v, 8, 0);
    }
    let (label, counter) = if matches!(x.v16(var::VARIANT), 4 | 10) {
        (data::CARDS, counters[3])
    } else {
        (data::FREEPLAY, counters[2])
    };
    print(x, label);
    let v = x.rd32(counter);
    decimal(x, data::CELLS[2], v, 8, 0);
    layers_on(x);
    Flow::Done
}
