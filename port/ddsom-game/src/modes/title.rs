//! Écrans de titre : « shuukyaku » (sous-mode 2, `0xfa296`) et second titre (sous-mode 12, `0xfaef2`).
//!
//! Chaque écran est une suite d'étapes (`FF0E72`, pas de 2) exécutées une par tour de la machine à modes ;
//! une étape prépare des objets système (variantes de SOBJ, SET), attend qu'ils aient fini (champs du
//! bloc d'état que les objets modifient par le pointeur `+0x4c` de l'objet) et passe à la suivante.
//! Les attentes dans une étape (pool d'objets plein, fondu en cours) cèdent une image à
//! l'ordonnanceur et reprennent à l'adresse d'origine (`resume!`).

use super::{mode, sub, Block, Flow, Sequence};
use crate::mem::{rd16, rd8, rd32, set_low_word, wr16, wr32, wr8};
use crate::resume;
use crate::scene::Ctx;
use crate::Native;

/// Champs du bloc d'état (décalages depuis `mode::BASE`).
mod f {
    pub const STEP: u32 = 2;
    /// Octet lu par les objets du titre (variante d'animation à jouer).
    pub const CUE: u32 = 8;
    /// Objet SOBJ de la scène (adresse sur 16 bits, étendue du signe).
    pub const OBJECT: u32 = 0xa;
    /// Drapeau d'objet terminé (octet) ou vitesse de défilement (mot, avec `ACCEL`).
    pub const DONE: u32 = 0xc;
    pub const CUE_INDEX: u32 = 0xc;
    pub const SPEED: u32 = 0xc;
    pub const ACCEL: u32 = 0xe;
    /// Minuterie (octet ou mot selon l'étape).
    pub const TIMER: u32 = 0x10;
}

/// Variables de la RAM de travail (décalages depuis A5).
mod var {
    pub const VIDEO_FLAGS: u32 = 0x12;
    pub const LAYERS: u32 = 0x14;
    pub const REGION: u32 = 0x70;
    pub const SCREEN_TYPE: u32 = 0x36;
    pub const FADE_PAGES: u32 = 0x48e;
    pub const FADE_STEPS: u32 = 0x48f;
    pub const FADE_DELAY: u32 = 0x490;
    pub const SCROLL_1_X: u32 = 0x4b6;
    pub const SCROLL_2_Y: u32 = 0x4ba;
    pub const CAMERA_X: u32 = 0x506;
    pub const CAMERA_Y: u32 = 0x50a;
    pub const CAMERA_2_X: u32 = 0x556;
    pub const CAMERA_2_Y: u32 = 0x55a;
    pub const CPS_COPY: u32 = 0x534;
    pub const GAUGE_SKIP: u32 = 0x7fb2;
}

/// Sous-routines d'origine.
mod ext {
    pub const SOUND_RESET: u32 = 0xc0c;
    pub const POOLS_SOBJ: u32 = 0x21bc;
    pub const POOLS_SET: u32 = 0x2134;
    pub const POOLS_EFFCT: u32 = 0x2150;
    pub const RESET_PLAYERS: u32 = 0x1f184;
    pub const SPRITES_RESET: u32 = 0x1de7c;
    pub const ALLOC_SET: u32 = 0x2348;
    pub const ALLOC_SOBJ: u32 = 0x2408;
    /// Objet SOBJ de la scène de titre, de variante D0 (retenue = échec) ; SET de type 0xf.
    pub const SOBJ_VARIANT: u32 = 0xddeaa;
    pub const SET_VARIANT: u32 = 0xce00a;
    pub const SOUND_E0EBC: u32 = 0xe0ebc;
    /// Arrête (et lance) la tâche du texte « INSERT COIN » clignotant (`taches/invite.rs`).
    pub const INVITE_STOP: u32 = 0xe03c6;
    pub const INVITE_START: u32 = 0xe03e4;
    pub const SOUND_E185C: u32 = 0xe185c;
    pub const PALETTE_SET: u32 = 0x1a67e;
    pub const PALETTE_FADE: u32 = 0x1a8b2;
    pub const PALETTE_PAGE_A: u32 = 0x1a60a;
    pub const PALETTE_PAGE_B: u32 = 0x1a608;
    pub const PALETTE_PAGE_C: u32 = 0x1a3d4;
    pub const BRIGHTNESS_WHITE: u32 = 0x1686;
    pub const TILE_BLOCK: u32 = 0x26d4;
    pub const TILE_BLOCK_2: u32 = 0x2766;
    pub const CLEAR_BLOCK: u32 = 0x260c;
    pub const PRINT: u32 = 0xd5e;
    pub const TASK_CREATE: u32 = 0x9b6;
    /// Tâches de fondu : vers la couleur, depuis la couleur.
    pub const FADE_TASK_IN: u32 = 0x14ba;
    pub const FADE_TASK_OUT: u32 = 0x14d0;
    /// Fin du texte de titre : continue dans l'original (objet SOBJ qui passe à la suite).
    pub const OBJECT_LAUNCH: u32 = 0xea288;
}

