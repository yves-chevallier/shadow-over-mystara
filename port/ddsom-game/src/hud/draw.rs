//! Dessin des panneaux sur la couche de caractères : bannières, portraits, chiffres, jauge de PV, noms.
//!
//! Chaque zone d'un panneau (`zone::*`) est un rectangle de cases dont l'origine vient de `layer::origin`.
//! Les tuiles viennent de tables de la ROM ; les attributs de palette dépendent du joueur (8 + numéro).

use cps2::Cps2Bus;

use super::layer::{columns, origin, Brush, COLUMN, EMPTY};
use super::{panel, player, v8, var, Slot};
use crate::mem::{prog16, rd16, rd32, rd8, wr16, wr32};

/// Zones d'un panneau (index de ligne de la table des origines).
pub mod zone {
    /// Nom du personnage choisi (4x2 cases).
    pub const NAME_LETTERS: u16 = 1;
    /// Jauge de PV.
    pub const HP_BAR: u16 = 2;
    pub const CHARACTER_NAME: u16 = 3;
    /// Bannière « INSERT COIN / PUSH START » effacée avant de dessiner une autre.
    pub const BANNER: u16 = 4;
    /// Compte à rebours du choix.
    pub const COUNTDOWN: u16 = 5;
    /// Chiffres du compte à rebours de reprise.
    pub const CONTINUE_DIGITS: u16 = 6;
    pub const PORTRAIT: u16 = 7;
    pub const FACE: u16 = 8;
    pub const ITEMS: u16 = 9;
    pub const MONEY: u16 = 0xa;
    pub const SCORE: u16 = 0xb;
    /// Grand cadre du joueur (effacé quand il quitte).
    pub const FRAME: u16 = 0xc;
    pub const SELECTION: u16 = 0xd;
    pub const NAMES_ALT: u16 = 0xe;
    pub const START_BANNER: u16 = 0xf;
    pub const ITEMS_ROW: u16 = 0x10;
    pub const RANKS: u16 = 0x11;
    pub const CLASS_ICON: u16 = 0x12;
}

/// Attribut de palette d'un joueur.
pub fn player_attr(b: &mut Cps2Bus, s: Slot) -> u16 {
    8 + s.index(b) as u16
}

/// Colonnes étroites (disposition à 2 colonnes, `A5+0x344` impair).
pub fn narrow(b: &mut Cps2Bus) -> bool {
    v8(b, var::LAYOUT) & 1 != 0
}

pub(super) fn zone_brush<'a>(b: &'a mut Cps2Bus, s: Slot, zone: u16) -> Brush<'a> {
    let index = s.index(b);
    let at = origin(b, index, zone);
    Brush::new(b, at)
}

/// Efface un rectangle de `columns` colonnes et `rows` lignes à partir de `at`.
fn clear_rect(b: &mut Cps2Bus, at: u32, rows: u32, columns: u32) {
    for column in 0..columns {
        for row in 0..rows {
            wr32(b, at + column * COLUMN + row * 4, EMPTY);
        }
    }
}

/// 0x16c86 : efface les deux bandes de la couche réservées aux panneaux (haut et bas de l'écran).
pub fn clear_panels(b: &mut Cps2Bus) {
    clear_rect(b, 0x91_2008, 4, 48);
    clear_rect(b, 0x91_2068, 4, 48);
}

// ---------------------------------------------------------------------------------------------
// Bannières
// ---------------------------------------------------------------------------------------------

/// 0x17542 : efface la bannière du panneau (3 cases de haut : 24 colonnes, ou deux fois 16 colonnes en
/// disposition à 3 joueurs ; la seconde moitié vient 25 lignes plus bas).
pub fn clear_banner(b: &mut Cps2Bus, s: Slot) {
    const WIDE: u32 = 0x3f_55e2;
    const HALF: u32 = 0x3f_6758;
    const ATTR: u16 = 0xe;
    let layout = v8(b, var::LAYOUT);
    let mut brush = zone_brush(b, s, zone::BANNER);
    let start = brush.at;
    let (table, n) = if layout == 1 { (HALF, 16) } else { (WIDE, 24) };
    columns(&mut brush, table, n, ATTR);
    if layout != 0 {
        brush.at = start + 0x64;
        columns(&mut brush, table, n, ATTR);
    }
}

