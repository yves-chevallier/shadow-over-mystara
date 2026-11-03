//! Tables of the engine: `data/engine.json` (`ddsom_data::GameData::engine`, read by `ai`, `ai_aids`, `aids`,
//! `camera_modes`, `scroll`, `video`, `palette`, `sprites`, `placement` and `families/{small,creators}` of the game
//! through `ddsom-game/src/tables.rs`; docs/fin.md, L12). The tables of the engine already described by another file
//! are read from that file: initial hit points, immunities and approach points (`data/enemies.json`), per-scene camera
//! tables and player markers (`data/scenes.json`), aura palettes (`data/rest.json`), scenery maps (`levels.json`),
//! fixed terrain maps (`terrain.json`), scene palettes (`scene_palettes.json`), palette library (`palettes.png`),
//! visibility boxes, ground marks and collision boxes (`sprites/tables.json`).
//!
//! Same rules as the SET tables (`set.rs`): a table of fixed shape has its number of rows; the others run up to the
//! next table of the same space or the next instruction that the reference executes, at most 0x400 bytes. A table of
//! addresses ([`pointers`]) runs up to the first address that is not one, or up to the first table it designates.

use std::io;
use std::path::Path;

use ddsom_assets::data::DataFile;

use crate::data::{build, save};
use crate::rom::Program;
use crate::set::Tables;

/// Number of addresses of the table at `at`: up to the first long that is not an even address of the program ROM, or
/// up to the first address it designates (the designated tables often follow it), at most 256.
fn pointer_count(rom: &Program, at: u32, program: bool) -> u32 {
    let mut first_target = u32::MAX;
    let mut n = 0;
    while n < 256 && at + 4 * n < first_target {
        let v = rom.long(at + 4 * n, program);
        if !(0x400..0x40_0000).contains(&v) || v & 1 != 0 {
            break;
        }
        if v > at {
            first_target = first_target.min(v);
        }
        n += 1;
    }
    n
}

/// A table of addresses (longs of the given space) and the data tables (format `format`, automatic length) it
/// designates; `count`: its number of addresses, when the code bounds it.
fn pointers(t: &mut Tables, rom: &Program, name: &str, at: u32, program: bool, count: Option<u32>, format: &'static str) {
    let n = count.unwrap_or_else(|| pointer_count(rom, at, program));
    t.pointed(name, at, program, n, format);
}