const LAYER_BITS: u16 = 0xe;
const FADE_PAGES_ALL: u8 = 0xf;
/// Palette des écrans de titre.
const TITLE_PALETTE: u32 = 0x53;
const GFX_PLANE_1: u32 = 0x914000;
const GFX_PLANE_2: u32 = 0x917000;
const GFX_PLANE_1_START: u32 = 0x912000;
const GFX_TABLE_B: u32 = 0x91b800;
const CPS_REG_VIDEO: u32 = 0x804122;
const CPS_REG_SCROLL: u32 = 0x804108;
const SPRITE_CLEAR_AREA: u32 = 0x920000;

/// Écran du sous-mode `sub`, s'il est réécrit ici.
pub fn screen(x: &mut Ctx, sub: u8) -> Option<Flow> {
    let step = x.m8(f::STEP);
    Some(match sub {
        2 => title_step(x, step),
        0xc => second_title_step(x, step),
        _ => return None,
    })
}

fn unknown_step(x: &mut Ctx, entry: u32) -> Flow {
    x.jump(entry)
}

// ---------------------------------------------------------------------------------------------
// Aides communes

/// Libère les pools d'objets et remet les sprites à zéro (début et fin d'écran).
fn reset_objects(x: &mut Ctx) {
    let a0 = x.c.a[0];
    for f in [ext::POOLS_SOBJ, ext::POOLS_SET, ext::POOLS_EFFCT, ext::RESET_PLAYERS, ext::SPRITES_RESET] {
        x.call(f);
    }
    x.c.a[0] = a0;
}

/// Crée la tâche de fondu `entry` (emplacement 0x10) sans toucher à A0.
fn start_fade_task(x: &mut Ctx, entry: u32) {
    let a0 = x.c.a[0];
    x.c.a[0] = entry;
    x.call_d0(ext::TASK_CREATE, 0x10);
    x.c.a[0] = a0;
}

/// Réglages de fondu communs (délai, pages).
fn fade_setup(x: &mut Ctx, delay: u16, pages: u8) {
    x.set_v16(var::FADE_DELAY, delay);
    x.set_v8(var::FADE_PAGES, pages);
}

/// Charge la palette de titre sur toutes les pages (début d'écran).
fn load_title_palette(x: &mut Ctx) {
    x.call_d0(ext::PALETTE_SET, TITLE_PALETTE);
    x.call_d0(ext::PALETTE_FADE, TITLE_PALETTE);
}

/// Copie deux blocs de tuiles de plan 1 puis deux de plan 2 (images du titre).
fn load_title_tiles(x: &mut Ctx, a: u16, b: u16) {
    x.c.a[1] = GFX_PLANE_1;
    set_low_word(&mut x.c.d[0], a);
    x.call(ext::TILE_BLOCK);
    set_low_word(&mut x.c.d[0], a + 1);
    x.call(ext::TILE_BLOCK);
    set_low_word(&mut x.c.d[0], b);
    x.c.a[1] = GFX_TABLE_B;
    x.call(ext::TILE_BLOCK_2);
    set_low_word(&mut x.c.d[0], b + 1);
    x.call(ext::TILE_BLOCK_2);
}

// ---------------------------------------------------------------------------------------------
// Sous-mode 2 : titre

fn title_step(x: &mut Ctx, step: u8) -> Flow {
    match step {
        0x00 => t00_reset(x),
        0x02 => t02_cue(x),
        0x04 => t04_fade_out(x),
        0x06 => t06_start(x),
        0x08 => t08_wait_flash(x),
        0x0a => t0a_wait(x),
        0x0c => t0c_fade_task(x),
        0x0e => t0e_wait_fade(x),
        0x10 => t10_start(x),
        0x12 => t12_wait(x),
        0x14 => t14_wait(x),
        0x16 => t16_slide(x),
        0x18 => t18_palette(x),
        0x1a => t1a_wait(x),
        0x1c => t1c_wait_logo(x),
        0x1e => t1e_end(x),
        _ => unknown_step(x, 0xfa296),
    }
}

/// Étape 0 : remet tout à zéro, charge la palette et les images, affiche le fond.
fn t00_reset(x: &mut Ctx) -> Flow {
    reset_objects(x);
    x.call(ext::SOUND_RESET);
    x.set_v16(var::LAYERS, 0x12c0);
    x.wait(4, T00_LOAD)
}
const T00_LOAD: u32 = 0xfa2fe;
resume!(R_T00_LOAD, T00_LOAD, |x| {
    x.c.d[0] = 0;
    x.set_v16(var::CAMERA_Y, 0x300);
    for off in [var::CAMERA_X, var::SCROLL_1_X, var::SCROLL_2_Y, var::CAMERA_2_X, var::CAMERA_2_Y] {
        x.set_v16(off, 0);
    }
    load_title_palette(x);
    x.call_d0(ext::PALETTE_PAGE_A, 2);
    x.call_d0(ext::PALETTE_PAGE_C, 1);
    load_title_tiles(x, 138, 403);
    set_low_word(&mut x.c.d[3], 0x300);
    x.render();
    x.or16(var::LAYERS, LAYER_BITS);
    x.wait(1, T00_FADE)
});
const T00_FADE: u32 = 0xfa384;
resume!(R_T00_FADE, T00_FADE, |x| {
    x.set_m8(f::TIMER, 30);
    x.set_m8(f::CUE_INDEX, 0);
    x.set_m8(0xd, 6);
    x.set_m8(f::CUE, 0);
    x.next_step();
    x.call(ext::SOUND_E0EBC);
    fade_setup(x, 3, FADE_PAGES_ALL);
    x.jump_to(sub::FADE_IN)
});

