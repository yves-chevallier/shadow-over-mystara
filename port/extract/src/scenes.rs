//! Scene tables: `data/scenes.json` (`scene/entry.rs`, `scene/exit.rs`, `scene/blackout.rs`,
//! `scene/tally.rs`, `scene/shop.rs`, `scene/bonus.rs`, `camera_modes.rs`, `popups.rs`, `sprites.rs`,
//! `families/player/peff.rs`; the SET tables are in `data/set.json`, `set.rs`).

use ddsom_assets::data::Region;

use crate::data::{data, prog, region, span, Spec};
use crate::rom::Program;

/// End of a tile list (0xf6f90, `tiles::glyphs`): attributes, then tiles; 0xffff: new
/// attributes (next word); 0: end.
fn glyphs_end(rom: &Program, at: u32) -> u32 {
    let mut p = at + 2;
    loop {
        match rom.rd16(p) {
            0 => return p + 2,
            0xffff => p += 4,
            _ => p += 2,
        }
    }
}

/// Tile list, raw words up to and including the final 0.
fn glyphs(rom: &Program, name: &str, at: u32) -> (Spec, u32) {
    let end = glyphs_end(rom, at);
    (span(name, at, end, false, "u16", &[]), end)
}

pub fn specs(rom: &Program) -> (Vec<Region>, Vec<Spec>) {
    let mut regions = Vec::new();
    let mut s = vec![
        // ---- entry.rs: scene entry (data, per scene) ----
        span("scene_marker", 0x21156, 0x2123c, false, "u8*2", &["marker", "unused"]),
        span("start_positions", 0x2123c, 0x2196c, false, "i16*8", &["x1", "y1", "x2", "y2", "x3", "y3", "x4", "y4"]),
        span("start_states", 0x2196c, 0x21b38, false, "u8*4", &[]),
        span("palette_a", 0x21b38, 0x21bb0, false, "u8", &[]),
        span("palette_b", 0x21bb0, 0x21c28, false, "u8", &[]),
        prog("scene_38_durations", 0x22c28, "u16", &[], 5),
        // ---- exit.rs, blackout.rs: scene exit ----
        prog("exit_sequence", 0xf3170, "u8", &[], 110),
        prog("route_scenes", 0xf4a0c, "u8", &[], 4),
        data("blackout_palettes", 0xf80c4, "i8 u8*3", &["set", "page1", "page2", "page3"], 70),
        // ---- tally.rs: end-of-level tally ----
        data("time_step", 0xf5a62, "x16*2", &["ten_seconds", "minute"], 1),
        data("second_bonus", 0xf5b06, "x8", &[], 4),
        prog("total_at", 0xf5c08, "i16", &[], 8),
        prog("label_at", 0xf5c18, "i16", &[], 8),
        data("panel_at", 0xf5d82, "i16", &[], 8),
        data("panel_value_at", 0xf5d92, "i16", &[], 8),
        prog("payout_at", 0xf5f0a, "i16", &[], 8),
        data("time_bonus", 0xf6ba0, "x32", &[], 31),
        data("hits_bonus", 0xf6c1c, "x32", &[], 81),
        data("treasure_bonus", 0x19c0e, "x32", &[], 105),
        prog("stage_names", 0xf6ed2, "addr", &[], 22),
        prog("lead_name_first", 0xf6f48, "u8", &[], 22),
        prog("lead_clear_first", 0xf6f6a, "u8", &[], 22),
        data("player_label", 0xf7320, "u16", &[], 11),
        // ---- shop.rs: shop between two levels ----
        data("shop_music", 0xf7ab0, "u8", &[], 20),
        span("shop_of_scene", 0xf7ad0, 0xf7b2a, false, "u8", &[]),
        data("shop_welcome", 0xf7b88, "u8", &[], 20),
        prog("shop_exit_list", 0xf7c14, "u8", &[], 20),
        prog("shop_exit_lists", 0xf7c28, "addr", &[], 2),
        data("shop_exit_messages/0", 0xf7c30, "u16", &[], 2),
        data("shop_exit_messages/1", 0xf7c34, "u16", &[], 2),
        data("shop_next_scene", 0xf7d5e, "u8", &[], 20),
        data("shop_chained", 0xf7d72, "u8", &[], 20),
        data("portraits", 0xf7e92, "x32 u16 u16", &["at", "tile", "attr"], 4),
        data("portraits_wide", 0xf7eb2, "x32 u16 u16", &["at", "tile", "attr"], 3),
        // ---- peff.rs: counters (scenes 0x45 to 0x52) ----
        prog("counter_scripts_a", 0x51106, "addr", &[], 14),
        prog("counter_scripts_b", 0x5113e, "addr", &[], 14),
        span("counter_scripts", 0x51176, 0x51306, false, "u8 u8 i16", &["op", "param", "d1"]),
        span("counter_zones_a", 0x51306, 0x51376, false, "i16*4", &["dx", "width", "dy", "depth"]),
        span("counter_zones_b", 0x51376, 0x513e6, false, "i16*4", &["dx", "width", "dy", "depth"]),
        // ---- camera_modes.rs: camera initialization ----
        span("camera_layers", 0x2ef22, 0x2f008, false, "x16", &[]),
        span("camera_output", 0x2f008, 0x2f0ee, false, "x16", &[]),
        span("camera_max_x", 0x2f0ee, 0x2f1d4, false, "u16", &[]),
        span("camera_min_x", 0x2f1d4, 0x2f2ba, false, "u16", &[]),
        span("camera_frozen", 0x2f2ba, 0x2f332, false, "u8", &[]),
        span("camera_hold_min", 0x2f332, 0x2f3aa, false, "u8", &[]),
        span("camera_row_step", 0x2f3aa, 0x2f576, false, "addr", &[]),
        span("camera2_start", 0x32484, 0x32650, false, "i16*2", &["x", "y"]),
        span("camera2_row_step", 0x32650, 0x3281c, false, "addr", &[]),
        // ---- interface ----
        prog("popup_origins", 0x19f6c, "i16", &[], 8),
        prog("popup_bottoms", 0x1a25e, "u16", &[], 9),
        data("markers", 0x1eb48, "x16 u16", &["x", "y"], 4),
        data("markers_alt", 0x1eb58, "x16 u16", &["x", "y"], 3),
    ];
    regions.push(region("entry", 0x21156, 0x21c28));
    regions.push(region("counters", 0x51176, 0x513e6));
    regions.push(region("camera1", 0x2ef22, 0x2f576));
    regions.push(region("camera2", 0x32484, 0x3281c));
    // ---- tally.rs: banner tile lists ("STAGE CLEAR", scene names) ----
    let mut at = 0xf6fec;
    let names: Vec<u32> = (0..22).map(|i| rom.prog32(0xf6ed2 + 4 * i)).collect();
    while at < 0xf71fa {
        let name = match at {
            0xf6fec => "clear_before".to_string(),
            0xf7000 => "clear_after".to_string(),
            _ => match names.iter().position(|&n| n == at) {
                Some(i) => format!("stage_name/{i}"),
                None => format!("glyphs/{at:#x}"),
            },
        };
        let (spec, end) = glyphs(rom, &name, at);
        s.push(spec);
        at = end;
    }
    regions.push(region("banner", 0xf6fec, 0xf71fa));
    // ---- bonus.rs: bonus-stage scenery (height, width, cells per column), 0xffff after each ----
    let mut at = 0x3f6c08;
    for name in ["bonus_backdrop_alt", "bonus_backdrop"] {
        let (rows, cols) = (rom.rd16(at) as u32, rom.rd16(at + 2) as u32);
        s.push(data(&format!("{name}.size"), at, "u16*2", &["rows", "columns"], 1));
        s.push(data(name, at + 4, "u16*2", &["code", "attr"], (rows * cols) as usize));
        let end = at + 4 + 4 * rows * cols;
        s.push(data(&format!("{name}.end"), end, "x16", &[], 1));
        at = end + 2;
    }
    regions.push(region("bonus", 0x3f6c08, at));
    (regions, s)
}
