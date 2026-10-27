//! Menu TEST (M1, `docs/service.md`) : le menu de service de la borne et ses écrans de test.
//!
//! La tâche du menu (`taches/service.rs`) appelle chaque image le gestionnaire de l'état courant :
//! préparation (0), menu (2) ou écran choisi (4). L'état vit dans l'enregistrement [`MENU`] (FF1CF8,
//! remis à zéro à la création de la tâche) :
//!
//! | Octet | Rôle |
//! |---|---|
//! | 0 | état du menu : 0 préparation, 2 menu, 4 écran de test |
//! | 1 | ligne choisie (0, 2... 16 : INPUT... EXIT) |
//! | 2 | étape de l'écran de test (0 : préparation) |
//! | 3-5 | curseurs des menus de configuration |
//! | 0x10, 0x11, 0x12 | répétition automatique (haut/bas, gauche/droite, tirs) |
//! | 0x30-0x33 | SOUND & VOICE : code (mot), volume, à réafficher |
//! | 0x5e | « Secret Test » demandé (si le mot 0xc0 du programme n'est pas nul : jamais) |
//!
//! Les écrans sont des fonctions Rust ; ceux qui cèdent la main au milieu (le menu après l'effacement de
//! l'écran) reprennent dans une native posée à l'adresse de retour d'origine, la pile de la tâche contenant
//! les mêmes adresses de retour que l'original.

pub mod essais;
pub mod memoire;
pub mod mire;
pub mod reglages;

use crate::config;
use crate::mem::{set_low_word, wr32};
use crate::modes::{Flow, Sequence};
use crate::scene::Ctx;
use crate::Native;

/// Enregistrement du menu (A5 - 0x6308), 0x60 octets.
pub const MENU: u32 = 0xff_1cf8;
pub const MENU_LONGS: u32 = 24;

/// Champs de l'enregistrement.
pub mod f {
    pub const STATE: u32 = 0;
    pub const CURSOR: u32 = 1;
    pub const STEP: u32 = 2;
    pub const REPEAT_V: u32 = 0x10;
    pub const REPEAT_H: u32 = 0x11;
    pub const REPEAT_SHOTS: u32 = 0x12;
    pub const SECRET: u32 = 0x5e;
}

/// États du menu.
mod state {
    pub const INIT: u8 = 0;
    pub const MENU: u8 = 2;
    pub const SCREEN: u8 = 4;
}

/// Variables de la RAM de travail (décalages depuis A5).
pub mod var {
    /// Entrées : état courant P1..P4, boutons système (départs, pièces), interrupteurs (test, service).
    pub const INPUT: u32 = 0;
    pub const SYSTEM: u32 = 4;
    pub const SWITCHES: u32 = 5;
    pub const PREV: u32 = 6;
    pub const LAYERS: u32 = 0x14;
    pub const FLAGS: u32 = 0x12;
    pub const CHUTES: u32 = 0x1d;
    pub const VARIANT: u32 = 0x70;
    pub const DEV_BOARD: u32 = 0x82;
    pub const CARD_DISPENSER: u32 = 0x84;
    /// Menu TEST actif (lu par le son).
    pub const ACTIVE: u32 = 0x7fcc;
    pub const SCROLLS: [u32; 6] = [0x4b6, 0x4ba, 0x506, 0x50a, 0x556, 0x55a];
    /// Octets remis à zéro par le menu (rôle inconnu ; `0x5bc` est la sortie recopiée en 0x8040e1 par le VBLANK).
    pub const FRAME_FLAG: u32 = 0x5bd;
    pub const BYTE_34: u32 = 0x34;
}

/// Bits des entrées d'un joueur (voir `inputs.rs`).
pub mod key {
    pub const RIGHT: u8 = 0x01;
    pub const LEFT: u8 = 0x02;
    pub const DOWN: u8 = 0x04;
    pub const UP: u8 = 0x08;
    pub const SHOT1: u8 = 0x10;
    pub const SHOT2: u8 = 0x20;
    /// Départ du joueur 1 dans l'octet système.
    pub const START1: u8 = 0x01;
}

/// Routines d'origine.
pub mod ext {
    pub const LOCK: u32 = 0xb96;
    pub const UNLOCK: u32 = 0xbac;
    pub const LAYERS_ON: u32 = 0xbc6;
    pub const LAYERS_OFF: u32 = 0xbfc;
    pub const CLEAR_OBJECTS: u32 = 0xc58;
    pub const CLEAR_SCROLL_1: u32 = 0xca8;
    pub const CLEAR_SCROLL_2: u32 = 0xcc8;
    pub const CLEAR_SCROLL_3: u32 = 0xce8;
    pub const PRINT: u32 = 0xd5e;
    pub const PALETTES: u32 = 0x1a296;
    pub const SOUND_REQUEST: u32 = 0xe0836;
    pub const BOOT_TASK: u32 = 0x11b16;
    pub const SECRET_TEST: u32 = 0xee16e;
}