/// AI helpers (`ai.rs`, `ai_aids/`): criteria, decision masks, stance and box draws, pushes, grabs, walking points,
/// draws by row, loot.
fn ai(t: &mut Tables, rom: &Program) {
    // target criteria drawn at random (16 bytes each)
    t.prog("ai/criteria_strict", 0x426a, "u8", Some(16));
    t.prog("ai/criteria_lenient", 0x439e, "u8", Some(16));
    // decision masks by row (`+0x72`, `+0x73`), then by rank; bytes by rank (`+0x76`)
    pointers(t, rom, "ai/decisions_a", 0xa4f2, false, Some(9), "x32");
    pointers(t, rom, "ai/decisions_b", 0xa516, false, None, "x32");
    pointers(t, rom, "ai/bytes_by_rank", 0xac58, false, None, "u8");
    // heading drift steps (signed bytes)
    t.prog("ai/drift_steps", 0x1058e, "i8", Some(16));
    // stance by row (`+0x70`), box chances (`+0x74`) and animations (`+0x75`)
    pointers(t, rom, "ai/stances", 0xa48c, false, None, "u8");
    pointers(t, rom, "ai/box_chances", 0x7fec, false, Some(8), "x32");
    pointers(t, rom, "ai/box_animations", 0x800c, false, None, "u8");
    // pushes of the players by a boss: profile by x gap / 16, then profiles (max height, max depth, push)
    t.prog("ai/push_profile_index", 0x1168e, "u8", Some(32));
    t.add("ai/push_profiles", 0x116ae, false, "i16 i16 x32", &["max_height", "max_depth", "push"], Some(16));
    // enemy attack boxes (8 bytes: offset and half-extent in x, z, y), then the grab moves (x, height, orientation,
    // direction)
    t.data("ai/attack_boxes", 0x123602, "i8 u8 i8 u8 i8 u8 u8 u8", Some(10));
    t.data("ai/seize_moves", 0x123652, "i8 i8 u8 i8", None);
    // body boxes of the players, one set per character (addresses in the collision boxes of `sprites/tables.json`)
    t.data("ai/body_box_sets", 0xfd4c, "addr", Some(6));
    // walking points mirrored by the direction of the target
    pointers(t, rom, "ai/mirrored_walk_points", 0xf06a, false, Some(1), "i16");
    // draws by row: bytes (`+0x6a`, `+0x6d`, `+0x8e`) and masks by rank (`+0x6c`, parries `+0x68`/`+0x69`)
    pointers(t, rom, "ai/draws_6b", 0x78ae, false, None, "u8");
    pointers(t, rom, "ai/masks_6c", 0x7a34, false, Some(7), "x32");
    pointers(t, rom, "ai/draws_6e", 0x7a50, false, Some(9), "u8");
    pointers(t, rom, "ai/parries", 0xb0b0, false, None, "x32");
    pointers(t, rom, "ai/draws_85", 0xf160, false, None, "u8");
    // countdown masks by players and rank, behaviour by type, parry levels
    t.prog("ai/countdown_masks", 0xff9a, "x16", None);
    t.prog("ai/behaviour_by_kind", 0x1000a, "u8", None);
    t.prog("ai/parry_levels", 0x10144, "u8", None);
    // HP thresholds of the phases of the enemies and bosses (`next_threshold`): 16 blocks of 0x100 bytes, two rows of
    // 64 words (by HP rank) per block; a phase beyond the second reads the next block, as the original does
    t.data("ai/hp_thresholds", 0x105e2, "i16", Some(0x800));
    // landing routines by type (enemies, then bosses from index 32)
    t.prog("ai/landing_routines", 0x3e49c, "addr", None);
    // debris sets (program-space addresses of rows of four words), ground debris (rows of six words)
    pointers(t, rom, "ai/debris_sets", 0x1178a, true, None, "i16");
    t.data("ai/ground_debris_0", 0x11a9e, "i16*6", Some(5));
    t.data("ai/ground_debris_1", 0x11ada, "i16*6", Some(5));
    // loot: object dropped once per type and its luck masks, loot by row, treasures by row and their lists
    t.prog("ai/once_variants", 0xb6e4, "u8", None);
    t.data("ai/once_masks", 0xb6fc, "x32", None);
    t.prog("ai/loot", 0xb806, "u8", None);
    t.prog("ai/treasure_rows", 0xe372, "u8", None);
    pointers(t, rom, "ai/treasure_lists", 0xe642, false, None, "u8");
    // special objects: (kind, variant) by row, then by difficulty and by the classes in play (contiguous tables)
    for (name, at, end) in [
        ("ai/special_rows", 0xbf26, 0xc4c6),
        ("ai/special_common", 0xc4c6, 0xca86),
        ("ai/special_pairs", 0xca86, 0xd046),
        ("ai/special_by_class", 0xd046, 0xd606),
        ("ai/special_kind_2", 0xd606, 0xd776),
        ("ai/special_cleric_or_mage", 0xd776, 0xda56),
        ("ai/special_mages", 0xda56, 0xdbc6),
        ("ai/special_clerics", 0xdbc6, 0xdd36),
        ("ai/special_mage_or_cleric", 0xdd36, 0xdea6),
    ] {
        t.data(name, at, "u8", Some((end - at) as usize));
    }
}

/// Small shared routines (`aids.rs`): direction table of 0x1386, by octant and step.
fn aids(t: &mut Tables) {
    t.prog("aids/directions", 0x140c, "u8", Some(64));
}

/// Placement of a resurrected player (`placement.rs`): starting point of the search per player, from the camera.
fn placement(t: &mut Tables) {
    t.prog("placement/start_x", 0x3658, "i16", Some(4));
    t.prog("placement/start_y", 0x3660, "i16", Some(4));
}