/// Étape 2 : lance les objets du titre l'un après l'autre, à intervalles donnés par une table.
fn t02_cue(x: &mut Ctx) -> Flow {
    x.render();
    let left = x.m8(f::TIMER).wrapping_sub(1);
    x.set_m8(f::TIMER, left);
    if left != 0 {
        return Flow::Done;
    }
    x.call_d0(ext::SOBJ_VARIANT, 6);
    if !x.ok() {
        x.next_step();
        return Flow::Done;
    }
    let obj = x.c.a[1];
    let index = x.m8(f::CUE_INDEX);
    wr8(x.b, obj + 0x2f, index);
    wr32(x.b, obj + 0x4c, mode::BASE);
    x.set_m8(f::CUE_INDEX, index.wrapping_add(1));
    let delay = x.prog8(T02_DELAYS + index as u32);
    x.set_m8(f::TIMER, delay);
    if delay & 0x80 == 0 {
        return Flow::Done;
    }
    x.next_step();
    Flow::Done
}
/// Délais (images) entre les apparitions, dans le code d'origine ; 0xff termine.
const T02_DELAYS: u32 = 0xfa42e;

/// Étape 4 : attend la fin des objets (`+0xd` nul), puis fondu vers le noir.
fn t04_fade_out(x: &mut Ctx) -> Flow {
    x.render();
    if x.m8(0xd) != 0 {
        return Flow::Done;
    }
    x.next_step();
    fade_setup(x, 3, FADE_PAGES_ALL);
    x.jump_to(sub::FADE_OUT)
}

/// Étape 6 : fond d'écran suivant ; crée l'objet de variante 7 (attend si le pool est plein).
fn t06_start(x: &mut Ctx) -> Flow {
    x.set_v16(var::CAMERA_Y, 0);
    x.set_m8(f::CUE, 1);
    x.render();
    x.and16(var::LAYERS, !LAYER_BITS);
    x.wait(4, T06_OBJECT)
}
const T06_OBJECT: u32 = 0xfa472;
resume!(R_T06_OBJECT, T06_OBJECT, t06_object);
fn t06_object(x: &mut Ctx) -> Flow {
    x.call_d0(ext::SOBJ_VARIANT, 7);
    if !x.ok() {
        return x.wait(1, T06_RETRY);
    }
    x.set_m8(f::DONE, 1);
    wr32(x.b, x.c.a[1] + 0x4c, mode::BASE);
    wr8(x.b, x.c.a[1] + 0x2f, 0);
    x.next_step();
    Flow::Done
}
const T06_RETRY: u32 = 0xfa486;
resume!(R_T06_RETRY, T06_RETRY, t06_object);

/// Étape 8 : attend la fin de l'objet, éclair blanc.
fn t08_wait_flash(x: &mut Ctx) -> Flow {
    x.render();
    if x.m8(f::DONE) != 0 {
        return Flow::Done;
    }
    x.set_v8(var::FADE_PAGES, FADE_PAGES_ALL);
    x.call(ext::BRIGHTNESS_WHITE);
    x.wait(4, T08_NEXT)
}
const T08_NEXT: u32 = 0xfa4bc;
resume!(R_T08_NEXT, T08_NEXT, |x| {
    x.next_step();
    x.set_v16(var::LAYERS, 0x12ce);
    x.set_v16(var::CAMERA_Y, 0x300);
    x.set_m16(f::TIMER, 60);
    x.render();
    Flow::Done
});

/// Étape 10 : après 60 images, crée l'objet SET de variante 1 (attend si le pool est plein).
fn t0a_wait(x: &mut Ctx) -> Flow {
    x.render();
    let left = x.m16(f::TIMER).wrapping_sub(1);
    x.set_m16(f::TIMER, left);
    if left != 0 {
        return Flow::Done;
    }
    t0a_object(x)
}
fn t0a_object(x: &mut Ctx) -> Flow {
    x.set_m8(f::DONE, 1);
    x.call_d0(ext::SET_VARIANT, 1);
    if !x.ok() {
        return x.wait(1, T0A_RETRY);
    }
    wr32(x.b, x.c.a[1] + 0x4c, mode::BASE);
    x.next_step();
    Flow::Done
}
const T0A_RETRY: u32 = 0xfa4fc;
resume!(R_T0A_RETRY, T0A_RETRY, |x| {
    // `bra 0xfa4e8` : retente sans reposer le drapeau
    x.call_d0(ext::SET_VARIANT, 1);
    if !x.ok() {
        return x.wait(1, T0A_RETRY);
    }
    wr32(x.b, x.c.a[1] + 0x4c, mode::BASE);
    x.next_step();
    Flow::Done
});