/// Textes du menu (descripteurs en ROM).
mod text {
    pub const MENU: u32 = 0x29f1e;
    pub const HELP: u32 = 0x2a03a;
}

/// Sorties de la carte (0x400004), valeur posée par le menu.
const OUTPUTS: u32 = 0x400004;
const MENU_OUTPUTS: u16 = 0x2355;
/// Mot du programme qui autorise le « Secret Test » (nul dans ce jeu).
const SECRET_ENABLE: u32 = 0xc0;
/// Couche de caractères : flèche du menu, ligne « 7. CONFIGURATION » colorée si les réglages diffèrent
/// des réglages d'usine.
const MENU_ARROW: u32 = 0x912690;
const MENU_CONFIG_LINE: u32 = 0x912940;
/// Couleur de mise en évidence (attribut de case).
pub const HIGHLIGHT: u16 = 0x13;
/// Lignes du menu : la dernière est EXIT.
const LAST_LINE: u8 = 0x10;

// ---------------------------------------------------------------------------------------------
// Accès

pub fn get(x: &mut Ctx, field: u32) -> u8 {
    x.rd8(MENU + field)
}
pub fn set(x: &mut Ctx, field: u32, v: u8) {
    crate::mem::wr8(x.b, MENU + field, v)
}
pub fn get16(x: &mut Ctx, field: u32) -> u16 {
    x.rd16(MENU + field)
}
pub fn set16(x: &mut Ctx, field: u32, v: u16) {
    x.wr16(MENU + field, v)
}
/// Étape suivante de l'écran.
pub fn next_step(x: &mut Ctx) {
    let s = get(x, f::STEP).wrapping_add(2);
    set(x, f::STEP, s);
}

/// Touches de P1 nouvellement pressées (actif à 1).
pub fn pressed(x: &mut Ctx) -> u8 {
    x.v8(var::INPUT) & !x.v8(var::PREV)
}

/// Appelle une routine d'origine avec D0 (mot bas) chargé.
#[track_caller]
pub fn call_d0(x: &mut Ctx, addr: u32, d0: u16) {
    set_low_word(&mut x.c.d[0], d0);
    x.call(addr);
}
/// Affiche un texte (descripteur en ROM).
pub fn print(x: &mut Ctx, descriptor: u32) {
    crate::text::print(x.b, descriptor);
}
/// Couches de l'écran coupées / rétablies (masque 0x12ce).
pub const ALL_LAYERS: u16 = 0x12ce;
pub fn layers_off(x: &mut Ctx) {
    call_d0(x, ext::LAYERS_OFF, ALL_LAYERS);
}
pub fn layers_on(x: &mut Ctx) {
    call_d0(x, ext::LAYERS_ON, ALL_LAYERS);
}

// ---------------------------------------------------------------------------------------------
// Aides communes (0x29948-0x29de0)

/// Réglage à répétition automatique (`0x29948` vertical : haut recule ; `0x29994` horizontal : gauche
/// recule ; `0x2ca5e` tirs : tir 2 recule, sauf avec le tir 1). `value` avance de `step` dans `0..=max` en
/// bouclant ; une touche nouvellement pressée agit aussitôt puis après 30 images, une touche tenue toutes
/// les 8 images (compteur `counter` de l'enregistrement). Rend la nouvelle valeur si la touche a agi.
fn repeat(x: &mut Ctx, value: u8, step: u8, max: u8, keys: u8, back: fn(u8) -> bool, counter: u32) -> Option<u8> {
    let held = x.v8(var::INPUT) & keys;
    if held == 0 {
        return None;
    }
    let new = !x.v8(var::PREV) & keys & held;
    let dir = if new != 0 {
        set(x, counter, 0x1e);
        new
    } else {
        let left = (get(x, counter) as i8 as i16) - 1;
        set(x, counter, left as u8);
        if left > 0 {
            return None;
        }
        set(x, counter, 8);
        held
    };
    let v = if back(dir) {
        let v = value as i8 as i16 - step as i8 as i16;
        if v >= 0 { v as u8 } else { max }
    } else {
        let v = value.wrapping_add(step);
        if (max as i8) >= (v as i8) { v } else { 0 }
    };
    Some(v)
}

