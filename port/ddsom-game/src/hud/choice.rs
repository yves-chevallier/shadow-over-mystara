//! Drawing of the character choice and the player name: selection letters, names, ranking items,
//! portrait of the chosen character.

use cps2::Cps2Bus;

use super::draw::{
    character_name, character_palettes, choice_palette, clear_row, face, narrow, player_attr, zone, zone_brush,
};
use super::layer::{clear_cell, columns, Brush, COLUMN};
use super::{panel, player, var, Slot};
use crate::screens::tables::{self, Rom};

/// Byte `+0x15d` of the player: selection family (2: none); its letter-table entry says
/// whether the character has its own image (zero byte: attribute 0xc).
fn selection_row(b: &mut Cps2Bus, s: Slot) -> Option<u8> {
    let kind = s.g8(b, player::SELECTION_KIND);
    if kind == 2 {
        return None;
    }
    let entry = s.g8(b, player::SELECTIONS + ((kind as u32) << 4) + 8);
    Some(s.g8(b, player::SELECTION_ROWS + entry as u32))
}

/// Selection letter strip (16 cells): start tile and attribute for the `selector` class, two
/// tables (depending on `A5+0x70`) of 8 bytes per class; a negative tile clears the strip.
const SELECTION_LETTERS: [Rom; 2] = [Rom::at(0x16220), Rom::at(0x164d8)];

/// 0x16790: the selection letter strip (16 cells in a row), empty if the player has no selection
/// (`+0x15c` zero) or if the table entry is negative.
pub fn selection_letters(b: &mut Cps2Bus, s: Slot, selector: u8) {
    if s.p8(b, panel::DISPLAYED_A) == 0xfe {
        return;
    }
    let mut at = zone_brush(b, s, zone::SELECTION).at;
    let alternate = var::LANGUAGE.get(b) != 0;
    let table = SELECTION_LETTERS[alternate as usize];
    let entry = (selector as u32) << 3;
    if s.g8(b, player::SELECTION_COUNT) == 0 || (table.word(b, entry) as i16) < 0 {
        if var::FROZEN.get(b) != 0 {
            return;
        }
        for _ in 0..16 {
            clear_cell(b, at);
            at += COLUMN;
        }
        return;
    }
    let mut tile = table.word(b, entry);
    let mut attr = table.word(b, entry + 2);
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

/// Start tile and attribute of the selection name, per class (PC-relative).
const SELECTION_NAMES: Rom = Rom::at(0x160c4);

/// 0x16038: the selection name for the `selector` class (4 columns of 2 cells).
pub fn selection_name(b: &mut Cps2Bus, s: Slot, selector: u8) {
    let entry = 4 * selector as u32;
    let mut tile = SELECTION_NAMES.pc_word(b, entry);
    let mut attr = SELECTION_NAMES.pc_word(b, entry + 2);
    if tile == 0xd40 && (s.g8(b, player::CHARACTER) >> 1) != 4 {
        tile = 0xc48;
        attr = 0x13;
    }
    if s.g8(b, player::SELECTION_COUNT) == 0 && var::SETTINGS.get(b) & 0x20 != 0 {
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

/// Pointers to the letter tables (one per name position), read PC-relative.
const LETTER_TABLES: Rom = Rom::at(0x16e0a);

/// 0x16dac: the eight letters of the player name (`player+0x160`), each `width` columns of three cells,
/// starting at `at`; `shift` offsets the tiles (narrow columns). Returns the cell following the letters.
fn name_letters(b: &mut Cps2Bus, s: Slot, at: u32, width: u32, shift: u16) -> u32 {
    let (first, second) = (s.g32(b, player::NAME), s.g32(b, player::NAME + 4));
    s.set_p32(b, panel::SHOWN_NAME, first);
    s.set_p32(b, panel::SHOWN_NAME + 4, second);
    let number = s.g8(b, player::NUMBER) as u16;
    let mut brush = Brush::new(b, at);
    for position in 0..8u32 {
        let letter = s.g8(brush.b, player::NAME + position);
        let entry = (letter.wrapping_add(1) as u16).wrapping_mul(4) as u32;
        let table = LETTER_TABLES.pc_long(brush.b, 4 * position);
        // an empty letter (0xff) takes the player's palette
        let attr_base = if entry == 0 { number } else { 0 };
        let mut tile = tables::word(brush.b, table + entry).wrapping_add(shift);
        let attr = attr_base.wrapping_add(tables::word(brush.b, table + entry + 2));
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

/// 0x16d1a: the player name in multi-player layouts (not during phase 4).
pub fn names(b: &mut Cps2Bus, s: Slot) {
    if var::PHASE.get(b) == 4 || var::LAYOUT.get(b) == 0 {
        return;
    }
    let at = zone_brush(b, s, zone::FRAME).at + 0x64;
    if narrow(b) {
        name_letters(b, s, at, 2, 3);
    } else {
        name_letters(b, s, at, 3, 0);
    }
}

/// 0x16d5a: the player name in the selection area; players 3 and 4 clear their rows first.
pub fn names_alt(b: &mut Cps2Bus, s: Slot) {
    let mut at = zone_brush(b, s, zone::NAMES_ALT).at;
    let frozen = var::FROZEN.get(b) != 0;
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
    } else if var::BOSS_ROW.get(b) == 0 {
        at = clear_row(b, at);
        at = clear_row(b, at);
    } else {
        at = clear_row(b, at + 4);
    }
    name_letters(b, s, at, 3, 0);
}

/// Tiles of the ranking items (2x2 cells per code) and of the special codes, code tables.
const RANK_TILES: Rom = Rom::at(0x169a4);
const RANK_SPECIAL_TILES: Rom = Rom::at(0x16aa8);
const RANK_BLANK: u16 = 0x94c;

/// 0x16910: the player's six items in the ranking area (2x2 cells each).
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
            let mut tile = RANK_SPECIAL_TILES.pc_word(brush.b, index);
            for _ in k..6 {
                put(&mut brush, tile);
                tile = tile.wrapping_add(2);
            }
            return;
        } else {
            let tile = RANK_TILES.pc_word(brush.b, 2 * code as u32);
            put(&mut brush, tile);
        }
    }
}

/// Pointers to the choice portrait images (per character and variant), one series per language.
const CHOICE_PORTRAITS: [Rom; 2] = [Rom::at(0x16c26), Rom::at(0x16c56)];
/// Class of each character (byte, read PC-relative).
const CLASS_OF_CHARACTER: Rom = Rom::at(0x16c14);

/// 0x16b8a: the chosen character `character`: its portrait, palettes, name and face.
pub fn show_character(b: &mut Cps2Bus, s: Slot, character: u8) {
    s.set_g8(b, player::CHARACTER, character);
    let mut offset = ((character & !1) as u32) * 2;
    let mut width = 20;
    if narrow(b) {
        offset += 0x18;
        width = 12;
    }
    let alternate = var::LANGUAGE.get(b) != 0;
    let picture = CHOICE_PORTRAITS[alternate as usize].long(b, offset & 0xff);
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
    let class = CLASS_OF_CHARACTER.pc_byte(b, (choice >> 1) as u32);
    face(b, s, class);
}