/// Étape 12 : attend l'objet, puis lance le fondu au noir (tâche).
fn t0c_fade_task(x: &mut Ctx) -> Flow {
    x.render();
    if x.m8(f::DONE) != 0 {
        return Flow::Done;
    }
    x.next_step();
    fade_setup(x, 3, FADE_PAGES_ALL);
    start_fade_task(x, ext::FADE_TASK_OUT);
    Flow::Done
}

/// Étape 14 : attend la fin du fondu (`A5+0x48f` nul).
fn t0e_wait_fade(x: &mut Ctx) -> Flow {
    x.render();
    if x.v8(var::FADE_STEPS) != 0 {
        return Flow::Done;
    }
    x.next_step();
    Flow::Done
}

/// Étape 16 : deuxième partie du titre : caméra à 512, objet de variante 7 (attend si plein).
fn t10_start(x: &mut Ctx) -> Flow {
    x.set_v16(var::CAMERA_X, 0x200);
    x.set_m8(f::CUE, 0xff);
    x.render();
    x.and16(var::LAYERS, !LAYER_BITS);
    x.wait(4, T10_OBJECT)
}
const T10_OBJECT: u32 = 0xfa56e;
resume!(R_T10_OBJECT, T10_OBJECT, t10_object);
fn t10_object(x: &mut Ctx) -> Flow {
    x.call_d0(ext::SOBJ_VARIANT, 7);
    if !x.ok() {
        return x.wait(1, T10_RETRY);
    }
    x.set_m8(f::DONE, 1);
    wr32(x.b, x.c.a[1] + 0x4c, mode::BASE);
    wr8(x.b, x.c.a[1] + 0x2f, 1);
    x.next_step();
    Flow::Done
}
const T10_RETRY: u32 = 0xfa582;
resume!(R_T10_RETRY, T10_RETRY, t10_object);

/// Étape 18 : éclair blanc, puis charge le décor du bas (quatre bandes de deux blocs de tuiles).
fn t12_wait(x: &mut Ctx) -> Flow {
    x.render();
    if x.m8(f::DONE) != 0 {
        return Flow::Done;
    }
    x.set_v8(var::FADE_PAGES, FADE_PAGES_ALL);
    x.call(ext::BRIGHTNESS_WHITE);
    x.wait(4, T12_SCENERY)
}
const T12_SCENERY: u32 = 0xfa5ba;
resume!(R_T12_SCENERY, T12_SCENERY, |x| {
    set_low_word(&mut x.c.d[3], 0x70);
    x.set_m8(f::CUE, 2);
    x.render();
    x.set_v16(var::LAYERS, 0x12ce);
    x.set_m16(f::TIMER, 60);
    x.set_v16(var::CAMERA_Y, 0x70);
    x.set_v16(var::CAMERA_X, 0);
    x.set_v16(var::SCROLL_1_X, 0);
    x.set_v16(var::SCROLL_2_Y, 0);
    let mut tile = 480u16;
    x.c.a[1] = GFX_PLANE_1;
    for _ in 0..4 {
        set_low_word(&mut x.c.d[0], tile);
        x.call(ext::TILE_BLOCK);
        set_low_word(&mut x.c.d[0], tile + 1);
        x.call(ext::TILE_BLOCK);
        tile += 2;
        x.c.a[1] += 0x800;
    }
    set_low_word(&mut x.c.d[2], tile);
    x.c.d[1] = 0xffff;
    x.render();
    x.set_v16(var::LAYERS, 0x09ce);
    x.wait(1, T12_FADE)
});
const T12_FADE: u32 = 0xfa620;
resume!(R_T12_FADE, T12_FADE, |x| {
    fade_setup(x, 3, FADE_PAGES_ALL);
    start_fade_task(x, ext::FADE_TASK_IN);
    x.next_step();
    x.set_m16(f::SPEED, 0xb00);
    x.set_m16(f::ACCEL, 0xffe0);
    x.set_m16(f::TIMER, 60);
    x.call_d0(ext::SET_VARIANT, 0);
    let link = x.c.a[1] as u16;
    x.set_m16(f::OBJECT, link);
    Flow::Done
});

/// Étape 20 : attend 60 images, puis lance l'animation 3.
fn t14_wait(x: &mut Ctx) -> Flow {
    x.render();
    let left = x.m8(f::TIMER).wrapping_sub(1);
    x.set_m8(f::TIMER, left);
    if left != 0 {
        return Flow::Done;
    }
    x.set_m8(f::CUE, 3);
    x.next_step();
    Flow::Done
}