pub fn vertical(x: &mut Ctx, value: u8, step: u8, max: u8) -> Option<u8> {
    repeat(x, value, step, max, key::UP | key::DOWN, |d| d & key::UP != 0, f::REPEAT_V)
}
pub fn horizontal(x: &mut Ctx, value: u8, step: u8, max: u8) -> Option<u8> {
    repeat(x, value, step, max, key::RIGHT | key::LEFT, |d| d & key::LEFT != 0, f::REPEAT_H)
}
pub fn shots(x: &mut Ctx, value: u8, step: u8, max: u8) -> Option<u8> {
    repeat(x, value, step, max, key::SHOT1 | key::SHOT2, |d| d & key::SHOT1 == 0, f::REPEAT_SHOTS)
}

/// Case d'une ligne de menu : `base` + 4 × `line` (une ligne de menu sur deux de la couche).
fn line_cell(base: u32, line: u8) -> u32 {
    base + 4 * line as u32
}
/// `0x299fc` : flèche (tuile 0x2a) devant la ligne ; `0x29a10` : effacée.
pub fn arrow(x: &mut Ctx, base: u32, line: u8) {
    wr32(x.b, line_cell(base, line), 0x002a_0000);
}
pub fn no_arrow(x: &mut Ctx, base: u32, line: u8) {
    wr32(x.b, line_cell(base, line), 0x0020_0000);
}
/// Déplace la flèche d'une ligne à l'autre.
pub fn move_arrow(x: &mut Ctx, base: u32, from: u8, to: u8) {
    no_arrow(x, base, from);
    arrow(x, base, to);
}

/// `0x299e0` : attribut `attr` sur `len` + 1 cases d'une ligne de la couche de caractères.
pub fn paint(x: &mut Ctx, cell: u32, attr: u16, len: u32) {
    for k in 0..=len {
        x.wr16(cell + 2 + 0x80 * k, attr);
    }
}

/// `0x29a7a` : pleine luminosité pour les quatre pages de palettes (0x91c000-0x91cfff).
pub fn full_brightness(x: &mut Ctx) {
    for a in (0x91c000..0x91d000).step_by(4) {
        let v = x.rd32(a) | 0xf000_f000;
        wr32(x.b, a, v);
    }
}

/// Tampon de la conversion décimale (A5 - 0x62a8, 8 octets BCD).
const BCD: u32 = 0xff_1d58;

/// `0x29da4` : `value` en décimal codé binaire, dans le tampon.
fn to_bcd(x: &mut Ctx, value: u32) -> u64 {
    let mut bcd = 0u64;
    let mut v = value;
    for shift in 0.. {
        if v == 0 {
            break;
        }
        bcd |= ((v % 10) as u64) << (4 * shift);
        v /= 10;
    }
    wr32(x.b, BCD, (bcd >> 32) as u32);
    wr32(x.b, BCD + 4, bcd as u32);
    bcd
}

/// `0x29d88` : les `n` chiffres hexadécimaux de poids faible de `value` (tuile = chiffre), de gauche à
/// droite, attribut `attr`. Rend la case qui suit.
pub fn hex_digits(x: &mut Ctx, mut cell: u32, value: u64, n: u32, attr: u16) -> u32 {
    for k in (0..n).rev() {
        x.wr16(cell, (value >> (4 * k)) as u16 & 0xf);
        x.wr16(cell + 2, attr);
        cell += 0x80;
    }
    cell
}

/// `0x29cf2` (8 chiffres), `0x29d3a` (2 chiffres) : nombre en décimal.
pub fn decimal(x: &mut Ctx, cell: u32, value: u32, n: u32, attr: u16) -> u32 {
    let bcd = to_bcd(x, value);
    hex_digits(x, cell, bcd, n, attr)
}

// ---------------------------------------------------------------------------------------------
// Le menu

/// Un tour de la tâche : gestionnaire de l'état du menu.
pub fn handler(x: &mut Ctx) -> Flow {
    match get(x, f::STATE) {
        state::INIT => {
            x.set_v16(var::ACTIVE, 1);
            set(x, f::STATE, state::MENU);
            show_menu(x)
        }
        state::MENU => menu(x),
        _ => screen(x),
    }
}

/// `0x291a0` : écran remis à neuf (couches coupées, VRAM effacée, palettes d'origine à pleine luminosité),
/// une image d'attente, puis le menu (`menu_after_clear`).
pub fn show_menu(x: &mut Ctx) -> Flow {
    layers_off(x);
    for f in [ext::CLEAR_OBJECTS, ext::CLEAR_SCROLL_1, ext::CLEAR_SCROLL_2, ext::CLEAR_SCROLL_3, ext::PALETTES] {
        x.call(f);
    }
    full_brightness(x);
    for off in var::SCROLLS {
        x.set_v16(off, 0);
    }
    crate::taches::une_image(x, crate::taches::service::Etape::MenuAfterClear)
}