/// Cameras (`scroll.rs`, `camera_modes/`): per-scene tables that `data/scenes.json` does not describe, the strip steps
/// that its tables `camera_row_step` and `camera2_row_step` designate, the scripts' tables.
fn cameras(t: &mut Tables, rom: &Program) {
    t.data("scroll/camera_service", 0x33bdc, "u8", Some(120));
    t.add("scroll/starts", 0x33c54, false, "i16 i16", &["x", "y"], Some(115));
    for table in [0x2f3aa, 0x32650] {
        for k in 0..115 {
            let a = rom.rd32(table + 4 * k);
            if a < 0x40_0000 {
                t.data(&format!("camera/row_step@{a:#x}"), a, "u16", Some(1));
            }
        }
    }
    // undulation: per-line scroll table copied nine times (0x3022a)
    t.data("camera/ripple_rows", 0x3028e, "x32", Some(16));
    // thunderstorm: palette fade steps (source in the palette library, destination), ended by a null source
    let steps = (0..).find(|&k| rom.prog32(0x30816 + 8 * k) == 0).unwrap();
    t.add("camera/palette_steps", 0x30816, true, "addr addr", &["source", "destination"], Some(steps as usize + 1));
    for k in 0..steps {
        let source = rom.prog32(0x30816 + 8 * k);
        t.data(&format!("camera/palette_step@{source:#x}"), source, "x16", Some(24));
    }
    // camera 2: blocks of layer 3 loaded at the entry of scenes 4/91, 6, 12/101, 18 (block, offset from 0x918000)
    for (at, n) in [(0x321fc, 24), (0x32284, 8), (0x322cc, 8), (0x32314, 8)] {
        t.add(&format!("camera2/blocks@{at:#x}"), at, false, "u16 i16", &["block", "offset"], Some(n));
    }
}

/// Video primitives (`video.rs`): the tables of the block banks of the grid loaders (layer 2 in data space, layer 3 in
/// program space), the table of fixed screens copied at startup.
fn video(t: &mut Tables) {
    t.data("video/scroll2_banks", 0x275a, "addr", None);
    // the grids of blocks of layer 2 that no scene map uses (the backgrounds that the screens load by number), after
    // the strips of `levels.json`, and the bytes between the last block bank of layer 2 and the strips of layer 3
    t.data("video/scroll2_grids", 0x20c0a0, "x16", Some((0x211ba0 - 0x20c0a0) / 2));
    t.data("video/scroll2_tail", 0x2457c0, "x16", Some((0x2459c0 - 0x2457c0) / 2));
    t.prog("video/scroll3_banks", 0x27e6, "addr", None);
    t.data("video/fixed_screens", 0x19a1a, "x16", Some(60));
}

/// Texts displayed in sprites (`sprites.rs`, three slots of string pointers: bytes, or words for the two-player
/// cabinet), by ranges of the strings that the game shows.
fn sprite_texts(t: &mut Tables) {
    // the visibility boxes and ground marks past the tables of `sprites/tables.json` (128 and 97 entries) that an
    // index byte can designate (original glitch: the bytes that follow, code included, read as boxes and marks)
    t.data("sprites/bounds_beyond", 0x1ca938, "x16", Some((0x1cad38 - 0x1ca938) / 2));
    t.data("sprites/marks_beyond", 0x1ea6c, "x16", Some((0x1f45c - 0x1ea6c) / 2));
    const RANGES: [(u32, u32); 33] = [
        (0x1d8be, 0x1ddd0),
        (0x1ef5c, 0x1ef5e),
        (0x200f6, 0x20522),
        (0xb2b6a, 0xb2c54),
        (0xde4e2, 0xde608),
        (0xdea84, 0xdebf0),
        (0xdec66, 0xdedee),
        (0xe01cc, 0xe0256),
        (0xf8596, 0xf863c),
        (0xf881c, 0xf8856),
        (0xf8992, 0xf89ea),
        (0x185c60, 0x185d0a),
        (0x185e3c, 0x185ede),
        (0x186162, 0x186208),
        (0x1865c8, 0x1869b8),
        (0x186a32, 0x186b42),
        (0x186c1e, 0x186cce),
        (0x186d18, 0x186d6e),
        (0x187086, 0x187328),
        (0x18745c, 0x1875ea),
        (0x18911e, 0x1891be),
        (0x1892d2, 0x18936a),
        (0x1895c6, 0x189662),
        (0x1899e6, 0x189d5e),
        (0x189dce, 0x189eb6),
        (0x189f7e, 0x18a01a),
        (0x18a05a, 0x18a0a6),
        (0x18a382, 0x18a5f2),
        (0x18a712, 0x18a882),
        (0x1b53e2, 0x1b53ee),
        (0x1b5406, 0x1b540a),
        (0x1bf146, 0x1bf156),
        (0x1c3f3a, 0x1c3fe6),
    ];
    for (at, end) in RANGES {
        t.data(&format!("sprites/texts@{at:#x}"), at, "u8", Some((end - at) as usize));
    }
}

