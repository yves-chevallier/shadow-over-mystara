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
    t.palette(0x25c4c0, 0x2000);
}

/// Texts of the character layer that `texts.json` does not know (`text.rs`): the "PUSH START" prompt of the prompt
/// task (`tasks/prompt.rs`).
fn texts(t: &mut Tables) {
    t.data("prompt_texts", 0xe04b0, "u8", Some(0xe04e4 - 0xe04b0));
}

/// Mode screens (`modes/`).
fn modes(t: &mut Tables) {
    // selection (`select.rs`): variant per character, announcements, cursor moves
    t.prog("select_variants", 0x1d118, "u8", Some(6));
    t.prog("select_banners", 0x1d3aa, "addr", None);
    t.data("select_cursor_up", 0x1d6cc, "i8", Some(6));
    t.data("select_cursor_down", 0x1d6d2, "i8", Some(6));
    // demonstrations (`demo/`): sound commands, phase handlers, presentation of the characters
    t.prog("demo_sound_commands", 0x1f3aa, "addr", Some(6));
    t.prog("demo_phases_a", 0x1f990, "addr", Some(4));
    t.prog("demo_phases_b", 0x20a9c, "addr", Some(4));
    t.prog("demo_present_tiles", 0x1fd04, "u16*2", None);
    t.data("demo_present_palettes", 0x1fe3e, "x16", Some((0x1febe - 0x1fe3e) / 2));
    t.data("demo_present_data", 0x1febe, "u8", None);
    t.prog("demo_frame_attrs", 0x1ffa2, "i16", None);
    t.prog("demo_box_attrs", 0x1ffd0, "u16", None);
    t.data("demo_text_1", 0x20070, "u8", Some(0x200ca - 0x20070));
    t.pointed("demo_text_2", 0x200ca, true, 6, "u8");
    // startup (`boot.rs`): region byte, palette, warnings, default high scores
    t.data("boot_region", 0xc4, "u8", Some(1));
    t.data("boot_palette", 0x133ba, "x16", Some(16));
    t.data("boot_high_scores", 0x18f1e, "u8", Some(140));
    t.data("boot_high_scores_alt", 0x18faa, "u8", Some(140));
    // ranking (`ranking.rs`): palette, text of the ranking lines
    t.data("ranking_palette", 0xfc6d8, "x16", Some(16));
    t.data("ranking_text", 0xfc938, "u8", None);
    t.palette(0x2537c0, 0x20);
    // title (`title.rs`): delays between the appearances, offsets of the texts, logo palette
    t.prog("title_delays", 0xfa42e, "u8", None);
    t.data("title_text_offsets", 0xfa8bc, "i16*2", Some(3));
    t.palette(0x2538c0, 0x80);
    // ending (`ending/`): offsets of the reward objects
    t.prog("ending_reward_offsets", 0xfc3e4, "i16*2", Some(3));
    t.data("ending_rewards", 0xfb7bc, "x32*2", Some(24));
    t.data("ending_terrain", 0xfbe92, "u8", Some(0xfbebc - 0xfbe92));
    t.data("ending_score_places", 0xfbebc, "i16*2", Some(4));
    t.data("ending_last_tiles", 0xfc4e2, "u16", Some(4));
    for at in [0x291980, 0x28fa80, 0x28faa0, 0x28f4c0, 0x292260, 0x291f00] {
        t.palette(at, 0x20);
    }
}