/// `0x291ec` : le menu, une fois l'écran effacé.
pub fn menu_after_clear(x: &mut Ctx) -> Flow {
    x.set_v8(var::FRAME_FLAG, 0);
    set(x, f::SECRET, 0);
    set(x, f::REPEAT_V, 1);
    x.wr16(OUTPUTS, MENU_OUTPUTS);
    call_d0(x, ext::UNLOCK, 0xf1);
    print(x, text::MENU);
    print(x, text::HELP);
    let line = get(x, f::CURSOR);
    arrow(x, MENU_ARROW, line);
    if !config::is_factory(x) {
        paint(x, MENU_CONFIG_LINE, HIGHLIGHT, 0xc);
    }
    layers_on(x);
    Flow::Done
}

/// `0x29254` : choix d'une ligne (haut, bas), entrée dans l'écran (tir 1).
fn menu(x: &mut Ctx) -> Flow {
    if x.rd16(SECRET_ENABLE) != 0 {
        if get(x, f::SECRET) != 0 {
            return x.jump_to(ext::SECRET_TEST);
        }
        if (x.v8(0) & x.v8(1)) & key::SHOT2 != 0 {
            set(x, f::SECRET, 1);
            set(x, f::STEP, 0);
            return Flow::Done;
        }
    }
    let line = get(x, f::CURSOR);
    if let Some(to) = vertical(x, line, 2, LAST_LINE) {
        if to != line {
            move_arrow(x, MENU_ARROW, line, to);
            set(x, f::CURSOR, to);
        }
    }
    if pressed(x) & key::SHOT1 != 0 {
        set(x, f::STATE, state::SCREEN);
        if get(x, f::CURSOR) != LAST_LINE {
            x.call(ext::CLEAR_SCROLL_1);
        }
    }
    Flow::Done
}

/// `0x292da` : l'écran de la ligne choisie.
fn screen(x: &mut Ctx) -> Flow {
    match get(x, f::CURSOR) {
        0 => essais::input(x),
        2 => essais::output(x),
        4 => essais::sound(x),
        6 => mire::color(x),
        8 => mire::cross_hatch(x),
        10 => mire::game_data(x),
        12 => reglages::configuration(x),
        14 => memoire::memory(x),
        _ => exit(x),
    }
}

/// `0x29314` (9. EXIT) : écran effacé, la tâche redevient la tâche de démarrage (`0xae4`).
fn exit(x: &mut Ctx) -> Flow {
    for f in [ext::CLEAR_OBJECTS, ext::CLEAR_SCROLL_1, ext::CLEAR_SCROLL_2, ext::CLEAR_SCROLL_3] {
        x.call(f);
    }
    for off in var::SCROLLS {
        x.set_v32(off, 0);
    }
    x.set_v16(var::ACTIVE, 0);
    x.c.a[0] = ext::BOOT_TASK;
    crate::taches::recommencer(x)
}

/// `0x29a48` : départ 1P tenu et tir 1 : retour au menu.
pub fn exit_check(x: &mut Ctx) -> Flow {
    if x.v8(var::SYSTEM) & key::START1 == 0 || x.v8(var::INPUT) & key::SHOT1 == 0 {
        return Flow::Done;
    }
    back_to_menu(x)
}

/// `0x29a5a` : retour au menu (le son est coupé).
pub fn back_to_menu(x: &mut Ctx) -> Flow {
    crate::sound::service(x.b, crate::sound::Service::ArretGeneral);
    set(x, f::STATE, state::MENU);
    // étape de l'écran et curseurs de la configuration (octets 2 à 5)
    x.wr16(MENU + f::STEP, 0);
    x.wr16(MENU + f::STEP + 2, 0);
    let flags = x.v16(var::FLAGS) & 0x7fff;
    x.set_v16(var::FLAGS, flags);
    x.set_v8(var::BYTE_34, 0);
    show_menu(x)
}

/// Les deux blocages définitifs du menu TEST et des réglages : la tâche reste sur une boucle `bra.b *` de l'original,
/// que la machine tient sans interpréteur en laissant passer le temps jusqu'à la fin de l'image. Aucune
/// EEPROM ni aucune RAM de la machine émulée n'y mène (voir `docs/service.md` §7) : ils existent pour que
/// l'adresse ait une native.
const fn blocage(entry: u32, name: &'static str) -> Native {
    Native { entry, name, run: |_, _| {}, touches_sound: false, outputs: 0 }
}

/// « NG » du MEMORY CHECK (`0x2d112`).
pub const BLOCAGE_MEMOIRE: Native = blocage(memoire::BLOQUE, "service_blocage_memoire");
/// « EEPROM ERROR » (`0x2d7fa`).
pub const BLOCAGE_EEPROM: Native = blocage(config::BLOQUE, "service_blocage_eeprom");

pub const NATIVES: &[Native] = &[BLOCAGE_MEMOIRE, BLOCAGE_EEPROM];