/// Palettes (`palette.rs`): a page of the scene palettes whose entry is null is copied from address 0 (original glitch:
/// the reset vectors and the start of the program read as 32 palettes).
fn palettes(t: &mut Tables) {
    t.data("palette/from_zero", 0, "x16", Some(0x200));
}

/// Small creators and routines (`families/small.rs`): roars of boss 15 (counts by rank / 2), the stones' offsets, the
/// undead counts by rank / 4, the four palettes of 0x101da.
fn small(t: &mut Tables) {
    t.prog("small/roar_big", 0xa726c, "u8", Some(16));
    t.prog("small/roar_small", 0xa72b4, "u8", Some(16));
    t.prog("small/stone_offsets", 0x537ca, "i16 i16", Some(12));
    t.prog("small/undead_counts", 0x3baf8, "u8", Some(8));
    t.data("small/palettes_69", 0x10218, "x16", Some(64));
}

/// Object creators (`families/creators.rs`): the kinds of weapon that come with a shot (two masks), the SET kind of the
/// scene objects, the music and exit routines of the scene objects by variant, the argument of SET 0x15 by creator type.
fn creators(t: &mut Tables) {
    t.prog("creators/weapon_shots", 0x51c5c, "x32", Some(2));
    t.prog("creators/scene_sets", 0xc834c, "u8", Some(4));
    t.prog("creators/music_routines", 0xd978c, "addr", Some(3));
    t.prog("creators/music_of_variant", 0xd9798, "u8", None);
    t.prog("creators/exit_routines", 0xd982e, "addr", Some(2));
    t.prog("creators/exit_of_variant", 0xd9836, "u8", None);
    t.prog("creators/set_15_args", 0xd697c, "x16", None);
}

/// Rank (`objects/rank.rs`): the rows of settings (16 bytes), chosen by `A5+0x2a`; the last three rows are read in
/// the code that follows the six rows of the table (original glitch: a row number above 5).
fn rank(t: &mut Tables) {
    t.data("rank/presets", 0x1f1d8, "x32*4", Some(9));
}

/// Settings (`config.rs`): the default block of each variant (0x30 bytes, read in data space at the address that the
/// displacement table designates), the coins per credit by coin mode, the credits by credit mode, the message of the
/// failed EEPROM write.
fn config(t: &mut Tables, rom: &Program) {
    const VARIANTS: u32 = 0x2d656;
    // displacements by variant (`A5+0x70` = 0, 2... 10), relative to the table
    t.prog("config/variants", VARIANTS, "i16", Some(6));
    let mut blocks: Vec<u32> = (0..6).map(|k| VARIANTS.wrapping_add(rom.prog16(VARIANTS + 2 * k) as i16 as u32)).collect();
    blocks.sort_unstable();
    blocks.dedup();
    for at in blocks {
        t.data(&format!("config/defaults@{at:#x}"), at, "u8", Some(0x30));
    }
    t.prog("config/coins", 0x2d38c, "x16", Some(17));
    t.prog("config/credits_a", 0x2d40e, "u8", Some(0x18));
    t.prog("config/credits_b", 0x2d426, "u8", Some(0x0c));
    t.data("config/error_message", 0x2d7fc, "u8", Some(12));
}