/// Scene sequences (`scene/`) besides the tables of `data/scenes.json`.
fn scenes(t: &mut Tables) {
    t.prog("exit_sequence_jumps", 0xf31ea, "i16", None);
    t.prog("exit_player_waits", 0xf5542, "u16", Some(5));
    t.prog("bonus_dispatch", 0xf5946, "u16", Some(2));
    t.prog("characters_masks", 0xf672e, "u8", None);
    t.prog("characters_tables", 0xf679c, "u8", Some(2));
    t.prog("big_digits", 0xf6e74, "u16", Some(10));
    t.data("shop_musics", 0xf7ac4, "x32", None);
    t.data("shop_welcomes", 0xf7b9c, "x32", None);
    // scenes past the tables of `scenes.json` (the demonstration scenes 109 to 114 read one entry further)
    t.data("shop_of_scene_tail", 0xf7b2a, "u8", Some(0x18));
    t.prog("exit_sequence_tail", 0xf31de, "u8", Some(0xc));
    // transitions (`transition/`): answers of the questions, scrolling texts, remaining-time prizes
    for at in [0xf847c, 0xf92e4, 0xf932e, 0xf937a, 0xf99e2] {
        t.prog(&format!("transition_answers@{at:#x}"), at, "u8", None);
    }
    for at in [0xf858a, 0xf8810, 0xf8986] {
        t.pointed(&format!("transition_scroll_texts@{at:#x}"), at, true, 2, "u16");
    }
    t.prog("transition_next_scenes", 0xf9454, "u8", None);
    t.prog("transition_time_prizes", 0xf95ba, "u8", Some(0x350));
}

/// Service menu (`service/`): colour ramps of the test pattern, levers and directions of the input test, coin modes
/// of the card dispenser.
fn service(t: &mut Tables) {
    t.data("service_color_ramps", 0x293a6, "u16", None);
    t.data("service_levers", 0x2a63c, "x32", Some(4));
    t.data("service_directions", 0x2a64c, "u8", None);
    t.prog("service_card_modes_4", 0x2c008, "u8", Some(4));
    t.prog("service_card_modes", 0x2c00c, "u8", Some(4));
    t.prog("service_ram_patterns", 0x2d0ce, "x16", Some(4));
}

/// Spawn scripts of the scenes (`scene/script.rs`): the table of one long per scene, then each script up to and
/// including its end word (0xffff), entries of 0x12 bytes (spawn) or 10 bytes (call, word `+2` = 0xffff).
fn spawn_scripts(t: &mut Tables, rom: &Program) {
    const TABLE: u32 = 0xf2f62;
    const SCENES: u32 = 115;
    t.data("spawn_scripts", TABLE, "addr", Some(SCENES as usize));
    let mut starts: Vec<u32> = (0..SCENES).map(|s| rom.rd32(TABLE + 4 * s)).collect();
    starts.sort();
    starts.dedup();
    for at in starts {
        let mut p = at;
        while rom.rd16(p) != 0xffff {
            p += if rom.rd16(p + 2) == 0xffff { 10 } else { 0x12 };
        }
        t.data(&format!("spawn_script@{at:#x}"), at, "u8", Some((p + 2 - at) as usize));
    }
    t.prog("spawn_allocators", 0xf2f38, "i16", None);
}

/// The bytes that the ROM test of the service menu (`service/memory.rs`) sums: one byte every 0xfff in each of the
/// eight blocks of 0x80000 bytes, and the expected sums (header words at 0xca). Built after the other tables so that
/// these single bytes do not cut their automatic ends.
fn rom_test() -> Vec<Spec> {
    let mut s = vec![crate::data::data("rom_test_sums", 0xca, "u16", &[], 8)];
    for k in 0..8u32 {
        let mut a = 0x80000 * k;
        while a < 0x80000 * (k + 1) {
            s.push(crate::data::data(&format!("rom_test@{a:#x}"), a, "u8", &[], 1));
            a += 0xfff;
        }
    }
    s
}

pub fn specs(rom: &Program) -> Vec<Spec> {
    let mut t = Tables::new(rom);
    credits(&mut t);
    texts(&mut t);
    hud(&mut t);
    modes(&mut t);
    spawn_scripts(&mut t, rom);
    scenes(&mut t);
    service(&mut t);
    let mut specs = t.finish();
    specs.extend(rom_test());
    specs
}

/// The tables in memory (mode with ROMs).
pub fn screens(rom: &Program) -> DataFile {
    build(rom, Vec::new(), &specs(rom), Vec::new())
}

pub fn extract(roms: &Path, path: &Path) -> io::Result<()> {
    let rom = Program::load(roms)?;
    save(&rom, path, Vec::new(), &specs(&rom), Vec::new())
}
