//! Écrans de la machine à modes : démarrage, avertissements, images fixes de l'attract.
//!
//! Chaque sous-mode (`FF0E71`) est un écran ; on y entre à l'étape 0 (préparation) et l'on y
//! reste, une étape à la fois, jusqu'à ce que sa minuterie (`FF0E78`) atteigne zéro.

use super::{mode, sub, Flow, Sequence};
use crate::mem::{rd16, rd8, wr16, wr8};
use crate::resume;
use crate::scene::Ctx;
use crate::Native;

/// Variables de la RAM de travail (décalages depuis A5).
mod var {
    /// Bits 1 à 3 : plans de fond allumés (mot de commande vidéo).
    pub const LAYERS: u32 = 0x14;
    /// Fondu : pages, délai.
    pub const FADE_PAGES: u32 = 0x48e;
    pub const FADE_DELAY: u32 = 0x490;
    /// Défilement vertical du plan 2 ; abscisses des caméras (longs).
    pub const SCROLL_2: u32 = 0x4ba;
    pub const CAMERA: u32 = 0x506;
    pub const CAMERA_2: u32 = 0x50a;
}

/// Plans de fond (bits 1 à 3 du mot de commande vidéo).
const LAYER_BITS: u16 = 0xe;
/// Octet à FF0F53, posé par la tâche d'écran fixe quand l'écran est prêt.
const SCREEN_READY: u32 = 0xff_0f53;
/// Tâche d'écran fixe (exécuteur d'objets) et son emplacement.
const FIXED_SCREEN_TASK: u32 = 0x1835e;
const FIXED_SCREEN_SLOT: u32 = 0xf0;
const SETUP_SCREEN: u32 = 0x180ac;
/// Palette des écrans de texte et ses deux blocs de tuiles (écran « copyright »).
const TEXT_PALETTE: u32 = 0x53;
const TILE_PLANE_3: u32 = 0x917000;
const SOUND_SCREEN_A: u32 = 0xe0be2;
const SOUND_SCREEN_B: u32 = 0xe0c16;

/// Écran du sous-mode `sub`, s'il est réécrit.
pub fn screen(x: &mut Ctx, sub: u8) -> Option<Flow> {
    Some(match sub {
        0 | 6 => fixed_screen(x),
        8 => legal_screen(x),
        _ => return None,
    })
}

/// Sous-modes 0 et 6 (`0x1371e`) : crée la tâche d'écran fixe, attend qu'elle ait fini, puis fond.
fn fixed_screen(x: &mut Ctx) -> Flow {
    if rd8(x.b, mode::STEP) == 0 {
        wr8(x.b, mode::STEP, 2);
        wr8(x.b, mode::BOOTED, 1);
        x.call(SETUP_SCREEN);
        x.c.d[0] = FIXED_SCREEN_SLOT;
        x.c.a[0] = FIXED_SCREEN_TASK;
        x.call(sub::TASK_CREATE);
        return Flow::Done;
    }
    if rd8(x.b, SCREEN_READY) == 0 {
        return Flow::Done;
    }
    let s = rd8(x.b, mode::SUB).wrapping_add(2);
    wr8(x.b, mode::SUB, s);
    wr8(x.b, mode::STEP, 0);
    wr16(x.b, mode::TIMER, 1);
    x.set_v16(var::FADE_DELAY, 3);
    x.set_v8(var::FADE_PAGES, 0xf);
    x.tail(sub::FADE_OUT, FIXED_SCREEN_FADED)
}

const FIXED_SCREEN_FADED: u32 = 0x1376e;
resume!(FIXED_SCREEN_AFTER_FADE, FIXED_SCREEN_FADED, |x| {
    x.set_v32(var::SCROLL_2, 0);
    x.call(sub::CLEAR_SPRITES);
    x.call(sub::CLEAR_SCROLL_1);
    Flow::Done
});