/// Étape 22 : la caméra glisse vers le bas (vitesse croissante jusqu'à 0x400) puis s'arrête à 0x300.
fn t16_slide(x: &mut Ctx) -> Flow {
    x.render();
    let mut speed = x.m16(f::SPEED);
    if speed != 0x400 {
        speed = speed.wrapping_add(x.m16(f::ACCEL));
        x.set_m16(f::SPEED, speed);
    }
    let delta = (((speed as u32) << 16) as i32) >> 8;
    for off in [var::CAMERA_Y, var::CAMERA_2_Y] {
        let v = rd32(x.b, crate::mem::A5 + off).wrapping_add(delta as u32);
        wr32(x.b, crate::mem::A5 + off, v);
    }
    if x.v16(var::CAMERA_Y) < 0x300 {
        return Flow::Done;
    }
    x.set_v16(var::CAMERA_Y, 0x300);
    x.next_step();
    x.set_m16(f::TIMER, 30);
    Flow::Done
}

/// Étape 24 : copie la palette du logo (quatre blocs de 32 octets), crée l'objet du logo et
/// attend qu'il ait disparu.
fn t18_palette(x: &mut Ctx) -> Flow {
    let left = x.m16(f::TIMER).wrapping_sub(1);
    x.set_m16(f::TIMER, left);
    if left != 0 {
        return Flow::Done;
    }
    let saved = (x.c.d, x.c.a[2]);
    // quatre blocs de 32 octets de la ROM vers la table des palettes
    let mut src = LOGO_PALETTE;
    for dst in [0x91c2e0u32, 0x91c300, 0x91c360, 0x91c3e0] {
        for i in 0..8 {
            let v = rd32(x.b, src + 4 * i);
            wr32(x.b, dst + 4 * i, v);
        }
        src += 32;
    }
    x.c.a[4] = src;
    x.c.a[1] = 0x91c3e0;
    x.set_v8(var::FADE_PAGES, 1);
    x.call(ext::BRIGHTNESS_WHITE);
    x.c.d[..7].copy_from_slice(&saved.0[..7]);
    x.c.a[2] = saved.1;
    t18_object(x)
}
fn t18_object(x: &mut Ctx) -> Flow {
    x.call(ext::ALLOC_SET);
    if !x.ok() {
        return x.wait(1, T18_RETRY);
    }
    let o = x.c.a[1];
    wr8(x.b, o, 1);
    wr8(x.b, o + 2, 4);
    wr8(x.b, o + 3, 0x34);
    x.set_m16(f::DONE, o as u16);
    x.set_m16(f::TIMER, 80);
    x.wait(1, T18_LOOP)
}
/// Palette du logo : quatre blocs consécutifs de 32 octets dans la ROM.
const LOGO_PALETTE: u32 = 0x2538c0;
const T18_RETRY: u32 = 0xfa722;
resume!(R_T18_RETRY, T18_RETRY, t18_object);
const T18_LOOP: u32 = 0xfa746;
resume!(R_T18_LOOP, T18_LOOP, |x| {
    x.push_return(T18_AFTER);
    t18_sub(x)
});
const T18_AFTER: u32 = 0xfa74a;
resume!(R_T18_AFTER, T18_AFTER, |x| {
    x.render();
    let link = x.m16(f::DONE);
    let o = link as i16 as i32 as u32 & 0xff_ffff;
    if rd8(x.b, o) != 0 {
        return x.wait(1, T18_LOOP);
    }
    x.next_step();
    x.set_m16(f::TIMER, 60);
    Flow::Done
});
/// Sous-routine `0xfa762` (appelée par `bsr`, revient en `T18_AFTER`) : après 80 images, crée l'objet
/// du texte et lance le script de l'original.
fn t18_sub(x: &mut Ctx) -> Flow {
    let left = x.m16(f::TIMER).wrapping_sub(1);
    x.set_m16(f::TIMER, left);
    if left != 0 {
        crate::mem::rts(x.c, x.b);
        return Flow::Yield;
    }
    t18_text_object(x)
}
fn t18_text_object(x: &mut Ctx) -> Flow {
    x.call(ext::ALLOC_SOBJ);
    if !x.ok() {
        return x.wait(1, T18_SUB_RETRY);
    }
    let o = x.c.a[1];
    wr8(x.b, o, 1);
    wr8(x.b, o + 2, 0);
    wr8(x.b, o + 3, 0xf0);
    wr8(x.b, o + 0x2f, 0);
    wr32(x.b, o + 8, 0);
    wr16(x.b, o + 0xc, 0x3f0);
    wr32(x.b, o + 0x10, 1);
    let target = x.m16(f::OBJECT) as i16 as i32 as u32 & 0xff_ffff;
    x.c.a[1] = target;
    wr32(x.b, target + 4, 0x0600_0000);
    x.set_v16(var::SCROLL_2_Y, 0);
    x.set_v16(var::LAYERS, 0x12ce);
    x.jump_to(ext::OBJECT_LAUNCH)
}
const T18_SUB_RETRY: u32 = 0xfa77c;
resume!(R_T18_SUB_RETRY, T18_SUB_RETRY, |x| {
    // `bra 0xfa762`
    t18_sub(x)
});

