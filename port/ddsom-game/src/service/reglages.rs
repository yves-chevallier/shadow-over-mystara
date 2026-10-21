//! Écran 7. CONFIGURATION du menu TEST (`0x2bb0a`) : menu de configuration (SYSTEM, GAME, DEFAULT,
//! SAVE & EXIT) et ses deux sous-menus, 7-1 SYSTEM CONFIGURATION et 7-2 GAME CONFIGURATION.
//!
//! Les réglages sont modifiés directement dans les variables du jeu (`A5+0x1c...`, voir `config.rs`) ;
//! « SAVE & EXIT » les enregistre dans l'EEPROM. À l'entrée, leurs valeurs sont recopiées (A5 - 0x6292...) :
//! un réglage différent de sa valeur d'entrée est affiché en couleur, et la ligne « 4. SAVE & EXIT » (du
//! menu) l'est aussi tant que les réglages diffèrent des réglages d'usine.
//!
//! Champs de l'enregistrement : étape (`+2` : 0 entrée, 2 menu, 4 sous-menu), ligne du menu (`+3`), étape
//! du sous-menu (`+4`), ligne du sous-menu (`+5`), compteurs de répétition (`+0x10` haut/bas, `+0x11`
//! gauche/droite, `+0x12` tirs).

use super::{arrow, back_to_menu, ext, f, get, horizontal, key, layers_off, layers_on, move_arrow, paint, pressed, print, set, shots, var, vertical, HIGHLIGHT};
use crate::config;
use crate::modes::Flow;
use crate::scene::Ctx;

/// Champs de l'enregistrement propres à l'écran.
mod field {
    pub const LINE: u32 = 3;
    pub const SUB_STEP: u32 = 4;
    pub const SUB_LINE: u32 = 5;
}

/// Lignes du menu de configuration.
mod line {
    pub const SYSTEM: u8 = 0;
    pub const GAME: u8 = 2;
    pub const DEFAULT: u8 = 4;
    pub const SAVE: u8 = 6;
}

/// Textes.
mod text {
    pub const TITLE: u32 = 0x2caa6;
    pub const MENU: u32 = 0x2cad2;
    pub const SELECT: u32 = 0x2cb16;
    pub const HELP_START: u32 = 0x2cb36;
    pub const HELP_RESET: u32 = 0x2cb56;
    pub const HELP_EXIT: u32 = 0x2cb76;
    pub const SAVING: u32 = 0x2cdce;
    pub const SYSTEM_TITLE: u32 = 0x2cb96;
    /// Lignes de 7-1 : sans langue ni cartes, avec la langue (variante 8), avec les cartes (4, 10).
    pub const SYSTEM_LINES: u32 = 0x2cbc8;
    pub const SYSTEM_LINES_LANGUAGE: u32 = 0x2cc3c;
    pub const SYSTEM_LINES_CARDS: u32 = 0x2ce1a;
    pub const GAME_TITLE: u32 = 0x2ccc0;
    pub const GAME_LINES: u32 = 0x2ccee;
    pub const SUB_SELECT: u32 = 0x2cd12;
    pub const SUB_HELP: u32 = 0x2cd36;
    pub const SUB_HELP_EXIT: u32 = 0x2cd82;
}

/// Flèches du menu et des sous-menus ; ligne « 4. SAVE & EXIT » colorée hors réglages d'usine.
const MENU_ARROW: u32 = 0x91289c;
const SUB_ARROW: u32 = 0x91211c;
const SAVE_LINE: u32 = 0x912b2c;

/// Variables du jeu (décalages depuis A5).
mod v {
    pub const COIN_MODE: u32 = 0x1c;
    pub const CONTINUE: u32 = 0x23;
    /// Mode de monnayage « départ gratuit » : la ligne CONTINUE est sautée.
    pub const FREE_START: u8 = 0x11;
}

/// Valeurs d'entrée des réglages (A5 - 0x6292...), pour la mise en évidence des changements.
mod saved {
    pub const COIN_MODE: u32 = 0xff_1d6e;
    pub const CHUTES: u32 = 0xff_1d6f;
    pub const CONTINUE: u32 = 0xff_1d70;
    /// CONTINUE choisi (rétabli quand le mode de monnayage quitte le départ gratuit).
    pub const CONTINUE_CHOSEN: u32 = 0xff_1d71;
    pub const MONITOR: u32 = 0xff_1d72;
    pub const DEMO_SOUND: u32 = 0xff_1d73;
    pub const SOUND_MODE: u32 = 0xff_1d74;
    pub const LANGUAGE: u32 = 0xff_1d76;
    pub const CARD: u32 = 0xff_1d78;
    pub const CARD_POINT: u32 = 0xff_1d79;
    pub const BYTE_29: u32 = 0xff_1d7a;
    pub const DIFFICULTY: u32 = 0xff_1d7b;
    pub const BYTE_2B: u32 = 0xff_1d7c;
}

