//! Dessin du choix du personnage et du nom du joueur : lettres de sélection, noms, objets du classement,
//! portrait du personnage choisi.

use cps2::Cps2Bus;

use super::draw::{
    character_name, character_palettes, choice_palette, clear_row, face, narrow, player_attr, zone, zone_brush,
};
use super::layer::{columns, Brush, COLUMN, EMPTY};
use super::{panel, player, v8, var, Slot};
use crate::mem::{prog16, prog32, prog8, rd16, rd32, rd8, wr32, A5};

/// L'octet `+0x15d` du joueur : famille de la sélection (2 : aucune) ; l'entrée de sa table de lettres dit
/// si le personnage a une image propre (octet nul : attribut 0xc).
fn selection_row(b: &mut Cps2Bus, s: Slot) -> Option<u8> {
    let kind = s.g8(b, 0x15d);
    if kind == 2 {
        return None;
    }
    let entry = rd8(b, s.player + 0x120 + ((kind as u32) << 4) + 8);
    Some(rd8(b, s.player + 0x150 + entry as u32))
}

/// Bande de lettres de sélection (16 cases) : tuile et attribut de départ pour la classe `selector`, deux
/// tables (selon `A5+0x70`) de 8 octets par classe ; une tuile négative vide la bande.
const SELECTION_LETTERS: [u32; 2] = [0x16220, 0x164d8];

/// 0x16790 : la bande de lettres de sélection (16 cases en ligne), vide si le joueur n'a pas de sélection
/// (`+0x15c` nul) ou si l'entrée de la table est négative.
pub fn selection_letters(b: &mut Cps2Bus, s: Slot, selector: u8) {
    if s.p8(b, panel::DISPLAYED_A) == 0xfe {
        return;
    }
    let mut at = zone_brush(b, s, zone::SELECTION).at;
    let alternate = rd16(b, A5 + 0x70) != 0;
    let entry = SELECTION_LETTERS[alternate as usize] + ((selector as u32) << 3);
    if s.g8(b, 0x15c) == 0 || (rd16(b, entry) as i16) < 0 {
        if rd8(b, var::FROZEN) != 0 {
            return;
        }
        for _ in 0..16 {
            wr32(b, at, EMPTY);
            at += COLUMN;
        }
        return;
    }
    let mut tile = rd16(b, entry);
    let mut attr = rd16(b, entry + 2);
    if selection_row(b, s) == Some(0) {
        attr = 0xc;
    }
    let mut brush = Brush::new(b, at);
    for _ in 0..16 {
        brush.cell(tile, attr);
        brush.skip_column_after_cell();
        tile = tile.wrapping_add(1);
    }
}

/// Tuile et attribut de départ du nom de sélection, par classe (relatifs au PC).
const SELECTION_NAMES: u32 = 0x160c4;

/// 0x16038 : le nom de la sélection pour la classe `selector` (4 colonnes de 2 cases).
pub fn selection_name(b: &mut Cps2Bus, s: Slot, selector: u8) {
    let entry = SELECTION_NAMES + 4 * selector as u32;
    let mut tile = prog16(b, entry);
    let mut attr = prog16(b, entry + 2);
    if tile == 0xd40 && (s.g8(b, player::CHARACTER) >> 1) != 4 {
        tile = 0xc48;
        attr = 0x13;
    }
    if s.g8(b, 0x15c) == 0 && v8(b, var::SETTINGS) & 0x20 != 0 {
        tile = tile.wrapping_add(4);
    }
    if selection_row(b, s) == Some(0) {
        attr = 0xc;
    }
    let mut brush = zone_brush(b, s, zone::NAME_LETTERS);
    for _ in 0..4 {
        brush.cell(tile, attr);
        brush.cell(tile.wrapping_add(0x10), attr);
        brush.next_column(2);
        tile = tile.wrapping_add(1);
    }
}

/// Pointeurs des tables de lettres (une par position du nom), lus relativement au PC.
const LETTER_TABLES: u32 = 0x16e0a;

/// 0x16dac : les huit lettres du nom du joueur (`player+0x160`), chacune de `width` colonnes de trois cases,
/// à partir de `at` ; `shift` décale les tuiles (colonnes étroites). Rend la case qui suit les lettres.
fn name_letters(b: &mut Cps2Bus, s: Slot, at: u32, width: u32, shift: u16) -> u32 {
    let (first, second) = (s.g32(b, 0x160), s.g32(b, 0x164));
    s.set_p32(b, panel::SHOWN_NAME, first);
    s.set_p32(b, panel::SHOWN_NAME + 4, second);
    let number = s.g8(b, player::NUMBER) as u16;
    let mut brush = Brush::new(b, at);
    for position in 0..8u32 {
        let letter = s.g8(brush.b, 0x160 + position);
        let entry = (letter.wrapping_add(1) as u16).wrapping_mul(4) as u32;
        let table = prog32(brush.b, LETTER_TABLES + 4 * position);
        // une lettre vide (0xff) prend la palette du joueur
        let attr_base = if entry == 0 { number } else { 0 };
        let mut tile = rd16(brush.b, table + entry).wrapping_add(shift);
        let attr = attr_base.wrapping_add(rd16(brush.b, table + entry + 2));
        for _ in 0..width {
            for row in 0..3 {
                brush.cell(tile.wrapping_add(0x10 * row), attr);
            }
            tile = tile.wrapping_add(1);
            brush.at += COLUMN - 12;
        }
    }
    brush.at
}

