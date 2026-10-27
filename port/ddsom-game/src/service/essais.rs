//! Écrans 1. INPUT, 2. OUTPUT et 3. SOUND & VOICE du menu TEST.
//!
//! Chacun a deux étapes (préparation, puis mise à jour à chaque image) et revient au menu par départ 1P
//! tenu + tir 1 (`exit_check`).

use super::{call_d0, exit_check, ext, f, get, get16, hex_digits, key, layers_off, layers_on, next_step, pressed, print, set, set16, var};
use crate::modes::Flow;
use crate::scene::Ctx;

/// Joueurs affichés selon le type de monnayeurs (`A5+0x1d` : 0-2 deux joueurs, 3-6 trois, 7+ quatre).
fn players(x: &mut Ctx) -> u32 {
    match x.v8(var::CHUTES) as i8 {
        c if c >= 7 => 4,
        c if c >= 3 => 3,
        _ => 2,
    }
}

// ---------------------------------------------------------------------------------------------
// 1. INPUT (0x2a4f4)

mod input {
    /// Titre, aide, et tableaux des 2, 3 ou 4 joueurs (pointeurs adressés relativement au PC).
    pub const TITLE: u32 = 0x2a6e4;
    pub const HELP: u32 = 0x2a726;
    pub const TABLES: u32 = 0x2a558;
    /// Cases des « 0/1 » (tables de pointeurs en ROM) : service, test, pièces, départs, leviers (une table
    /// de 8 directions par joueur), boutons.
    pub const SERVICE: u32 = 0x2a9f4;
    pub const TEST: u32 = 0x2a9f8;
    pub const COINS: u32 = 0x2a9fc;
    pub const STARTS: u32 = 0x2aa0c;
    pub const LEVERS: u32 = 0x2a63c;
    pub const BUTTONS: u32 = 0x2aa9c;
    /// Valeurs du levier (4 bits) pour chacune des 8 directions affichées.
    pub const DIRECTIONS: u32 = 0x2a64c;
}

pub fn input(x: &mut Ctx) -> Flow {
    if get(x, f::STEP) == 0 {
        next_step(x);
        layers_off(x);
        print(x, input::TITLE);
        print(x, input::HELP);
        layers_on(x);
        let table = 4 * (players(x) - 2);
        let text = (x.prog16(input::TABLES + table) as u32) << 16 | x.prog16(input::TABLES + table + 2) as u32;
        print(x, text);
    } else {
        input_update(x);
    }
    exit_check(x)
}

/// `0x2a6b6` : « 1 » en couleur ou « 0 » dans la case `k` de la table `cells`.
fn show_bit(x: &mut Ctx, cells: u32, k: u32, on: bool) {
    let cell = x.rd32(cells + 4 * k);
    x.wr16(cell, 0x30 + on as u16);
    x.wr16(cell + 2, if on { super::HIGHLIGHT } else { 0 });
}

/// `0x2a564` : état de chaque entrée.
fn input_update(x: &mut Ctx) {
    let switches = x.v8(var::SWITCHES);
    show_bit(x, input::SERVICE, 0, switches & 4 != 0);
    show_bit(x, input::TEST, 0, switches & 2 != 0);
    let n = players(x);
    let system = x.v8(var::SYSTEM);
    for k in 0..n {
        show_bit(x, input::COINS, k, (system >> 4) >> k & 1 != 0);
    }
    for k in 0..n {
        show_bit(x, input::STARTS, k, system >> k & 1 != 0);
    }
    for p in 0..n {
        let lever = x.v8(var::INPUT + p) & 0xf;
        let cells = x.rd32(input::LEVERS + 4 * p);
        for k in 0..8 {
            let on = lever == x.rd8(input::DIRECTIONS + k);
            show_bit(x, cells, 7 - k, on);
        }
    }
    // boutons : 4 bits par joueur, P1 d'abord
    let mut buttons = 0u32;
    for p in 0..4 {
        buttons |= ((x.v8(var::INPUT + p) >> 4) as u32) << (4 * p);
    }
    for k in 0..4 * n {
        show_bit(x, input::BUTTONS, k, buttons >> k & 1 != 0);
    }
}

// ---------------------------------------------------------------------------------------------
// 2. OUTPUT (0x2aadc)