/// Variante du jeu : la liste des réglages en dépend.
#[derive(Clone, Copy, PartialEq)]
enum Variant {
    /// Variantes 4 et 10 : distributeur de cartes.
    Cards(u16),
    /// Variante 8 : choix de la langue.
    Language,
    Plain,
}

fn variant(x: &mut Ctx) -> Variant {
    match x.v16(var::VARIANT) {
        8 => Variant::Language,
        n @ (4 | 10) => Variant::Cards(n),
        _ => Variant::Plain,
    }
}

// ---------------------------------------------------------------------------------------------
// Les réglages

/// Affichage d'un réglage.
#[derive(Clone, Copy)]
enum Show {
    /// Texte de `width` caractères, le `n`-ième de la table à `stride` octets d'écart.
    Text { table: u32, stride: u32, width: u32 },
    /// « ON » / « OFF ».
    OnOff,
}

/// Un réglage : variable (mot pour la langue), pas et maximum, valeur d'entrée, case et affichage.
#[derive(Clone, Copy)]
struct Setting {
    var: u32,
    word: bool,
    step: u8,
    max: u8,
    saved: u32,
    cell: u32,
    show: Show,
}

const fn text(table: u32, stride: u32, width: u32) -> Show {
    Show::Text { table, stride, width }
}

const COIN_MODE: Setting =
    Setting { var: v::COIN_MODE, word: false, step: 1, max: 0x12, saved: saved::COIN_MODE, cell: 0x91299c, show: text(0x2c042, 32, 32) };
const CHUTES: Setting = Setting { var: 0x1d, word: false, step: 1, max: 0xb, saved: saved::CHUTES, cell: 0x9129a4, show: text(0x2c314, 26, 26) };
const CONTINUE: Setting = Setting { var: v::CONTINUE, word: false, step: 1, max: 1, saved: saved::CONTINUE, cell: 0x9129ac, show: Show::OnOff };
const MONITOR: Setting = Setting { var: 0x26, word: false, step: 1, max: 1, saved: saved::MONITOR, cell: 0x9129b4, show: text(0x2c4e8, 8, 8) };
const DEMO_SOUND: Setting = Setting { var: 0x27, word: false, step: 1, max: 1, saved: saved::DEMO_SOUND, cell: 0x9129bc, show: Show::OnOff };
const SOUND_MODE: Setting =
    Setting { var: 0x28, word: false, step: 1, max: 1, saved: saved::SOUND_MODE, cell: 0x9129c4, show: text(0x2c590, 16, 16) };
/// Langue : mot `A5+0x72`, valeurs paires 2 à 6 (0, le japonais, est sauté).
const LANGUAGE: Setting = Setting { var: 0x72, word: true, step: 2, max: 6, saved: saved::LANGUAGE, cell: 0x9129cc, show: text(0x2c610, 8, 16) };
const CARD: Setting = Setting { var: 0x84, word: false, step: 1, max: 1, saved: saved::CARD, cell: 0x912b4c, show: Show::OnOff };
/// Points de carte : 2 à 14.
const CARD_POINT: Setting =
    Setting { var: 0x85, word: false, step: 1, max: 0xe, saved: saved::CARD_POINT, cell: 0x912b54, show: text(0x2c71a, 16, 16) };
const DIFFICULTY: Setting =
    Setting { var: 0x2a, word: false, step: 1, max: 7, saved: saved::DIFFICULTY, cell: 0x912a1c, show: text(0x2c862, 20, 20) };

/// Ce que fait une ligne de sous-menu.
#[derive(Clone, Copy)]
enum Line {
    Coin,
    Chutes,
    Setting(Setting),
    Exit,
}