/// Étape 26 : après 60 images, vide les plans et crée l'objet du logo (variante 66).
fn t1a_wait(x: &mut Ctx) -> Flow {
    x.render();
    let left = x.m16(f::TIMER).wrapping_sub(1);
    x.set_m16(f::TIMER, left);
    if left != 0 {
        return Flow::Done;
    }
    x.next_step();
    x.c.a[1] = GFX_PLANE_1_START;
    x.call_d0(ext::CLEAR_BLOCK, 7);
    x.call_d0(ext::CLEAR_BLOCK, 8);
    x.call(ext::SOUND_E185C);
    t1a_object(x)
}
fn t1a_object(x: &mut Ctx) -> Flow {
    x.call(ext::ALLOC_SET);
    if !x.ok() {
        return x.wait(1, T1A_RETRY);
    }
    let o = x.c.a[1];
    wr8(x.b, o, 1);
    wr8(x.b, o + 2, 0);
    wr8(x.b, o + 3, 0x42);
    wr8(x.b, o + 0x2f, 3);
    x.set_m16(f::OBJECT, o as u16);
    Flow::Done
}
const T1A_RETRY: u32 = 0xfa7fc;
resume!(R_T1A_RETRY, T1A_RETRY, t1a_object);

/// Étape 28 : attend que l'objet du logo ait atteint l'état 6, puis crée les textes (3 objets) et affiche
/// la mention de droits selon la région.
fn t1c_wait_logo(x: &mut Ctx) -> Flow {
    x.render();
    let o = x.m16(f::OBJECT) as i16 as i32 as u32 & 0xff_ffff;
    if rd8(x.b, o + 4) != 6 {
        return Flow::Done;
    }
    x.next_step();
    title_text(x)
}
/// `0xfa830` : trois objets de texte autour de la caméra, puis le texte de droits.
pub fn title_text(x: &mut Ctx) -> Flow {
    x.c.d[5] = 2;
    x.c.d[4] = 0x00e4_e5e6;
    x.c.a[2] = 0xfa8bc;
    x.c.a[3] = crate::mem::A5 + 0x4fe;
    if x.v8(var::SCREEN_TYPE) != 0 {
        x.c.a[3] = crate::mem::A5 + 0x54e;
    }
    loop {
        x.call(ext::ALLOC_SOBJ);
        if !x.ok() {
            break;
        }
        let o = x.c.a[1];
        wr8(x.b, o, 1);
        wr8(x.b, o + 2, 0);
        wr8(x.b, o + 3, x.c.d[4] as u8);
        x.c.d[4] >>= 8;
        let cam = x.c.a[3];
        let (px, py) = (rd16(x.b, cam + 8), rd16(x.b, cam + 12));
        let (dx, dy) = (rd16(x.b, x.c.a[2]), rd16(x.b, x.c.a[2] + 2));
        x.c.a[2] += 4;
        wr16(x.b, o + 8, px.wrapping_add(dx));
        wr16(x.b, o + 12, py.wrapping_add(dy));
        wr32(x.b, o + 0x10, 0);
        let left = (x.c.d[5] as u16).wrapping_sub(1);
        set_low_word(&mut x.c.d[5], left);
        if left == 0xffff {
            break;
        }
    }
    x.wait(1, T1C_TEXT)
}
const T1C_TEXT: u32 = 0xfa88e;
resume!(R_T1C_TEXT, T1C_TEXT, |x| {
    x.render();
    x.call(ext::INVITE_START);
    x.set_m16(f::TIMER, 300);
    x.c.a[4] = 0xfacc4;
    x.call(ext::PRINT);
    let region = x.v16(var::REGION);
    x.c.d[0] = (x.c.d[0] & 0xffff_0000) | (region.wrapping_add(region) as u32);
    let index = (x.c.d[0] as u16) as i16 as i32 as u32;
    x.c.a[4] = rd32(x.b, 0xfad92u32.wrapping_add(index));
    x.call(ext::PRINT);
    Flow::Done
});

/// Étape 30 : après 300 images, fondu au noir (tâche), vide tout et passe au sous-mode suivant.
fn t1e_end(x: &mut Ctx) -> Flow {
    x.render();
    leave_screen(x, T1E_FADED)
}
const T1E_FADED: u32 = 0xfa90e;

/// Fin d'écran commune aux deux titres : minuterie de 300 images, sous-mode suivant, fondu
/// au noir par la tâche, puis nettoyage en trois temps.
fn leave_screen(x: &mut Ctx, loop_resume: u32) -> Flow {
    let left = x.m16(f::TIMER).wrapping_sub(1);
    x.set_m16(f::TIMER, left);
    if left != 0 {
        return Flow::Done;
    }
    let s = rd8(x.b, mode::SUB).wrapping_add(2);
    wr8(x.b, mode::SUB, s);
    wr8(x.b, mode::BASE + f::STEP, 0);
    x.set_v8(var::FADE_STEPS, 1);
    x.set_v16(var::FADE_DELAY, 4);
    x.set_v8(var::FADE_PAGES, FADE_PAGES_ALL);
    start_fade_task(x, ext::FADE_TASK_OUT);
    x.wait(1, loop_resume)
}