/// Sous-mode 8 (`0x1377e`) : image à deux blocs de tuiles pendant 300 images, entre deux fondus.
fn legal_screen(x: &mut Ctx) -> Flow {
    if rd8(x.b, mode::STEP) != 0 {
        return legal_screen_wait(x);
    }
    x.and16(var::LAYERS, !LAYER_BITS);
    x.wait(4, LEGAL_SETUP)
}

const LEGAL_SETUP: u32 = 0x13794;
resume!(LEGAL_SCREEN_SETUP, LEGAL_SETUP, |x| {
    wr16(x.b, mode::TIMER, 300);
    let step = rd8(x.b, mode::STEP).wrapping_add(2);
    wr8(x.b, mode::STEP, step);
    x.c.d[0] = TEXT_PALETTE;
    x.call(sub::LOAD_PALETTE);
    x.call(sub::CLEAR_SCROLL_3);
    x.set_v32(var::CAMERA, 0);
    x.set_v32(var::CAMERA_2, 0);
    x.wait(1, LEGAL_TILES)
});

const LEGAL_TILES: u32 = 0x137bc;
resume!(LEGAL_SCREEN_TILES, LEGAL_TILES, |x| {
    x.c.d[0] = 0x1c0;
    x.c.a[1] = TILE_PLANE_3;
    x.call(sub::TILE_BLOCK);
    x.c.d[0] = 0x1c1;
    x.call(sub::TILE_BLOCK);
    x.or16(var::LAYERS, LAYER_BITS);
    x.wait(1, LEGAL_SOUND)
});

const LEGAL_SOUND: u32 = 0x137e8;
resume!(LEGAL_SCREEN_SOUND, LEGAL_SOUND, |x| {
    x.call(SOUND_SCREEN_A);
    x.call(SOUND_SCREEN_B);
    x.set_v8(var::FADE_PAGES, 0xf);
    x.jump_to(sub::FADE_IN)
});

/// Étape d'attente du sous-mode 8 : à la fin de la minuterie, fondu au noir et écran suivant.
fn legal_screen_wait(x: &mut Ctx) -> Flow {
    let left = rd16(x.b, mode::TIMER).wrapping_sub(1);
    wr16(x.b, mode::TIMER, left);
    if left != 0 {
        return Flow::Done;
    }
    let s = rd8(x.b, mode::SUB).wrapping_add(2);
    wr8(x.b, mode::SUB, s);
    wr8(x.b, mode::STEP, 0);
    x.set_v8(var::FADE_PAGES, 0xf);
    x.tail(sub::FADE_OUT, LEGAL_FADED)
}

const LEGAL_FADED: u32 = 0x1381c;
resume!(LEGAL_SCREEN_FADED, LEGAL_FADED, |x| {
    x.and16(var::LAYERS, !LAYER_BITS);
    x.wait(1, LEGAL_CLEARED)
});

const LEGAL_CLEARED: u32 = 0x1382a;
resume!(LEGAL_SCREEN_CLEARED, LEGAL_CLEARED, |x| {
    x.call(sub::CLEAR_SCROLL_2);
    x.or16(var::LAYERS, LAYER_BITS);
    x.wait(1, LEGAL_PALETTE)
});

const LEGAL_PALETTE: u32 = 0x1383e;
resume!(LEGAL_SCREEN_PALETTE, LEGAL_PALETTE, |x| {
    wr16(x.b, mode::TIMER, 300);
    x.c.d[0] = 0;
    x.call(sub::LOAD_PALETTE);
    Flow::Done
});

pub const NATIVES: &[Native] = &[
    FIXED_SCREEN_AFTER_FADE,
    LEGAL_SCREEN_SETUP,
    LEGAL_SCREEN_TILES,
    LEGAL_SCREEN_SOUND,
    LEGAL_SCREEN_FADED,
    LEGAL_SCREEN_CLEARED,
    LEGAL_SCREEN_PALETTE,
];