/// Lignes de 7-1 selon la variante (tables 0x2be98, 0x2bea6, 0x2beb6).
fn system_lines(variant: Variant) -> &'static [Line] {
    use Line::*;
    const PLAIN: &[Line] =
        &[Coin, Chutes, Setting(CONTINUE), Setting(MONITOR), Setting(DEMO_SOUND), Setting(SOUND_MODE), Exit];
    const LANGUAGE_LINES: &[Line] =
        &[Coin, Chutes, Setting(CONTINUE), Setting(MONITOR), Setting(DEMO_SOUND), Setting(SOUND_MODE), Setting(LANGUAGE), Exit];
    const CARD_LINES: &[Line] = &[
        Coin,
        Chutes,
        Setting(CONTINUE),
        Setting(MONITOR),
        Setting(DEMO_SOUND),
        Setting(SOUND_MODE),
        Setting(CARD),
        Setting(CARD_POINT),
        Exit,
    ];
    match variant {
        Variant::Plain => PLAIN,
        Variant::Language => LANGUAGE_LINES,
        Variant::Cards(_) => CARD_LINES,
    }
}

fn read(x: &mut Ctx, s: &Setting) -> u8 {
    if s.word { x.v16(s.var) as u8 } else { x.v8(s.var) }
}

fn write(x: &mut Ctx, s: &Setting, value: u8) {
    if s.word {
        x.set_v16(s.var, value as u16);
    } else {
        x.set_v8(s.var, value);
    }
}

/// Affiche la valeur du réglage (`0x2ca46` / `0x2ca12`), en couleur si elle diffère de la valeur d'entrée.
fn draw(x: &mut Ctx, s: &Setting) {
    let value = read(x, s);
    let width = match s.show {
        Show::Text { table, stride, width } => {
            let from = table + stride * value as u32;
            for k in 0..width {
                let ch = x.rd8(from + k) as u16;
                x.wr16(s.cell + 0x80 * k, ch);
                x.wr16(s.cell + 0x80 * k + 2, 0);
            }
            width
        }
        Show::OnOff => {
            let word: &[u8; 3] = if value != 0 { b"ON " } else { b"OFF" };
            for (k, &ch) in word.iter().enumerate() {
                crate::mem::wr32(x.b, s.cell + 0x80 * k as u32, (ch as u32) << 16);
            }
            3
        }
    };
    let changed = if s.word { x.v16(s.var) != x.rd16(s.saved) } else { value != x.rd8(s.saved) };
    if changed {
        paint(x, s.cell, HIGHLIGHT, width - 1);
    }
}

/// Gauche/droite, sinon tir 1/tir 2 : la nouvelle valeur si elle a changé.
fn edit(x: &mut Ctx, value: u8, step: u8, max: u8) -> Option<u8> {
    match horizontal(x, value, step, max) {
        Some(v) => Some(v),
        None => shots(x, value, step, max),
    }
}

/// Un réglage ordinaire (`0x2c44e`, `0x2c498`...).
fn setting(x: &mut Ctx, s: &Setting) {
    let current = read(x, s);
    let Some(mut value) = edit(x, current, s.step, s.max) else { return };
    match s.var {
        0x72 if value == 0 => value = if x.v16(s.var) as i16 > 2 { 2 } else { 6 },
        0x85 if (value as i8) <= 1 => value = if x.v8(s.var) as i8 > 2 { 2 } else { 0xe },
        v::CONTINUE => crate::mem::wr8(x.b, saved::CONTINUE_CHOSEN, value),
        _ => {}
    }
    write(x, s, value);
    draw(x, s);
}

/// `0x2c2a4` : type de monnayeurs (0-2 avec les cartes, 0-11 sinon).
fn chutes(x: &mut Ctx, variant: Variant) {
    let max = if let Variant::Cards(_) = variant { 2 } else { CHUTES.max };
    let current = read(x, &CHUTES);
    let Some(value) = edit(x, current, 1, max) else { return };
    write(x, &CHUTES, value);
    draw(x, &CHUTES);
}