/// Attente de la fin du fondu (`A5+0x48f`), puis effacement et cession de 16 images.
fn leave_wait(x: &mut Ctx, loop_resume: u32, cleared: u32, second_title: bool) -> Flow {
    x.render();
    if x.v8(var::FADE_STEPS) != 0 {
        return x.wait(1, loop_resume);
    }
    let a0 = x.c.a[0];
    // l'original libère les pools dans un ordre différent selon l'écran
    let pools = if second_title {
        [ext::POOLS_SET, ext::POOLS_EFFCT, ext::POOLS_SOBJ]
    } else {
        [ext::POOLS_SOBJ, ext::POOLS_SET, ext::POOLS_EFFCT]
    };
    for f in pools.into_iter().chain([ext::RESET_PLAYERS, ext::SPRITES_RESET]) {
        x.call(f);
    }
    x.c.a[0] = a0;
    x.call(sub::CLEAR_SPRITES);
    if second_title {
        x.set_v8(var::SCREEN_TYPE, 0);
        x.and16(var::VIDEO_FLAGS, !1);
    }
    x.and16(var::LAYERS, !LAYER_BITS);
    x.wait(16, cleared)
}
resume!(R_T1E_WAIT, T1E_FADED, |x| leave_wait(x, T1E_FADED, T1E_CLEARED, false));
const T1E_CLEARED: u32 = 0xfa94e;
resume!(R_T1E_CLEARED, T1E_CLEARED, |x| {
    x.call(ext::INVITE_STOP);
    x.call(sub::CLEAR_SCROLL_1);
    x.call(sub::CLEAR_SCROLL_2);
    x.or16(var::LAYERS, LAYER_BITS);
    x.wait(1, T1E_DONE)
});
const T1E_DONE: u32 = 0xfa96e;
resume!(R_T1E_DONE, T1E_DONE, |x| {
    x.set_m16(f::TIMER, 300);
    x.c.d[0] = 1;
    x.jump_to(ext::PALETTE_PAGE_A)
});

// ---------------------------------------------------------------------------------------------
// Sous-mode 12 : second titre

fn second_title_step(x: &mut Ctx, step: u8) -> Flow {
    match step {
        0 => {
            x.call(ext::INVITE_STOP);
            x.next_step();
            reset_objects(x);
            Flow::Done
        }
        2 => s02_setup(x),
        4 => s04_wait_object(x),
        6 => s06_wait_logo(x),
        8 => {
            x.render();
            leave_screen(x, S08_WAIT)
        }
        _ => unknown_step(x, 0xfaef2),
    }
}

/// Étape 2 : fond de titre, palette, deux objets (variante 9 de SOBJ, puis SET 2).
fn s02_setup(x: &mut Ctx) -> Flow {
    x.set_v16(var::LAYERS, 0x12c2);
    x.c.d[0] = 0;
    for off in [var::CAMERA_X, var::CAMERA_Y, var::CAMERA_2_X, var::CAMERA_2_Y] {
        x.set_v32(off, 0);
    }
    x.wait(4, S02_LOAD)
}
const S02_LOAD: u32 = 0xfaf5c;
resume!(R_S02_LOAD, S02_LOAD, |x| {
    x.set_v8(var::SCREEN_TYPE, 1);
    load_title_palette(x);
    x.call_d0(ext::PALETTE_PAGE_C, 1);
    fade_setup(x, 3, 13);
    x.call(ext::BRIGHTNESS_WHITE);
    x.or16(var::VIDEO_FLAGS, 1);
    let flags = x.v16(var::VIDEO_FLAGS);
    wr16(x.b, CPS_REG_VIDEO, flags);
    wr16(x.b, CPS_REG_SCROLL, 0x9200);
    x.set_v16(var::CPS_COPY, 0x9200);
    x.set_v16(var::GAUGE_SKIP, 0);
    clear_sprite_area(x);
    x.c.d[0] = 0;
    set_low_word(&mut x.c.d[0], 454);
    x.c.a[1] = GFX_PLANE_2;
    x.call(ext::TILE_BLOCK);
    set_low_word(&mut x.c.d[0], 455);
    x.call(ext::TILE_BLOCK);
    set_low_word(&mut x.c.d[0], 368);
    x.c.a[1] = GFX_TABLE_B;
    x.call(ext::TILE_BLOCK_2);
    set_low_word(&mut x.c.d[0], 369);
    x.call(ext::TILE_BLOCK_2);
    s02_object(x)
});
fn s02_object(x: &mut Ctx) -> Flow {
    x.call_d0(ext::SOBJ_VARIANT, 9);
    if !x.ok() {
        return x.wait(1, S02_RETRY);
    }
    x.set_m8(f::CUE, 0);
    wr32(x.b, x.c.a[1] + 0x4c, mode::BASE);
    let link = x.c.a[1] as u16;
    x.set_m16(f::DONE, link);
    x.render();
    x.wait(2, S02_SHOW)
}
const S02_RETRY: u32 = 0xfaff8;
resume!(R_S02_RETRY, S02_RETRY, |x| {
    s02_object(x)
});
const S02_SHOW: u32 = 0xfb012;
resume!(R_S02_SHOW, S02_SHOW, |x| {
    x.or16(var::LAYERS, LAYER_BITS);
    x.next_step();
    x.set_v16(var::SCROLL_2_Y, 0);
    x.call_d0(ext::SET_VARIANT, 2);
    x.wait(1, S02_PALETTE)
});
const S02_PALETTE: u32 = 0xfb030;
resume!(R_S02_PALETTE, S02_PALETTE, |x| {
    x.call(ext::PALETTE_PAGE_B);
    x.set_v8(var::FADE_PAGES, 2);
    x.jump_to(ext::BRIGHTNESS_WHITE)
});