/// 0x17400 / 0x17458 : bannière « PUSH START » (après effacement de l'ancienne), versions large et étroite.
pub fn start_banner(b: &mut Cps2Bus, s: Slot, tables: (u32, u32)) {
    clear_banner(b, s);
    let (table, n) = if narrow(b) { (tables.1, 12) } else { (tables.0, 17) };
    let mut brush = zone_brush(b, s, zone::START_BANNER);
    columns(&mut brush, table, n, 0xe);
}

pub const START_BANNER_A: (u32, u32) = (0x3f_5c06, 0x3f_6710);
pub const START_BANNER_B: (u32, u32) = (0x3f_5c6c, 0x3f_66c8);

/// Une des bannières « INSERT COIN » (tables de pointeurs dans le code, une colonne de 3 mots par entrée) :
/// `source` est l'adresse du pointeur de table, `rows` le nombre de colonnes moins un.
pub fn pointer_banner(b: &mut Cps2Bus, s: Slot, pointer: u32, columns_count: u32, attr: u16) {
    let table = rd32(b, pointer);
    let mut brush = zone_brush(b, s, zone::START_BANNER);
    columns(&mut brush, table, columns_count, attr);
}

// ---------------------------------------------------------------------------------------------
// Chiffres
// ---------------------------------------------------------------------------------------------

/// 0x159fc : le compte à rebours du choix (deux chiffres BCD, une case chacun).
pub fn countdown_digits(b: &mut Cps2Bus, s: Slot) {
    let value = s.p8(b, panel::COUNTDOWN);
    let attr = player_attr(b, s);
    let mut brush = zone_brush(b, s, zone::COUNTDOWN);
    for digit in [value >> 4, value & 0xf] {
        brush.cell(0x65c + digit as u16, attr);
        brush.skip_column_after_cell();
    }
}

/// 0x1749e : les deux chiffres du compte à rebours de reprise (grands : deux cases de haut, ou une seule en
/// colonnes étroites). Le chiffre des dizaines utilise des tuiles dans l'ordre inverse.
pub fn continue_digits(b: &mut Cps2Bus, s: Slot) {
    let value = s.p8(b, panel::COUNTDOWN);
    let narrow = narrow(b);
    let brush = zone_brush(b, s, zone::CONTINUE_DIGITS);
    let (tens, units) = ((value >> 4) as u16, (value & 0xf) as u16);
    let at = brush.at;
    let (tens_tile, units_tile) = if narrow { (0x641u16.wrapping_sub(tens), 0x642 + units) } else { (0x621u16.wrapping_sub(tens), 0x622 + units) };
    for (column, tile) in [(0, tens_tile), (1, units_tile)] {
        let cell = at + column * COLUMN;
        wr16(brush.b, cell, tile);
        wr16(brush.b, cell + 2, 0xe);
        if !narrow {
            wr16(brush.b, cell + 4, tile + 0x10);
            wr16(brush.b, cell + 6, 0xe);
        }
    }
}

/// Un chiffre décimal à l'écran : nul non significatif (tant qu'aucun chiffre n'a été écrit) remplacé par
/// une case vide. Rend la position suivante.
struct Digits<'a> {
    brush: Brush<'a>,
    attr: u16,
    base: u16,
    started: bool,
}

impl Digits<'_> {
    fn put(&mut self, nibble: u8) {
        let nibble = nibble & 0xf;
        if nibble == 0 && !self.started {
            self.brush.cell(0x8e1, self.attr);
        } else {
            self.brush.cell(self.base + nibble as u16, self.attr);
            self.started = true;
        }
        self.brush.skip_column_after_cell();
    }

    fn byte(&mut self, value: u8) {
        self.put(value >> 4);
        self.put(value);
    }
}

/// 0x1725e : le score (joueur `+0x110`, sept chiffres BCD) ; rend la case qui suit le dernier chiffre.
pub fn score(b: &mut Cps2Bus, s: Slot) -> u32 {
    let value = s.g32(b, player::SCORE);
    s.set_p32(b, panel::SHOWN_SCORE, value);
    let attr = player_attr(b, s);
    let [b0, b1, b2, b3] = value.to_be_bytes();
    let brush = zone_brush(b, s, zone::SCORE);
    let mut digits = Digits { brush, attr, base: 0x960, started: false };
    digits.put(b0);
    digits.byte(b1);
    digits.byte(b2);
    // le dernier octet est écrit en entier, zéros compris
    digits.put(b3 >> 4);
    digits.started = true;
    digits.put(b3);
    digits.brush.at
}