/// `0x2bf86` : mode de monnayage. Avec les cartes, la valeur choisie indexe une liste réduite de modes
/// (0x2c008 / 0x2c00c). Le départ gratuit force CONTINUE à ON ; sinon CONTINUE reprend la valeur choisie.
fn coin_mode(x: &mut Ctx, variant: Variant) {
    let max = match variant {
        Variant::Cards(4) => 2,
        Variant::Cards(_) => 3,
        _ => COIN_MODE.max,
    };
    let current = read(x, &COIN_MODE);
    let Some(value) = edit(x, current, 1, max) else { return };
    let cont = if value == v::FREE_START { 1 } else { x.rd8(saved::CONTINUE_CHOSEN) };
    x.set_v8(v::CONTINUE, cont);
    draw(x, &CONTINUE);
    let mode = match variant {
        Variant::Cards(4) => x.prog8(0x2c008 + if value as i8 > 2 { 1 } else { value as u32 }),
        Variant::Cards(_) => x.prog8(0x2c00c + if value as i8 > 3 { 2 } else { value as u32 }),
        _ => value,
    };
    x.set_v8(v::COIN_MODE, mode);
    draw(x, &COIN_MODE);
}

/// `0x2c9fe` : tir 1 revient au menu de configuration.
fn exit_line(x: &mut Ctx) -> Flow {
    if pressed(x) & key::SHOT1 != 0 {
        show_menu(x);
    }
    Flow::Done
}

// ---------------------------------------------------------------------------------------------
// Le menu de configuration

pub fn configuration(x: &mut Ctx) -> Flow {
    match get(x, f::STEP) {
        0 => {
            set(x, field::LINE, line::SYSTEM);
            remember(x);
            show_menu(x);
            Flow::Done
        }
        2 => menu(x),
        _ => match get(x, field::LINE) {
            line::SYSTEM => system(x),
            _ => game(x),
        },
    }
}

/// `0x2bb22` : valeurs d'entrée des réglages.
fn remember(x: &mut Ctx) {
    let bytes = [
        (saved::COIN_MODE, v::COIN_MODE),
        (saved::CHUTES, 0x1d),
        (saved::CONTINUE, v::CONTINUE),
        (saved::CONTINUE_CHOSEN, v::CONTINUE),
        (saved::MONITOR, 0x26),
        (saved::DEMO_SOUND, 0x27),
        (saved::SOUND_MODE, 0x28),
        (saved::BYTE_29, 0x29),
    ];
    for (to, from) in bytes {
        let b = x.v8(from);
        crate::mem::wr8(x.b, to, b);
    }
    let language = x.v16(LANGUAGE.var);
    x.wr16(saved::LANGUAGE, language);
    for (to, from) in [(saved::CARD, 0x84), (saved::CARD_POINT, 0x85), (saved::DIFFICULTY, 0x2a), (saved::BYTE_2B, 0x2b)] {
        let b = x.v8(from);
        crate::mem::wr8(x.b, to, b);
    }
}

/// `0x2bb70` : écran du menu de configuration.
fn show_menu(x: &mut Ctx) {
    set(x, f::STEP, 2);
    x.c.d[0] = 0;
    x.wr16(super::MENU + field::SUB_STEP, 0);
    for f in [ext::CLEAR_OBJECTS, ext::CLEAR_SCROLL_1, ext::CLEAR_SCROLL_2, ext::CLEAR_SCROLL_3, ext::PALETTES] {
        x.call(f);
    }
    super::full_brightness(x);
    // compteurs de répétition : D0 tel que le laisse le chargement des palettes
    let d0 = x.c.d[0] as u16;
    super::set16(x, f::REPEAT_V, d0);
    set(x, f::REPEAT_SHOTS, 0x7f);
    for t in [text::TITLE, text::MENU, text::SELECT, text::HELP_START] {
        print(x, t);
    }
    if !config::is_factory(x) {
        paint(x, SAVE_LINE, HIGHLIGHT, 6);
    }
    let line = get(x, field::LINE);
    arrow(x, MENU_ARROW, line);
}