/// Efface la zone de sprites de 512 octets (`0xfb18a`).
fn clear_sprite_area(x: &mut Ctx) {
    for i in 0..128 {
        wr32(x.b, SPRITE_CLEAR_AREA + 4 * i, 0);
    }
    x.c.a[1] = SPRITE_CLEAR_AREA + 512;
    x.c.d[0] = 0;
    x.c.d[1] = (x.c.d[1] & 0xffff_0000) | 0xffff;
}

/// Étape 4 : attend le signal de l'objet (`+8`), crée l'objet SET de variante 68.
fn s04_wait_object(x: &mut Ctx) -> Flow {
    x.render();
    if x.m8(f::CUE) == 0 {
        return Flow::Done;
    }
    x.next_step();
    s04_object(x)
}
fn s04_object(x: &mut Ctx) -> Flow {
    x.call(ext::ALLOC_SET);
    if !x.ok() {
        return x.wait(1, S04_RETRY);
    }
    let o = x.c.a[1];
    wr8(x.b, o, 1);
    wr8(x.b, o + 2, 0);
    wr8(x.b, o + 3, 0x44);
    wr8(x.b, o + 0x2f, 3);
    x.set_m16(f::OBJECT, o as u16);
    x.wait(1, S04_SHOW)
}
const S04_RETRY: u32 = 0xfb064;
resume!(R_S04_RETRY, S04_RETRY, s04_object);
const S04_SHOW: u32 = 0xfb088;
resume!(R_S04_SHOW, S04_SHOW, |x| {
    x.render();
    x.c.a[1] = GFX_PLANE_1_START;
    x.call_d0(ext::CLEAR_BLOCK, 7);
    x.call_d0(ext::CLEAR_BLOCK, 8);
    Flow::Done
});

/// Étape 6 : attend que l'objet du logo soit à l'état 6, puis texte de droits (comme le titre 1).
fn s06_wait_logo(x: &mut Ctx) -> Flow {
    x.render();
    let o = x.m16(f::OBJECT) as i16 as i32 as u32 & 0xff_ffff;
    if rd8(x.b, o + 4) != 6 {
        return Flow::Done;
    }
    x.next_step();
    title_text(x)
}

const S08_WAIT: u32 = 0xfb106;
resume!(R_S08_WAIT, S08_WAIT, |x| leave_wait(x, S08_WAIT, S08_CLEARED, true));
const S08_CLEARED: u32 = 0xfb152;
resume!(R_S08_CLEARED, S08_CLEARED, |x| {
    x.call(sub::CLEAR_SCROLL_1);
    x.call(sub::CLEAR_SCROLL_2);
    x.call(sub::CLEAR_SCROLL_3);
    x.set_v16(var::GAUGE_SKIP, 0);
    clear_sprite_area(x);
    x.call_d0(ext::PALETTE_PAGE_A, 1);
    x.or16(var::LAYERS, LAYER_BITS);
    x.wait(1, S08_DONE)
});
const S08_DONE: u32 = 0xfb182;
resume!(R_S08_DONE, S08_DONE, |x| {
    x.set_m16(f::TIMER, 300);
    Flow::Done
});

pub const NATIVES: &[Native] = &[
    R_T00_LOAD, R_T00_FADE, R_T06_OBJECT, R_T06_RETRY, R_T08_NEXT, R_T0A_RETRY, R_T10_OBJECT, R_T10_RETRY,
    R_T12_SCENERY, R_T12_FADE, R_T18_RETRY, R_T18_LOOP, R_T18_AFTER, R_T18_SUB_RETRY, R_T1A_RETRY,
    R_T1C_TEXT, R_T1E_WAIT, R_T1E_CLEARED, R_T1E_DONE, R_S02_LOAD, R_S02_RETRY, R_S02_SHOW,
    R_S02_PALETTE, R_S04_RETRY, R_S04_SHOW, R_S08_WAIT, R_S08_CLEARED, R_S08_DONE,
];
