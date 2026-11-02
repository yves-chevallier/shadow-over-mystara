//! Tables of the SET objects: `data/set.json` (`ddsom_data::GameData::set`, read by `families/set*` and
//! `families/scenery.rs` of the game through `families/set/tables.rs`; docs/fin.md, L6). The dialog windows, the
//! bubbles and their texts are in `data/dialogs.json` (`dialogs.rs`).
//!
//! A table of fixed shape has its number of rows. The others run up to the next table of the same space, or up to the
//! next 68000 instruction that the reference executes (`code_68k.txt`), whichever comes first: the bytes the code
//! reads past the shape the analysis gives (an index out of range) stay inside the table.

use std::collections::BTreeSet;
use std::io;
use std::path::Path;

use ddsom_assets::data::DataFile;

use crate::data::{build, row_size, save, Spec};
use crate::rom::Program;

const CODE: &str = include_str!("../code_68k.txt");

/// Starts of the executed instructions (all spaces).
fn code_starts() -> Vec<u32> {
    let mut v: Vec<u32> = CODE
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
        .map(|l| u32::from_str_radix(l.split_whitespace().next().unwrap(), 16).expect("code_68k.txt"))
        .collect();
    v.sort();
    v
}

/// Tables that other files describe, next to SET tables: an automatic table stops there.
const FOREIGN: [u32; 3] = [
    0xcbc02, // text box of the cleared level (texts.json)
    0xd6922, // continue prompt boxes (texts.json)
    0xd86e8, // text box of the passage door (texts.json)
];

/// Longest automatic table (bytes).
const MAX_AUTO: u32 = 0x400;

/// A table to extract: fixed number of rows, or automatic (`None`).
struct Item {
    name: String,
    rom: u32,
    program: bool,
    format: &'static str,
    fields: &'static [&'static str],
    rows: Option<usize>,
}

struct Tables<'a> {
    rom: &'a Program,
    items: Vec<Item>,
    /// Ranges of the palette library read as data (`[start, end)`).
    palettes: Vec<(u32, u32)>,
}

