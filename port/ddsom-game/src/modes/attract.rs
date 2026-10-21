//! Écrans de la machine à modes : démarrage, avertissements, images fixes de l'attract.
//!
//! Chaque sous-mode (`FF0E71`) est un écran ; on y entre à l'étape 0 (préparation) et l'on y
//! reste, une étape à la fois, jusqu'à ce que sa minuterie (`FF0E78`) atteigne zéro.

use super::{mode, sub, Flow, Sequence};
use crate::mem::{rd16, rd8, wr16, wr8, A5};
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
/// Arrête (et lance) la tâche du texte « INSERT COIN » clignotant (`taches/invite.rs`).
const INVITE_STOP: u32 = 0xe03c6;

/// Écran du sous-mode `sub`, s'il est réécrit.
pub fn screen(x: &mut Ctx, sub: u8) -> Option<Flow> {
    Some(match sub {
        0 | 6 => fixed_screen(x),
        8 => legal_screen(x),
        16 => second_legal_screen(x),
        18 => notice_screen(x),
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

// ---------------------------------------------------------------------------------------------
// Sous-mode 16 (`0x1384c`) : seconde image de droits, seulement si `A5+0x70` vaut 2

fn second_legal_screen(x: &mut Ctx) -> Flow {
    if rd16(x.b, A5 + 0x70) != 2 {
        wr8(x.b, mode::SUB, 0);
        wr8(x.b, mode::STEP, 0);
        return Flow::Done;
    }
    if rd8(x.b, mode::STEP) != 0 {
        return second_legal_wait(x);
    }
    x.and16(var::LAYERS, !LAYER_BITS);
    x.wait(4, SECOND_SETUP)
}
const SECOND_SETUP: u32 = 0x13876;
resume!(R_SECOND_SETUP, SECOND_SETUP, |x| {
    wr16(x.b, mode::TIMER, 300);
    let step = rd8(x.b, mode::STEP).wrapping_add(2);
    wr8(x.b, mode::STEP, step);
    x.c.d[0] = TEXT_PALETTE;
    x.call(sub::LOAD_PALETTE);
    x.call(sub::CLEAR_SCROLL_3);
    x.set_v32(var::CAMERA, 0);
    x.set_v32(var::CAMERA_2, 0);
    x.call(INVITE_STOP);
    x.wait(1, SECOND_TILES)
});
const SECOND_TILES: u32 = 0x1389e;
resume!(R_SECOND_TILES, SECOND_TILES, |x| {
    x.c.d[0] = 0x1c2;
    x.c.a[1] = TILE_PLANE_3;
    x.call(sub::TILE_BLOCK);
    x.c.d[0] = 0x1c3;
    x.call(sub::TILE_BLOCK);
    x.or16(var::LAYERS, LAYER_BITS);
    x.wait(1, SECOND_FADE_IN)
});
const SECOND_FADE_IN: u32 = 0x138d0;
resume!(R_SECOND_FADE_IN, SECOND_FADE_IN, |x| {
    x.set_v8(var::FADE_PAGES, 0xf);
    x.jump_to(sub::FADE_IN)
});
fn second_legal_wait(x: &mut Ctx) -> Flow {
    let left = rd16(x.b, mode::TIMER).wrapping_sub(1);
    wr16(x.b, mode::TIMER, left);
    if left != 0 {
        return Flow::Done;
    }
    let s = rd8(x.b, mode::SUB).wrapping_add(2);
    wr8(x.b, mode::SUB, s);
    wr8(x.b, mode::STEP, 0);
    x.set_v8(var::FADE_PAGES, 0xf);
    x.tail(sub::FADE_OUT, SECOND_FADED)
}
const SECOND_FADED: u32 = 0x138f8;
resume!(R_SECOND_FADED, SECOND_FADED, |x| {
    x.and16(var::LAYERS, !LAYER_BITS);
    x.wait(1, SECOND_CLEARED)
});
const SECOND_CLEARED: u32 = 0x13906;
resume!(R_SECOND_CLEARED, SECOND_CLEARED, |x| {
    x.call(sub::CLEAR_SCROLL_2);
    x.or16(var::LAYERS, LAYER_BITS);
    x.wait(1, SECOND_PALETTE)
});
const SECOND_PALETTE: u32 = 0x1391a;
resume!(R_SECOND_PALETTE, SECOND_PALETTE, |x| {
    x.c.d[0] = 0;
    x.jump_to(sub::LOAD_PALETTE)
});

// ---------------------------------------------------------------------------------------------
// Sous-mode 18 (`0x13922`) : mention « violence animée », 120 images

/// Texte de la mention (dans le code d'origine).
const NOTICE_TEXT: u32 = 0x13b52;

fn notice_screen(x: &mut Ctx) -> Flow {
    if rd8(x.b, mode::STEP) != 0 {
        return notice_countdown(x);
    }
    wr16(x.b, mode::TIMER, 120);
    let step = rd8(x.b, mode::STEP).wrapping_add(2);
    wr8(x.b, mode::STEP, step);
    x.and16(var::LAYERS, !LAYER_BITS);
    x.wait(4, NOTICE_TEXT_SHOWN)
}
const NOTICE_TEXT_SHOWN: u32 = 0x13942;
resume!(R_NOTICE_TEXT_SHOWN, NOTICE_TEXT_SHOWN, |x| {
    x.c.a[4] = NOTICE_TEXT;
    x.call(0xd5e);
    x.or16(var::LAYERS, LAYER_BITS);
    x.wait(1, NOTICE_FADE_IN)
});
const NOTICE_FADE_IN: u32 = 0x1395a;
resume!(R_NOTICE_FADE_IN, NOTICE_FADE_IN, |x| {
    x.set_v8(var::FADE_PAGES, 0xf);
    // `jsr 0x144c` : la suite est le décompte, juste après
    x.tail(sub::FADE_IN, NOTICE_COUNTDOWN)
});
const NOTICE_COUNTDOWN: u32 = 0x13966;
resume!(R_NOTICE_COUNTDOWN, NOTICE_COUNTDOWN, notice_countdown);
fn notice_countdown(x: &mut Ctx) -> Flow {
    let left = rd16(x.b, mode::TIMER).wrapping_sub(1);
    wr16(x.b, mode::TIMER, left);
    if left != 0 {
        return Flow::Done;
    }
    wr16(x.b, mode::TIMER, 300);
    wr8(x.b, mode::SUB, 2);
    wr8(x.b, mode::STEP, 0);
    x.set_v8(var::FADE_PAGES, 0xf);
    x.tail(sub::FADE_OUT, NOTICE_FADED)
}
const NOTICE_FADED: u32 = 0x1398a;
resume!(R_NOTICE_FADED, NOTICE_FADED, |x| {
    x.and16(var::LAYERS, !LAYER_BITS);
    x.wait(1, NOTICE_CLEARED)
});
const NOTICE_CLEARED: u32 = 0x13998;
resume!(R_NOTICE_CLEARED, NOTICE_CLEARED, |x| {
    x.call(sub::CLEAR_SCROLL_1);
    x.or16(var::LAYERS, LAYER_BITS);
    x.c.d[0] = 1;
    x.jump_to(sub::WAIT)
});


pub const NATIVES: &[Native] = &[
    FIXED_SCREEN_AFTER_FADE,
    LEGAL_SCREEN_SETUP,
    LEGAL_SCREEN_TILES,
    LEGAL_SCREEN_SOUND,
    LEGAL_SCREEN_FADED,
    LEGAL_SCREEN_CLEARED,
    LEGAL_SCREEN_PALETTE,
    R_SECOND_SETUP,
    R_SECOND_TILES,
    R_SECOND_FADE_IN,
    R_SECOND_FADED,
    R_SECOND_CLEARED,
    R_SECOND_PALETTE,
    R_NOTICE_TEXT_SHOWN,
    R_NOTICE_FADE_IN,
    R_NOTICE_COUNTDOWN,
    R_NOTICE_FADED,
    R_NOTICE_CLEARED,
];