/// The command tables of the sound wrappers and the tables of the dispatchers (`sound.rs`), read PC-relative; the lists
/// of the hit noises by category (data space) that the dispatcher 0xe662a designates. Addresses: `sound_tables.txt`.
fn sound(t: &mut Tables, rom: &Program) {
    for l in include_str!("../sound_tables.txt").lines().filter(|l| !l.starts_with('#')) {
        let (kind, at) = l.split_once(' ').unwrap_or_else(|| panic!("sound_tables.txt: '{l}'"));
        let at = u32::from_str_radix(at.trim_start_matches("0x"), 16).unwrap_or_else(|_| panic!("sound_tables.txt: '{l}'"));
        let name = format!("sound/{kind}@{at:#x}");
        match kind {
            "Delays" => t.data(&name, at, "x16", None),
            "Alternate" => t.prog(&name, at, "x16", Some(2)),
            "ByVariant" => t.prog(&name, at, "x16", Some(8)),
            "ByVariantLong" | "ByA1" => t.prog(&name, at, "addr", if kind == "ByA1" { None } else { Some(8) }),
            "ByVariantAndCategory" => {
                t.prog(&name, at, "addr", Some(8));
                // the last two rows (variants 12 and 14) are the code that follows: no list there
                let mut lists: Vec<u32> =
                    (0..8).map(|k| rom.prog32(at + 4 * k)).filter(|&a| a < 0x40_0000 && a & 1 == 0).collect();
                lists.sort_unstable();
                lists.dedup();
                for list in lists {
                    t.data(&format!("sound/categories@{list:#x}"), list, "addr", None);
                }
            }
            _ => t.prog(&name, at, "x16", None),
        }
    }
}

/// Reset (`startup.rs`): the program header (debug display of the sound locks, region word, "development card" byte),
/// the starting palette.
fn startup(t: &mut Tables) {
    t.data("startup/header", 0xc0, "x16", Some(3));
    t.data("startup/palette", 0x19f0, "x32", Some(8));
}

/// Prompts (`tasks/prompt.rs`): the text of each coin setting (from setting 1).
fn prompts(t: &mut Tables) {
    t.data("prompt/coin_texts", 0xe048c, "addr", Some(9));
}

/// Bytes of the code that the original reads as data (glitches written in clear in the game): the byte `+0x2c` of the
/// shard rain's handler (EFFCT, A1 = the handler's address 0xb997c).
fn code_bytes(t: &mut Tables) {
    t.data("effects/rain_handler_facing", 0xb997c + 0x2c, "u8", Some(1));
}

/// The ranking screen of the attract mode (`tasks/fixed_screen.rs`): tiles of the numbers of ranks 2 to 4, then of the
/// large digits, digits, ranks 5 to 10, portraits and letters (indexed by a name byte: 0xff is read as 0x81, the
/// other bytes past the letters read the words that follow, as the original does).
fn ranking(t: &mut Tables) {
    t.data("ranking/big_ranks", 0x18b88, "x16", Some(3));
    t.data("ranking/tiles", 0x18d58, "x16", Some((0x18dbe + 2 * 0x100 - 0x18d58) / 2));
}

/// All the tables of the engine.
fn all(t: &mut Tables, rom: &Program) {
    ranking(t);
    code_bytes(t);
    prompts(t);
    startup(t);
    config(t, rom);
    sound(t, rom);
    ai(t, rom);
    aids(t);
    placement(t);
    cameras(t, rom);
    video(t);
    sprite_texts(t);
    small(t);
    creators(t);
    rank(t);
    palettes(t);
}

/// The engine tables of the program `rom`.
pub fn engine(rom: &Program) -> DataFile {
    let mut t = Tables::new(rom);
    all(&mut t, rom);
    build(rom, Vec::new(), &t.finish(), Vec::new())
}

pub fn extract(roms: &Path, path: &Path) -> io::Result<()> {
    let rom = Program::load(roms)?;
    let mut t = Tables::new(&rom);
    all(&mut t, &rom);
    save(&rom, path, Vec::new(), &t.finish(), Vec::new())
}