mod output {
    pub const TITLE: u32 = 0x2acd2;
    pub const HELP: u32 = 0x2acf4;
    pub const LOCKS_12: u32 = 0x2ad16;
    pub const LOCKS_3: u32 = 0x2ad7a;
    pub const LOCKS_34: u32 = 0x2ad9c;
    pub const CARD: u32 = 0x2addc;
    /// Compteur électrique : état basculé par le tir 2 de P1 (A5 - 0x6294), et long remis à zéro (A5 - 0x6298).
    pub const COUNTER: u32 = 0xff_1d6c;
    pub const CLEARED: u32 = 0xff_1d68;
    /// Distributeur de cartes : commande (A5 - 0x5fcf).
    pub const DISPENSE: u32 = 0xff_2031;
}

/// Variantes du jeu qui ont un distributeur de cartes.
fn card_variant(x: &mut Ctx) -> bool {
    matches!(x.v16(var::VARIANT), 4 | 10)
}

pub fn output(x: &mut Ctx) -> Flow {
    if get(x, f::STEP) == 0 {
        next_step(x);
        x.wr16(output::CLEARED, 0);
        x.wr16(output::CLEARED + 2, 0);
        crate::mem::wr8(x.b, output::COUNTER, 0);
        layers_off(x);
        print(x, output::TITLE);
        print(x, output::HELP);
        print(x, output::LOCKS_12);
        let chutes = x.v8(var::CHUTES) as i8;
        if chutes > 2 {
            print(x, if chutes > 6 { output::LOCKS_34 } else { output::LOCKS_3 });
        }
        if card_variant(x) && x.v8(var::CARD_DISPENSER) != 0 {
            print(x, output::CARD);
        }
        layers_on(x);
    } else {
        output_update(x);
    }
    exit_check(x)
}

/// Verrou du monnayeur du joueur `p` (bit `mask`) : posé quand son tir 1 est pressé, levé quand il est
/// relâché.
fn coin_lock(x: &mut Ctx, p: u32, mask: u16) {
    let (cur, prev) = (x.v8(var::INPUT + p), x.v8(var::PREV + p));
    if cur & !prev & key::SHOT1 != 0 {
        call_d0(x, ext::LOCK, mask);
    } else if !cur & prev & key::SHOT1 != 0 {
        call_d0(x, ext::UNLOCK, mask);
    }
}

/// `0x2ab72`.
fn output_update(x: &mut Ctx) {
    coin_lock(x, 0, 0x10);
    coin_lock(x, 1, 0x20);
    let chutes = x.v8(var::CHUTES) as i8;
    if chutes >= 3 {
        coin_lock(x, 2, 0x40);
        if chutes >= 7 {
            coin_lock(x, 3, 0x80);
        }
    }
    // compteur électrique : le tir 2 de P1 le bascule à chaque changement
    if (x.v8(var::INPUT) ^ x.v8(var::PREV)) & key::SHOT2 != 0 {
        let on = crate::mem::rd8(x.b, output::COUNTER) ^ 1;
        crate::mem::wr8(x.b, output::COUNTER, on);
        call_d0(x, if on != 0 { ext::LOCK } else { ext::UNLOCK }, 1);
    }
    if card_variant(x) && x.v8(var::CARD_DISPENSER) != 0 {
        let new = x.v8(var::INPUT + 1) & !x.v8(var::PREV + 1);
        if new & key::SHOT2 != 0 {
            crate::mem::wr8(x.b, output::DISPENSE, 0x80);
        }
    }
}

// ---------------------------------------------------------------------------------------------
// 3. SOUND & VOICE (0x2b450)

mod sound {
    pub const TITLE: u32 = 0x2b618;
    pub const HELP: u32 = 0x2b7e4;
    /// Champs de l'enregistrement : code (mot), volume (0-39), code à réafficher.
    pub const CODE: u32 = 0x30;
    pub const VOLUME: u32 = 0x32;
    pub const REDRAW: u32 = 0x33;
    pub const REPEAT: u32 = 0x10;
    /// Plus grand code.
    pub const LAST: i32 = 0x4a0;
    /// Cases du code, du volume (deux chiffres) et de la jauge.
    pub const CODE_CELL: u32 = 0x912eac;
    pub const VOLUME_CELL: u32 = 0x912dd8;
    pub const GAUGE: u32 = 0x9121e0;
    pub const GAUGE_STEPS: u8 = 0x27;
    /// Volume du QSound, lu sur le port.
    pub const VOLUME_PORT: u32 = 0x804030;
    /// Pas du code par touche : (bit de la touche dans l'octet de P1, ajout au code).
    pub const KEYS: [(u8, i16); 6] = [(6, 0x100), (7, -0x100), (0, 0x10), (1, -0x10), (2, -1), (3, 1)];
}