/// `0x2bc22` : choix d'une ligne, puis SYSTEM / GAME (tir 1), DEFAULT (départ 1P + tir 1), SAVE & EXIT (tir 1).
fn menu(x: &mut Ctx) -> Flow {
    let line = get(x, field::LINE);
    if let Some(to) = vertical(x, line, 2, line::SAVE) {
        move_arrow(x, MENU_ARROW, line, to);
        set(x, field::LINE, to);
        let help = match to as i8 {
            l if l <= line::GAME as i8 => text::HELP_START,
            l if l == line::DEFAULT as i8 => text::HELP_RESET,
            _ => text::HELP_EXIT,
        };
        print(x, help);
    }
    let held = x.v8(var::INPUT);
    if held == 0 {
        return Flow::Done;
    }
    let new = pressed(x);
    match get(x, field::LINE) {
        l if l as i8 <= line::GAME as i8 => {
            if new & key::SHOT1 != 0 {
                super::next_step(x);
            }
        }
        line::DEFAULT => {
            if x.v8(var::SYSTEM) & key::START1 != 0 && held & key::SHOT1 != 0 {
                config::reset_factory(x);
                let cont = x.v8(v::CONTINUE);
                crate::mem::wr8(x.b, saved::CONTINUE_CHOSEN, cont);
                paint(x, SAVE_LINE, 0, 6);
            }
        }
        line::SAVE => {
            if new & key::SHOT1 != 0 {
                layers_off(x);
                x.call(ext::CLEAR_SCROLL_1);
                print(x, text::SAVING);
                layers_on(x);
                if !config::save(x.c, x.b) {
                    return Flow::Yield;
                }
                return back_to_menu(x);
            }
        }
        _ => {}
    }
    Flow::Done
}

/// Choix d'une ligne de sous-menu (haut, bas) ; la dernière (`exit`) a son propre texte d'aide. Rend vrai
/// si la ligne a changé.
fn sub_select(x: &mut Ctx, exit: u8, skip_continue: bool) -> bool {
    let line = get(x, field::SUB_LINE);
    let Some(mut to) = vertical(x, line, 2, exit) else { return false };
    if skip_continue && to == 4 {
        to = if (to as i8) > (line as i8) { to + 2 } else { to - 2 };
    }
    move_arrow(x, SUB_ARROW, line, to);
    set(x, field::SUB_LINE, to);
    print(x, if to == exit { text::SUB_HELP_EXIT } else { text::SUB_HELP });
    true
}

/// Entrée d'un sous-menu : titre, lignes, aide, flèche sur la première ligne.
fn sub_screen(x: &mut Ctx, texts: &[u32]) {
    let s = get(x, field::SUB_STEP).wrapping_add(2);
    set(x, field::SUB_STEP, s);
    set(x, field::SUB_LINE, 0);
    x.call(ext::CLEAR_SCROLL_1);
    for &t in texts {
        print(x, t);
    }
    print(x, text::SUB_SELECT);
    print(x, text::SUB_HELP);
    arrow(x, SUB_ARROW, 0);
}

/// `0x2bd2a` : 7-1 SYSTEM CONFIGURATION.
fn system(x: &mut Ctx) -> Flow {
    let variant = variant(x);
    if get(x, field::SUB_STEP) == 0 {
        let lines = match variant {
            Variant::Language => text::SYSTEM_LINES_LANGUAGE,
            Variant::Cards(_) => text::SYSTEM_LINES_CARDS,
            Variant::Plain => text::SYSTEM_LINES,
        };
        sub_screen(x, &[text::SYSTEM_TITLE, lines]);
        for s in [COIN_MODE, CHUTES, CONTINUE, MONITOR, DEMO_SOUND, SOUND_MODE] {
            draw(x, &s);
        }
        match variant {
            Variant::Language => {
                draw(x, &LANGUAGE);
                return Flow::Done;
            }
            // l'original enchaîne sur le choix d'une ligne dans la même image
            Variant::Cards(_) => {
                draw(x, &CARD);
                draw(x, &CARD_POINT);
            }
            Variant::Plain => return Flow::Done,
        }
    }
    let lines = system_lines(variant);
    let exit = 2 * (lines.len() as u8 - 1);
    let free_start = x.v8(v::COIN_MODE) == v::FREE_START;
    if sub_select(x, exit, free_start) {
        return Flow::Done;
    }
    match lines[get(x, field::SUB_LINE) as usize / 2] {
        Line::Coin => coin_mode(x, variant),
        Line::Chutes => chutes(x, variant),
        Line::Setting(s) => setting(x, &s),
        Line::Exit => return exit_line(x),
    }
    Flow::Done
}

/// `0x2bec8` : 7-2 GAME CONFIGURATION (difficulté).
fn game(x: &mut Ctx) -> Flow {
    if get(x, field::SUB_STEP) == 0 {
        sub_screen(x, &[text::GAME_TITLE, text::GAME_LINES]);
        draw(x, &DIFFICULTY);
        return Flow::Done;
    }
    if sub_select(x, 2, false) {
        return Flow::Done;
    }
    if get(x, field::SUB_LINE) == 0 {
        setting(x, &DIFFICULTY);
        Flow::Done
    } else {
        exit_line(x)
    }
}
