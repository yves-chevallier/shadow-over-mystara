//! Tables of the screens: `data/screens.json` (`ddsom_data::GameData::screens`, read by `modes/`, `hud/`, `service/`,
//! `scene/`, `credits*.rs` and `popups.rs` of the game through `screens/tables.rs`; docs/fin.md, L11). The scene
//! tables already described by `data/scenes.json` (`scenes.rs`), the demonstration scripts of `data/misc.json`
//! (`misc.rs`) and the texts of `texts.json` (`texts.rs`) are read from those files.
//!
//! Same rules as the SET tables (`set.rs`): a table of fixed shape has its number of rows; the others run up to the
//! next table of the same space or the next instruction that the reference executes, at most 0x400 bytes.

use std::io;
use std::path::Path;

use ddsom_assets::data::DataFile;

use crate::data::{build, save, Spec};
use crate::rom::Program;
use crate::set::Tables;

/// Credits screen and credits task (`credits_screen.rs`, `credits_task.rs`).
fn credits(t: &mut Tables) {
    // offsets in the character layer of each player's digits (longs; the first table also read as words, PC-relative)
    t.data("credit_digit_offsets", 0x14740, "x32", Some(9));
    t.prog("credit_digit_offsets_pc", 0x14740, "x32", Some(9));
    t.data("credit_digit_offsets_two_coins", 0x14764, "x32", Some(9));
    t.data("credit_tall_offsets", 0x1489a, "x32", Some(9));
}

/// Player interface (`hud/`).
fn hud(t: &mut Tables) {
    t.prog("hud_layouts", 0x15164, "u8", None);
    t.prog("hud_zone_origins", 0x172b4, "i16", Some(0x12e / 2 + 1));
    t.prog("hud_class_of_character", 0x158c0, "u8", None);
    t.prog("hud_class_of_character_alt", 0x15e18, "u8", None);
    t.prog("hud_class_of_character_play", 0x16b3c, "u8", None);
    t.prog("hud_class_of_character_choice", 0x16c14, "u8", None);
    // "INSERT COIN" and "PUSH START" banners: pointers to columns of three tiles (longs, data space)
    t.data("hud_banners", 0x15662, "addr", None);
    t.prog("hud_lobby_palettes", 0x179a2, "i16", Some(4));
    t.data("hud_selection_letters", 0x16220, "u16*4", Some(0x57));
    t.data("hud_selection_letters_alt", 0x164d8, "u16*4", Some(0x57));
    t.prog("hud_selection_names", 0x160c4, "u16*2", None);
    t.prog("hud_letter_tables", 0x16e0a, "addr", Some(8));
    t.data("hud_letters", 0x16e2a, "u16*2", None);
    t.prog("hud_rank_tiles", 0x169a4, "u16", None);
    t.prog("hud_rank_special_tiles", 0x16aa8, "u16", None);
    t.data("hud_choice_portraits", 0x16c26, "addr", Some(12));
    t.data("hud_choice_portraits_alt", 0x16c56, "addr", Some(12));
    t.prog("hud_name_tiles", 0x16fe2, "u16", None);
    t.prog("hud_class_icon_tiles", 0x1707c, "u16", None);
    t.data("hud_face_tiles", 0x17136, "u16*2", None);
    t.prog("hud_item_special_tiles", 0x171b4, "u16", None);
    t.prog("hud_bar_tiles", 0x17a64, "u16", Some(0x1e));
    t.data("hud_portrait_pointers", 0x17b96, "addr", Some(8));
    t.data("hud_big_digits", 0x17bb6, "u16", None);
    // columns of three tiles of the banners and the portraits (words)
    t.data("hud_columns", 0x3f54c2, "u16", Some((0x3f6c08 - 0x3f54c2) / 2));
    // palettes of the characters and of the choice (the palette library read as data)
    t.palette(0x25c4c0, 0x700);
}

/// Texts of the character layer that `texts.json` does not know (`text.rs`): the "PUSH START" prompt of the prompt
/// task (`tasks/prompt.rs`).
fn texts(t: &mut Tables) {
    t.data("prompt_texts", 0xe04b0, "u8", Some(0xe04e4 - 0xe04b0));
}

pub fn specs(rom: &Program) -> Vec<Spec> {
    let mut t = Tables::new(rom);
    credits(&mut t);
    texts(&mut t);
    hud(&mut t);
    t.finish()
}

/// The tables in memory (mode with ROMs).
pub fn screens(rom: &Program) -> DataFile {
    build(rom, Vec::new(), &specs(rom), Vec::new())
}

pub fn extract(roms: &Path, path: &Path) -> io::Result<()> {
    let rom = Program::load(roms)?;
    save(&rom, path, Vec::new(), &specs(&rom), Vec::new())
}