/// 0x16d1a : le nom du joueur dans les dispositions à plusieurs joueurs (pas pendant la phase 4).
pub fn names(b: &mut Cps2Bus, s: Slot) {
    if v8(b, var::PHASE) == 4 || v8(b, var::LAYOUT) == 0 {
        return;
    }
    let at = zone_brush(b, s, zone::FRAME).at + 0x64;
    if narrow(b) {
        name_letters(b, s, at, 2, 3);
    } else {
        name_letters(b, s, at, 3, 0);
    }
}

/// 0x16d5a : le nom du joueur dans la zone de sélection ; les joueurs 3 et 4 effacent d'abord leurs lignes.
pub fn names_alt(b: &mut Cps2Bus, s: Slot) {
    let mut at = zone_brush(b, s, zone::NAMES_ALT).at;
    let frozen = rd8(b, var::FROZEN) != 0;
    if s.index(b) < 2 {
        at = name_letters(b, s, at, 3, 0).wrapping_sub(0xbf4);
        if !frozen {
            at = clear_row(b, at);
            clear_row(b, at);
        }
        return;
    }
    if frozen {
        at += 8;
    } else if v8(b, 0x50) == 0 {
        at = clear_row(b, at);
        at = clear_row(b, at);
    } else {
        at = clear_row(b, at + 4);
    }
    name_letters(b, s, at, 3, 0);
}

/// Tuiles des objets du classement (2x2 cases par code) et des codes spéciaux, tables du code.
const RANK_TILES: u32 = 0x169a4;
const RANK_SPECIAL_TILES: u32 = 0x16aa8;
const RANK_BLANK: u16 = 0x94c;

/// 0x16910 : les six objets du joueur dans la zone du classement (2x2 cases chacun).
pub fn rank_items(b: &mut Cps2Bus, s: Slot) {
    let attr = 0x13 + s.index(b) as u16;
    let codes: Vec<u8> = (0..6).map(|k| s.g8(b, player::ITEMS + k)).collect();
    let mut brush = zone_brush(b, s, zone::RANKS);
    let put = |brush: &mut Brush, tile: u16| {
        for column in 0..2u16 {
            brush.cell(tile.wrapping_add(column), attr);
            brush.cell(tile.wrapping_add(column).wrapping_add(0x10), attr);
            brush.next_column(2);
        }
    };
    for (k, &code) in codes.iter().enumerate() {
        if code == 0xff {
            put(&mut brush, RANK_BLANK);
        } else if code >= 0xf9 {
            let index = (code.wrapping_neg().wrapping_sub(2) as u16 as u32) * 2;
            let mut tile = prog16(brush.b, RANK_SPECIAL_TILES + index);
            for _ in k..6 {
                put(&mut brush, tile);
                tile = tile.wrapping_add(2);
            }
            return;
        } else {
            let tile = prog16(brush.b, RANK_TILES + 2 * code as u32);
            put(&mut brush, tile);
        }
    }
}

/// Pointeurs des images de portraits de choix (par personnage et variante), une série par langue.
const CHOICE_PORTRAITS: [u32; 2] = [0x16c26, 0x16c56];
/// Classe de chaque personnage (octet, lu relativement au PC).
const CLASS_OF_CHARACTER: u32 = 0x16c14;

/// 0x16b8a : le personnage `character` choisi : son portrait, ses palettes, son nom et son visage.
pub fn show_character(b: &mut Cps2Bus, s: Slot, character: u8) {
    s.set_g8(b, player::CHARACTER, character);
    let mut offset = ((character & !1) as u32) * 2;
    let mut width = 20;
    if narrow(b) {
        offset += 0x18;
        width = 12;
    }
    let alternate = rd16(b, A5 + 0x70) != 0;
    let picture = rd32(b, CHOICE_PORTRAITS[alternate as usize] + (offset & 0xff));
    let attr = player_attr(b, s);
    let mut brush = zone_brush(b, s, zone::PORTRAIT);
    brush.at += 0x200;
    columns(&mut brush, picture, width, attr);
    character_palettes(b, s);
    choice_palette(b, s);
    let choice = s.p8(b, panel::CHOICE);
    character_name(b, s, choice);
    let locked = s.g8(b, player::CHARACTER) & !1;
    s.set_g8(b, player::CHARACTER, locked);
    let class = prog8(b, CLASS_OF_CHARACTER + (choice >> 1) as u32);
    face(b, s, class);
}