pub fn sound(x: &mut Ctx) -> Flow {
    if get(x, f::STEP) == 0 {
        next_step(x);
        set16(x, sound::CODE, 0);
        set(x, sound::REPEAT, 0);
        set(x, sound::REDRAW, 1);
        layers_off(x);
        print(x, sound::TITLE);
        print(x, sound::HELP);
        layers_on(x);
    } else {
        sound_update(x);
    }
    exit_check(x)
}

/// `0x2b4a4`.
fn sound_update(x: &mut Ctx) {
    let held = x.v8(var::INPUT);
    let new = pressed(x);
    for (bit, delta) in sound::KEYS {
        code_key(x, held, new, bit, delta);
    }
    request(x, new);
    if get(x, sound::REDRAW) != 0 {
        set(x, sound::REDRAW, 0);
        let code = get16(x, sound::CODE);
        hex_digits(x, sound::CODE_CELL, code as u64, 4, super::HIGHLIGHT);
    }
    volume(x);
    gauge(x);
}

/// `0x2b518` : une touche change le code, avec répétition automatique (compteur partagé).
fn code_key(x: &mut Ctx, held: u8, new: u8, bit: u8, delta: i16) {
    if new >> bit & 1 != 0 {
        set(x, sound::REPEAT, 0x14);
    } else {
        if held >> bit & 1 == 0 {
            return;
        }
        let left = get(x, sound::REPEAT) as i8 as i16 - 1;
        set(x, sound::REPEAT, left as u8);
        if left > 0 {
            return;
        }
        set(x, sound::REPEAT, 4);
    }
    set(x, sound::REDRAW, 1);
    let code = get16(x, sound::CODE) as i16 as i32 + delta as i32;
    let code = if code < 0 {
        sound::LAST
    } else if code as i16 as i32 > sound::LAST {
        0
    } else {
        code
    };
    set16(x, sound::CODE, code as u16);
}

/// `0x2b556` : tir 1 joue le code (s'il n'est pas nul), tir 2 coupe le son.
fn request(x: &mut Ctx, new: u8) {
    if new & key::SHOT1 != 0 {
        let code = get16(x, sound::CODE);
        if code != 0 {
            // D2 et D3 sont ceux que laissent les touches du code (0x2b50e)
            x.c.d[1] = code as u32;
            x.c.d[2] = 1;
            x.c.d[3] = 3;
            x.call(ext::SOUND_REQUEST);
        }
    } else if new & key::SHOT2 != 0 {
        crate::sound::service(x.b, crate::sound::Service::ArretGeneral);
    }
}

/// `0x2b582` : volume (0 à 39) lu sur le port : bits 0-4 (premier bit levé), bits 5-12 (5 par bit nul).
fn volume(x: &mut Ctx) {
    let port = x.rd16(sound::VOLUME_PORT);
    let low = (port.trailing_zeros()).min(5) as u8;
    let high = ((port >> 5).trailing_zeros()).min(8) as u8;
    let level = sound::GAUGE_STEPS.wrapping_sub(low + 5 * high);
    set(x, sound::VOLUME, level);
    super::decimal(x, sound::VOLUME_CELL, level as u32, 2, super::HIGHLIGHT);
}

/// `0x2b5c0` : jauge du volume.
fn gauge(x: &mut Ctx) {
    let mut cell = sound::GAUGE;
    crate::mem::wr32(x.b, cell, 0x28_0000);
    crate::mem::wr32(x.b, cell + 0x1480, 0x29_0000);
    cell += 0x80;
    let level = get(x, sound::VOLUME);
    for _ in 0..level {
        crate::mem::wr32(x.b, cell, 0x2d_0000);
        cell += 0x80;
    }
    crate::mem::wr32(x.b, cell, 0x2b_0013);
    cell += 0x80;
    for _ in 0..sound::GAUGE_STEPS.wrapping_sub(level) {
        crate::mem::wr32(x.b, cell, 0x20_0013);
        cell += 0x80;
    }
}