impl<'a> Tables<'a> {
    fn add(&mut self, name: &str, rom: u32, program: bool, format: &'static str, fields: &'static [&'static str], rows: Option<usize>) {
        self.items.push(Item { name: name.into(), rom, program, format, fields, rows });
    }

    /// Table of the data space.
    fn data(&mut self, name: &str, rom: u32, format: &'static str, rows: Option<usize>) {
        self.add(name, rom, false, format, &[], rows);
    }

    /// Table of the program space (read PC-relative).
    fn prog(&mut self, name: &str, rom: u32, format: &'static str, rows: Option<usize>) {
        self.add(name, rom, true, format, &[], rows);
    }

    fn long(&self, a: u32, program: bool) -> u32 {
        self.rom.long(a, program)
    }

    /// The `count` addresses of the table `at`, then an automatic data table of format `format` at each address.
    fn pointed(&mut self, name: &str, at: u32, program: bool, count: u32, format: &'static str) {
        self.add(name, at, program, "addr", &[], Some(count as usize));
        let targets: BTreeSet<u32> = (0..count).map(|i| self.long(at + 4 * i, program)).collect();
        for t in targets.into_iter().filter(|&t| t < 0x40_0000) {
            self.data(&format!("{name}@{t:#x}"), t, format, None);
        }
    }

    /// Palette colors read as data (`n` bytes from `at`).
    fn palette(&mut self, at: u32, n: u32) {
        self.palettes.push((at, at + n));
    }

    fn finish(mut self) -> Vec<Spec> {
        // palette ranges, merged
        self.palettes.sort();
        let mut merged: Vec<(u32, u32)> = Vec::new();
        for (a, e) in std::mem::take(&mut self.palettes) {
            match merged.last_mut() {
                Some(last) if a <= last.1 => last.1 = last.1.max(e),
                _ => merged.push((a, e)),
            }
        }
        for (a, e) in merged {
            self.add(&format!("palette@{a:#x}"), a, false, "x16", &[], Some(((e - a) / 2) as usize));
        }
        // one table per start and space
        let mut seen = BTreeSet::new();
        self.items.retain(|i| seen.insert((i.program, i.rom)));
        let code = code_starts();
        let starts: [BTreeSet<u32>; 2] = [false, true].map(|p| {
            self.items.iter().filter(|i| i.program == p).map(|i| i.rom).chain(FOREIGN).collect()
        });
        self.items
            .iter()
            .map(|i| {
                let size = row_size(i.format) as u32;
                let rows = i.rows.unwrap_or_else(|| {
                    let next_table = starts[i.program as usize].range(i.rom + 1..).next().copied().unwrap_or(u32::MAX);
                    let next_code = code[code.partition_point(|&c| c <= i.rom)..].first().copied().unwrap_or(u32::MAX);
                    let end = next_table.min(next_code).min(i.rom + MAX_AUTO);
                    (((end - i.rom) / size) as usize).max(1)
                });
                Spec { name: i.name.clone(), rom: i.rom, program: i.program, format: i.format, fields: i.fields, rows }
            })
            .collect()
    }
}

/// Type 0 (`palette_script`): script and source palette per variant (data space), then action per variant
/// (program space bytes) during a scene transition and at the end of the script.
const PALETTE_TABLE: u32 = 0xba57a;
const PALETTE_VARIANTS: usize = 0x4f;
const PALETTE_TRANSITION: u32 = 0xba44c;
const PALETTE_END: u32 = 0xba4ca;

/// Palette scripts: header (start row, duration of the first block), first block (row count then one index per row),
/// then blocks (duration, row count, indices) up to a null duration (end), 0xffff followed by an address (restart) or
/// negative followed by the address of the script that continues. The rows copied from the source palettes (32 bytes
/// per index, the index shifted on 16 bits as the original does) are palette ranges of the table.
fn palette_scripts(t: &mut Tables) {
    let rom = t.rom;
    t.add("palette_script_table", PALETTE_TABLE, false, "addr addr", &["script", "source"], Some(PALETTE_VARIANTS));
    t.prog("palette_script_transition", PALETTE_TRANSITION, "u8", Some(PALETTE_VARIANTS));
    t.prog("palette_script_end", PALETTE_END, "u8", Some(PALETTE_VARIANTS));
    let mut sources: Vec<(u32, u32)> = (0..PALETTE_VARIANTS as u32)
        .map(|v| (rom.rd32(PALETTE_TABLE + 8 * v), rom.rd32(PALETTE_TABLE + 8 * v + 4)))
        .collect();
    sources.sort();
    sources.dedup();
    let mut lines_of: std::collections::BTreeMap<u32, Vec<u16>> = Default::default();
    for &(script, _) in &sources {
        if lines_of.contains_key(&script) {
            continue;
        }
        let mut lines = Vec::new();
        let name = |k: &str| format!("palette_script/{script:#x}.{k}");
        t.add(&name("head"), script, false, "u16 u16", &["first_line", "timer"], Some(1));
        let n = rom.rd16(script + 4) as usize;
        t.add(&name("0"), script + 4, false, "u16", &["count"], Some(1));
        t.data(&name("0.lines"), script + 6, "u16", Some(n));
        lines.extend((0..n as u32).map(|i| rom.rd16(script + 6 + 2 * i)));
        let mut at = script + 6 + 2 * n as u32;
        for k in 1.. {
            let duration = rom.rd16(at);
            if duration == 0 {
                t.data(&name("end"), at, "x16", Some(1));
                break;
            }
            if duration & 0x8000 != 0 {
                let label = if duration == 0xffff { "restart" } else { "jump" };
                t.data(&name(label), at, "x16 addr", Some(1));
                break;
            }
            let n = rom.rd16(at + 2) as usize;
            t.add(&name(&k.to_string()), at, false, "u16 u16", &["duration", "count"], Some(1));
            t.data(&name(&format!("{k}.lines")), at + 4, "u16", Some(n));
            lines.extend((0..n as u32).map(|i| rom.rd16(at + 4 + 2 * i)));
            at += 4 + 2 * n as u32;
        }
        lines_of.insert(script, lines);
    }
    for (script, source) in sources {
        for &line in &lines_of[&script] {
            let offset = (line << 5) as u32;
            t.palette(source + offset, 0x20);
        }
    }
}

/// Types 4 and 6 (`tile_anim`): tables of 16-byte entries (program space: script address, tile map address in video
/// RAM, loop, six null bytes; the table ends at the first entry that is not one), then each script (blocks: duration,
/// columns, rows, one long per cell; a negative duration ends it).
const TILE_TABLES: [(u32, &str); 2] = [(0xc93a2, "4"), (0xcb024, "6")];
const TILE_ENTRY: &str = "addr addr u16 u16*3";
const TILE_FIELDS: &[&str] = &["script", "tilemap", "loop", "pad0", "pad1", "pad2"];

fn tile_scripts(t: &mut Tables) {
    let rom = t.rom;
    let mut scripts = BTreeSet::new();
    for (table, name) in TILE_TABLES {
        let mut n = 0;
        loop {
            let at = table + 16 * n;
            let (script, map) = (rom.prog32(at), rom.prog32(at + 4));
            if !(0x300000..0x400000).contains(&script) || !(0x900000..0x930000).contains(&map) {
                break;
            }
            scripts.insert(script);
            n += 1;
        }
        t.add(&format!("tile_anim_entries/{name}"), table, true, TILE_ENTRY, TILE_FIELDS, Some(n as usize));
    }
    for script in scripts {
        let (mut at, mut k) = (script, 0);
        loop {
            let duration = rom.rd16(at);
            if duration & 0x8000 != 0 {
                t.data(&format!("tile_anim/{script:#x}.end"), at, "x16", Some(1));
                break;
            }
            let (cols, rows) = (rom.rd16(at + 2) as usize, rom.rd16(at + 4) as usize);
            t.add(&format!("tile_anim/{script:#x}.{k}"), at, false, "u16*3", &["duration", "cols", "rows"], Some(1));
            if cols * rows > 0 {
                t.data(&format!("tile_anim/{script:#x}.{k}.cells"), at + 6, "u32", Some(cols * rows));
            }
            at += 6 + 4 * (cols * rows) as u32;
            k += 1;
        }
    }
}

/// Entry of an object script (SET 2 scenes and SET 21 waves): wait, maximum rank, X, Z, Y, minimum players or
/// difficulty, offset in the creator table, type, variant, `+0x2f`, `+0x2c`, word `+0x42`, absolute coordinates.
const SCRIPT_ENTRY: &str = "i16 u16 i16*3 u8*6 u16 u16";
const SCRIPT_FIELDS: &[&str] =
    &["delay", "rank", "x", "z", "y", "players", "creator", "kind", "variant", "p0", "p1", "f42", "absolute"];

/// Object scripts designated by the `count` addresses of `table` (data space): initial timer word, entries of 0x14
/// bytes, final word 0x8000.
fn object_scripts(t: &mut Tables, prefix: &str, table: u32, count: u32) {
    let rom = t.rom;
    t.data(&format!("{prefix}_scripts"), table, "addr", Some(count as usize));
    let mut starts: Vec<u32> = (0..count).map(|i| rom.rd32(table + 4 * i)).collect();
    starts.sort();
    starts.dedup();
    for &at in &starts {
        let mut p = at + 2;
        while rom.rd16(p) != 0x8000 {
            p += 0x14;
        }
        t.data(&format!("{prefix}/{at:#x}.timer"), at, "u16", Some(1));
        if p > at + 2 {
            t.add(&format!("{prefix}/{at:#x}"), at + 2, false, SCRIPT_ENTRY, SCRIPT_FIELDS, Some(((p - at - 2) / 0x14) as usize));
        }
        t.data(&format!("{prefix}/{at:#x}.end"), p, "x16", Some(1));
    }
}

/// The set (`set.rs`): types 0 to 14.
fn set_a(t: &mut Tables) {
    palette_scripts(t);
    tile_scripts(t);
    // scene_spawner: release routine per class (0xe, 0x10, 0x12)
    t.prog("free_by_class", 0xcc2b8, "addr", Some(3));
    // wave_leader: delays between two waves, by a 4-bit draw
    t.prog("wave_delays", 0xbc63e, "u8", Some(16));
    // exit_gate: doors (position, exit kind, zone, scene, depth ignored, bonus), player states per zone, placement
    t.add(
        "exit_gates",
        0xcb344,
        true,
        "i16*3 u8*10",
        &["x", "z", "y", "exit_kind", "zone", "scene", "ignore_depth", "bonus", "u0", "u1", "u2", "u3", "u4"],
        None,
    );
    t.prog("exit_gate_player_states", 0xcb998, "x32", None);
    t.prog("exit_gate_placement_row", 0xcbb92, "u8", None);
    t.prog("exit_gate_placement", 0xcbbe2, "i16*8", None);
    // dialogue: palette of the bubble frame
    t.palette(0x292260, 0x20);
    // palette_anim: 17 palettes, or 17 others 0x360 further
    t.palette(0x291f00, 0x360 + 17 * 0x20);
    // name_entry: grids, cursors, codes, check digit, board tiles, end records
    t.palette(0x25cd40, 0x320);
    t.prog("name_start_cells", 0xc9abe, "u16*2", None);
    t.data("name_defaults", 0xc9daa, "u8*8", None);
    t.prog("name_finish_positions", 0xca0e4, "u16", None);
    let records = 0xca1a4;
    t.prog("name_finish_records", records, "addr", None);
    t.data("name_replacements", 0xca5a6, "x32*2", None);
    t.prog("name_default_check", 0xca6aa, "u8", None);
    t.prog("name_special_tiles", 0xca83e, "u8", None);
    t.prog("name_narrow_row_offsets", 0xca894, "u16", None);
    t.prog("name_narrow_cells", 0xca8c8, "u16*2", None);
    t.prog("name_wide_row_offsets", 0xca9c8, "u16", None);
    t.prog("name_wide_cells", 0xca9f2, "u16*2", None);
    t.prog("name_grid_narrow", 0xcaacc, "u8", None);
    t.prog("name_grid_wide", 0xcab36, "u8", None);
    t.data("name_voiced", 0xcabdc, "u8", None);
    t.data("name_semi_voiced", 0xcac64, "u8", None);
    t.data("name_char_digits", 0xcacec, "u8", None);
    t.data("name_row_width_narrow", 0xcad74, "u16", None);
    t.data("name_row_width_wide", 0xcad80, "u16", None);
    t.data("name_special_codes", 0xcad8a, "u8", None);
    t.data("name_reset_codes", 0xcade6, "u8", None);
    let mut targets = BTreeSet::new();
    let mut i = 0;
    while records + 4 * i < 0xca1a4 + MAX_AUTO {
        let a = t.rom.prog32(records + 4 * i);
        if !(0xc0000..0xd0000).contains(&a) {
            break;
        }
        targets.insert(a);
        i += 1;
    }
    for a in targets {
        t.data(&format!("name_finish_record@{a:#x}"), a, "u8*5", None);
    }
}

/// The SET B types (`set_b.rs`): 16 to 32.
fn set_b(t: &mut Tables) {
    // bumper: kind and position per variant, boxes per set (hit, near, near 2)
    t.prog("bumper_kinds", 0xd4198, "u8", None);
    t.prog("bumper_positions", 0xd41a6, "i16*2", None);
    for (name, at) in [("bumper_hit", 0x11bffa), ("bumper_near", 0x11c11a), ("bumper_near2", 0x11c23a)] {
        t.pointed(name, at, false, 0x48, "i8 u8 i8 u8");
    }
    // boss_intro: explosion offsets
    t.prog("boss_intro_blast_dx", 0xd4910, "i16", Some(16));
    t.prog("boss_intro_blast_dy", 0xd4930, "i16", Some(16));
    // wave_spawner: scripts, start and end kinds, creators
    object_scripts(t, "wave", 0xd4c5e, 15);
    t.prog("wave_start_kind", 0xd4a0e, "u8", Some(20));
    t.prog("wave_end_kind", 0xd4bdc, "u8", Some(20));
    t.data("wave_creators", 0xd4b92, "addr", None);
    // life_bar: pieces per points, frame tile lists per boss kind (21, the names of `texts.json`)
    t.prog("life_bar_pieces", 0xd61b2, "u16", Some(9));
    t.pointed("life_bar_frames", 0xd6232, false, 21, "u8");
    // continue_countdown: prompt per stage, palette
    t.prog("continue_prompt", 0xd6464, "u8", None);
    t.palette(0x25c0e0, 0x20);
    // rank_adjust: rank deltas per variant
    t.pointed("rank_deltas", 0xd6aa0, false, 8, "i16");
    // rearguard: depth offsets
    t.prog("rearguard_offsets", 0xd80fe, "i16", None);
    // idle_hunter: waiting frames, active hunters per player count
    t.prog("idle_frames", 0xd8186, "u8", None);
    t.prog("idle_max_active", 0xd8448, "i16", None);
    // doors 27 and 29: parameters per variant, player states, placement
    const DOOR: &[&str] = &["x", "z", "y", "mode", "key", "stage", "f1b", "u0", "u1", "u2", "u3", "u4", "u5"];
    t.add("door_a", 0xd84e8, true, "i16*3 u8*10", DOOR, Some(3));
    t.add("door_b", 0xd8e5e, true, "i16*3 u8*10", DOOR, Some(3));
    t.prog("door_a_enter_states", 0xd8824, "x32", None);
    t.prog("door_a_placement_side", 0xd896e, "u8", None);
    t.prog("door_a_placement", 0xd89be, "i16*8", None);
    t.prog("door_b_placement", 0xd9052, "i16", None);
    // dialog: offsets of the kind 5 definitions, owner tokens, palettes
    t.prog("dialog_kind5_steps", 0xd7218, "u8", None);
    t.prog("dialog_owner_tokens", 0xd7988, "u16", Some(4));
    t.palette(0x292200, 0x80);
}

/// The SET C types (`set_c.rs`): 2 (scene controller), 3 (text window), 15 (screens between two levels).
fn set_c(t: &mut Tables) {
    // scene: scripts (116: the table ends where the first script starts), start and end actions per variant
    // (120), creators, curtain
    object_scripts(t, "scene", 0xbd276, 116);
    t.prog("scene_start_action", 0xbc880, "u8", Some(120));
    t.prog("scene_end_action", 0xbcba8, "u8", Some(120));
    t.data("scene_creators", 0xbcb4e, "addr", None);
    let curtain = 0xbcea4;
    let mut rows = 0;
    let mut colors = Vec::new();
    loop {
        let offset = t.rom.prog16(curtain + 6 * rows) as i16;
        rows += 1;
        if offset < 0 {
            break;
        }
        colors.push(offset as u32);
    }
    t.add("curtain", curtain, true, "i16 addr", &["colors", "dest"], Some(rows as usize));
    let mut colors: Vec<u32> = colors.into_iter().collect::<BTreeSet<_>>().into_iter().collect();
    colors.dedup();
    for c in colors {
        t.data(&format!("curtain_colors@{c:#x}"), 0xbcfc6 + c, "x16", Some(22));
    }
    // window: style 5 offsets, messages, names, title glyphs
    t.prog("window_style5_offsets", 0xc851c, "u8", None);
    t.data("window_default_names", 0xc8948, "u8", None);
    t.data("window_equip_lists", 0xc8e0c, "u8*6", Some(3));
    t.pointed("window_messages_lang4", 0xc8e34, true, 5, "u8");
    t.pointed("window_messages", 0xc8e48, true, 5, "u8");
    t.data("window_separator", 0xc91c4, "u16", Some(1));
    t.data("window_class_titles", 0xc91c6, "addr", None);
    t.data("window_glyphs", 0xc9216, "u16", None);
    // finale: shakes, delays, camera script, scenes, grids
    // shakes: two of four steps (0xd0b10, 0xd0b20), the opening shake (0xd0b30), the four-component shakes
    // (0xd0b50, 0xd0b70: eight bytes per step)
    t.data("finale_shakes", 0xd0b10, "i16*2", Some(32));
    t.prog("finale_delays", 0xce138, "u8", None);
    t.data("finale_camera", 0xce37a, "u16*8", None);
    t.pointed("finale_scenes", 0xce4ea, true, 12, "u16");
    t.data("finale_grids", 0xce686, "u16*2", None);
    // later variants: rewards, credits, markers, fireworks
    t.prog("reward_palettes", 0xcfdd2, "u16", None);
    t.prog("reward_ids", 0xcfe98, "u16", None);
    t.data("reward_icons", 0xcfec8, "u8", None);
    t.prog("reward_tests", 0xcff10, "u16", None);
    t.prog("marker_positions", 0xcf974, "i16*2", Some(8));
    t.prog("firework_order", 0xcfa3c, "u8", Some(16));
    t.prog("results_attrs", 0xd0854, "u8", Some(4));
    t.prog("results_dest", 0xd0858, "i16", None);
    t.data("credits_text", 0xd01de, "u8", Some(0xd06e5 - 0xd01de));
    t.data("results_tiles", 0xb2920, "u16", None);
    t.data("results_patterns", 0xb2a24, "u16", None);
    t.palette(0x253280, 0x20);
}

/// Shared with other families (`scenery.rs`, `set_residue.rs`): ESHL volleys, palette page 1.
fn shared(t: &mut Tables) {
    t.prog("eshl_volleys", 0xa66e2, "u8", None);
    // restoring palette page 1 (`scenery::restore_palette_page`): 32 blocks, then eight colours stored in the code
    t.palette(0x285280, 0x400);
    t.data("palette_page_1_colors", 0x1ad4a, "x16", Some(16));
}

pub fn specs(rom: &Program) -> Vec<Spec> {
    let mut t = Tables { rom, items: Vec::new(), palettes: Vec::new() };
    set_a(&mut t);
    set_b(&mut t);
    set_c(&mut t);
    shared(&mut t);
    t.finish()
}

/// The tables in memory (mode with ROMs).
pub fn set(rom: &Program) -> DataFile {
    build(rom, Vec::new(), &specs(rom), Vec::new())
}

pub fn extract(roms: &Path, path: &Path) -> io::Result<()> {
    let rom = Program::load(roms)?;
    save(&rom, path, Vec::new(), &specs(&rom), Vec::new())
}