/// 0x171da : l'argent (joueur `+0x114`) ; en colonnes étroites deux cases de remplissage précèdent les chiffres.
/// Rend la case qui suit le dernier chiffre.
pub fn money(b: &mut Cps2Bus, s: Slot) -> u32 {
    let value = s.g32(b, player::MONEY);
    s.set_p32(b, panel::SHOWN_MONEY, value);
    let attr = player_attr(b, s);
    let narrow = narrow(b);
    let [_, b1, b2, b3] = value.to_be_bytes();
    let mut brush = zone_brush(b, s, zone::MONEY);
    if narrow {
        for _ in 0..2 {
            brush.cell(0x8e1, attr);
            brush.skip_column_after_cell();
        }
    }
    let mut digits = Digits { brush, attr, base: 0x970, started: false };
    digits.put(b1);
    digits.byte(b2);
    digits.put(b3 >> 4);
    digits.started = true;
    digits.put(b3);
    digits.brush.at
}

// ---------------------------------------------------------------------------------------------
// Portraits et identité
// ---------------------------------------------------------------------------------------------

/// Tables de pointeurs de portraits (une par joueur) du code.
const PORTRAIT_POINTERS: [u32; 2] = [0x17b96, 0x17ba6];

/// 0x17096 / 0x170e0 : portrait du joueur (3 cases de haut) ; `alt` choisit la seconde série d'images.
pub fn portrait(b: &mut Cps2Bus, s: Slot, alt: bool) {
    let attr = player_attr(b, s);
    let index = s.index(b) as u32;
    let (table, n) = if narrow(b) {
        (if alt { 0x3f_6668 } else { 0x3f_6ba8 }, 16)
    } else {
        (rd32(b, PORTRAIT_POINTERS[alt as usize] + 4 * index), 24)
    };
    let mut brush = zone_brush(b, s, zone::PORTRAIT);
    columns(&mut brush, table, n, attr);
}

/// Tuile de la première case de chaque ligne du nom (par personnage et variante), table du code.
const NAME_TILES: u32 = 0x16fe2;

/// 0x17012 / 0x1701c : le nom du personnage `character` (4 cases de large, 2 de haut).
pub fn character_name(b: &mut Cps2Bus, s: Slot, character: u8) {
    let attr = 4 + s.index(b) as u16;
    let mut tile = prog16(b, NAME_TILES.wrapping_add_signed(2 * character as i8 as i32));
    let mut brush = zone_brush(b, s, zone::CHARACTER_NAME);
    let start = brush.at;
    for row in 0..2 {
        brush.at = start + 4 * row;
        for _ in 0..4 {
            brush.cell(tile, attr);
            brush.skip_column_after_cell();
            tile = tile.wrapping_add(1);
        }
        tile = tile.wrapping_add(0xc);
    }
}

/// Tuiles de la bande d'icône de classe (par personnage).
const CLASS_ICON_TILES: u32 = 0x1707c;

/// 0x17034 : icône de la classe du personnage du joueur (bande de 6 cases, ou une case en colonnes étroites).
pub fn class_icon(b: &mut Cps2Bus, s: Slot) {
    let character = s.g8(b, player::CHARACTER) & !1;
    let attr = player_attr(b, s);
    let narrow = narrow(b);
    let mut brush = zone_brush(b, s, zone::CLASS_ICON);
    if narrow {
        brush.cell(0x96a + (character >> 1) as u16, attr);
    } else {
        let mut tile = prog16(brush.b, CLASS_ICON_TILES + character as u32);
        for _ in 0..6 {
            brush.cell(tile, attr);
            brush.skip_column_after_cell();
            tile = tile.wrapping_add(1);
        }
    }
}

/// Tuiles des visages (deux cases par classe, lues dans le code comme des données).
const FACE_TILES: u32 = 0x17136;

/// 0x17106 : le visage de la classe `class` (8 à 15), deux cases en largeur.
pub fn face(b: &mut Cps2Bus, s: Slot, class: u8) {
    let attr = player_attr(b, s);
    let tiles = FACE_TILES + 4 * (class.wrapping_sub(8) as u32);
    let mut brush = zone_brush(b, s, zone::FACE);
    for k in 0..2 {
        let tile = rd16(brush.b, tiles + 2 * k);
        brush.cell(tile, attr);
        brush.skip_column_after_cell();
    }
}

/// Tuiles des objets spéciaux (codes 0xf9 à 0xfe d'une ligne d'objets), table du code.
const ITEM_SPECIAL_TILES: u32 = 0x171b4;

/// 0x17176 : les six objets du joueur (une case chacun ; un code spécial d'au moins 0xf9 remplit la fin de la
/// ligne d'un texte).
pub fn items(b: &mut Cps2Bus, s: Slot) {
    let attr = player_attr(b, s);
    let codes: Vec<u8> = (0..6).map(|k| s.g8(b, player::ITEMS + k)).collect();
    let mut brush = zone_brush(b, s, zone::ITEMS);
    for (k, &code) in codes.iter().enumerate() {
        if code == 0xff {
            brush.at += COLUMN;
        } else if code < 0xf9 {
            wr16(brush.b, brush.at, 0x860 + code as u16);
            wr16(brush.b, brush.at + 2, attr);
            brush.at += COLUMN;
        } else {
            let index = (code.wrapping_neg().wrapping_sub(2) as u16 as u32) * 2;
            let mut tile = prog16(brush.b, ITEM_SPECIAL_TILES + index);
            for _ in k..6 {
                brush.cell(tile, attr);
                brush.skip_column_after_cell();
                tile = tile.wrapping_add(1);
            }
            return;
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Effacements de zones
// ---------------------------------------------------------------------------------------------

/// 0x16f22 : efface le grand cadre du joueur.
pub fn clear_frame(b: &mut Cps2Bus, s: Slot) {
    let at = zone_brush(b, s, zone::FRAME).at;
    if narrow(b) {
        clear_rect(b, at, 5, 16);
    } else if rd8(b, var::FROZEN) != 0 {
        let at = if s.index(b) & 2 != 0 { at + 8 } else { at };
        clear_rect(b, at, 3, 24);
    } else if v8(b, 0x50) != 0 && s.index(b) & 2 != 0 {
        clear_rect(b, at + 4, 4, 24);
    } else {
        clear_rect(b, at, 5, 24);
    }
}

/// 0x16f9e : efface la ligne d'objets.
pub fn clear_items_row(b: &mut Cps2Bus, s: Slot) {
    let at = zone_brush(b, s, zone::ITEMS_ROW).at;
    let width = if narrow(b) { 16 } else { 24 };
    clear_rect(b, at, 4, width);
}

/// 0x16f0a : efface une ligne de 24 colonnes à `at` ; rend le début de la ligne suivante.
pub(super) fn clear_row(b: &mut Cps2Bus, at: u32) -> u32 {
    for column in 0..24 {
        wr32(b, at + column * COLUMN, EMPTY);
    }
    at + 4
}

// ---------------------------------------------------------------------------------------------
// Palettes
// ---------------------------------------------------------------------------------------------

/// 0x17b56 : recopie 8 longs (16 couleurs) de `source` vers `dest` en gardant la luminosité (quartets hauts)
/// du premier long de la destination.
pub fn blend_palette(b: &mut Cps2Bus, source: u32, dest: u32) {
    let keep = rd32(b, dest) & 0xf000_f000;
    for k in 0..8 {
        let color = rd32(b, source + 4 * k) & 0x0fff_0fff;
        wr32(b, dest + 4 * k, color | keep);
    }
}

/// Palette du joueur dans la page : 32 octets par palette.
pub fn blend_for(b: &mut Cps2Bus, s: Slot, source: u32, dest: u32) {
    let player = s.index(b) as u32;
    let character = s.g8(b, player::CHARACTER) as u32;
    blend_palette(b, source + 32 * character, dest + 32 * player);
}

/// 0x17b14 : les trois palettes du personnage du joueur.
pub fn character_palettes(b: &mut Cps2Bus, s: Slot) {
    blend_for(b, s, 0x25_c640, 0x91_c500);
    blend_for(b, s, 0x25_c4c0, 0x91_c480);
    blend_for(b, s, 0x25_c7c0, 0x91_c700);
}

/// 0x1796c : palette de la salle d'attente du joueur : `base` est le décalage propre au numéro du joueur
/// (table du code), auquel s'ajoute 32 octets par personnage.
pub fn lobby_palette(b: &mut Cps2Bus, s: Slot, base: u16) {
    let character = s.g8(b, player::CHARACTER) as u16;
    let source = base.wrapping_add(character << 5) as i16 as i32;
    let number = s.g8(b, player::NUMBER) as u16;
    let dest = ((0x13 + number) << 5) as i16 as i32;
    blend_palette(b, 0x25_c940u32.wrapping_add_signed(source), 0x91_c400u32.wrapping_add_signed(dest));
}

/// Case de fermeture après un nombre en colonnes étroites.
pub fn closing_cell(b: &mut Cps2Bus, s: Slot, at: u32, tile: u16) {
    let attr = player_attr(b, s);
    wr16(b, at, tile);
    wr16(b, at + 2, attr);
}

/// 0x17aee : palette du personnage en cours de choix.
pub fn choice_palette(b: &mut Cps2Bus, s: Slot) {
    let player = s.index(b) as u32;
    let choice = s.p8(b, panel::CHOICE) as u32;
    blend_palette(b, 0x25_c4c0 + 32 * choice, 0x91_c480 + 32 * player);
}

// ---------------------------------------------------------------------------------------------
// Jauge de PV
// ---------------------------------------------------------------------------------------------

/// Tuiles de la jauge : début d'un segment selon le reste (par cran de 0 à 7), pour quatre largeurs.
const BAR_TILES: [u32; 4] = [0x17a64, 0x17a76, 0x17a84, 0x17a8e];

/// 0x17a94 : une case de la jauge pour le reste `d0` (en pixels), entre les bornes `d1` et `d2`.
fn bar_cell(brush: &mut Brush, attr: u16, d0: i16, d1: i16, d2: i16) {
    let (limit, table) = if d0 >= 7 {
        (8, BAR_TILES[0])
    } else if d0 >= 5 {
        (6, BAR_TILES[1])
    } else if d0 >= 3 {
        (4, BAR_TILES[2])
    } else {
        (2, BAR_TILES[3])
    };
    let tile = prog16(brush.b, table.wrapping_add_signed(2 * d1 as i32));
    let mut extent = d2.wrapping_sub(d1) as u16;
    let room = (limit as u16).wrapping_sub(d1 as u16);
    if extent > room {
        extent = room;
    }
    brush.cell(tile.wrapping_add(extent), attr);
    brush.skip_column_after_cell();
}

/// 0x179c2 : la jauge de PV du panneau (un segment de 8 points de vie par case, un reste, puis le vide).
pub fn hp_bar(b: &mut Cps2Bus, s: Slot) {
    if s.p16(b, panel::HP_BAR) == 0 {
        return;
    }
    let attr = 0x18 + s.index(b) as u16;
    let mut d0 = s.p16(b, panel::SHOWN_HP_MAX) as i16;
    let mut d1 = s.g16(b, player::HP) as i16;
    let mut d2 = s.p16(b, panel::SHOWN_HP) as i16;
    if narrow(b) {
        d0 = ((d0 as u16) >> 1) as i16;
        d1 = ((d1 as u16) >> 1) as i16;
        d2 = ((d2 as u16) >> 1) as i16;
    }
    let mut brush = zone_brush(b, s, zone::HP_BAR);
    // segments pleins
    while d1 > 8 {
        d1 -= 8;
        d0 -= 8;
        d2 -= 8;
        brush.cell(0x82c, attr);
        brush.skip_column_after_cell();
    }
    bar_cell(&mut brush, attr, d0, d1, d2);
    if d2 != d1 {
        d0 -= 8;
        d2 -= 8;
        if d2 < 0 {
            d0 += 8;
        } else if d0 != d2 {
            // segments de PV perdus récemment
            loop {
                if d2 <= 8 {
                    bar_cell(&mut brush, attr, d0, 0, d2);
                    break;
                }
                d2 -= 8;
                d0 -= 8;
                brush.cell(0x808, attr);
                brush.skip_column_after_cell();
                if d0 == d2 {
                    break;
                }
            }
        }
    }
    // le reste de la jauge (PV maximum) : segments vides
    loop {
        d0 -= 8;
        if d0 <= 0 {
            break;
        }
        bar_cell(&mut brush, attr, d0, 0, 0);
    }
    s.set_p8(brush.b, panel::HP_DIRTY, 0);
}

