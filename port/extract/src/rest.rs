//! Tables read by the game in Rust (`ddsom-game`) that the other files do not cover: `data/rest.json`.
//! Measurement and list of the remaining zones: docs/assets.md « Lectures de la ROM programme par le Rust ».
//!
//! * tile-cell animation scripts of the type 4 and 6 SETs (`families/set.rs`, `tile_anim`);
//! * palette animation scripts of the type 0 SETs (`families/set.rs`, `palette_script`);
//! * player sub-objects, weapon and shield (`player/overlays.rs`);
//! * character aura palettes (`player/effects.rs`);
//! * initial task stacks (`startup.rs`) and the sound panning table (`sound.rs`).

use ddsom_assets::data::Region;

use crate::data::{data, prog, Spec};
use crate::rom::Program;

/// Tables of the tile-cell scripts (program space): types 4 and 6.
const TILE_TABLES: [(u32, &str); 2] = [(0xc93a2, "4"), (0xcb024, "6")];
/// One entry: script address, tile map address (video RAM), loop, six null bytes.
const TILE_ENTRY: &str = "addr addr u16 u16*3";
const TILE_FIELDS: &[&str] = &["script", "tilemap", "loop", "pad0", "pad1", "pad2"];

/// Aura palettes: offset (program space) of each character slot, then 8 blocks of 16
/// colors per slot for each of the two auras, and the normal palette.
const AURA_OFFSETS: u32 = 0x378aa;
const AURA_B: u32 = 0x378c2;
const AURA_C: u32 = 0x384c2;
const AURA_NORMAL: u32 = 0x390c2;
const AURA_SLOTS: usize = 12;

/// Type 0 SET: script table (data space, `script` and `source` per variant 0 to 0x4e), then actions
/// per variant (program space bytes) during a scene transition and at the end of the script.
const PALETTE_TABLE: u32 = 0xba57a;
const PALETTE_VARIANTS: usize = 0x4f;
const PALETTE_TRANSITION: u32 = 0xba44c;
const PALETTE_END: u32 = 0xba4ca;

/// Palette scripts: header (start row, duration of the first block), first block (row count then
/// one index per row), then blocks (duration, row count, indices) up to a null duration (end), 0xffff
/// followed by an address (restart) or negative followed by the address of the script that continues.
fn palette_scripts(rom: &Program, s: &mut Vec<Spec>) {
    s.push(data("palette_script_table", PALETTE_TABLE, "addr addr", &["script", "source"], PALETTE_VARIANTS));
    s.push(prog("palette_script_transition", PALETTE_TRANSITION, "u8", &[], PALETTE_VARIANTS));
    s.push(prog("palette_script_end", PALETTE_END, "u8", &[], PALETTE_VARIANTS));
    let scripts: std::collections::BTreeSet<u32> =
        (0..PALETTE_VARIANTS as u32).map(|v| rom.rd32(PALETTE_TABLE + 8 * v)).collect();
    for script in scripts {
        let name = |k: &str| format!("palette_script/{script:#x}.{k}");
        s.push(data(&name("head"), script, "u16 u16", &["first_line", "timer"], 1));
        let n = rom.rd16(script + 4) as usize;
        s.push(data(&name("0"), script + 4, "u16", &["count"], 1));
        s.push(data(&name("0.lines"), script + 6, "u16", &[], n));
        let mut at = script + 6 + 2 * n as u32;
        for k in 1.. {
            let duration = rom.rd16(at);
            if duration == 0 {
                s.push(data(&name("end"), at, "x16", &[], 1));
                break;
            }
            if duration & 0x8000 != 0 {
                let label = if duration == 0xffff { "restart" } else { "jump" };
                s.push(data(&name(label), at, "x16 addr", &[], 1));
                break;
            }
            let n = rom.rd16(at + 2) as usize;
            s.push(data(&name(&k.to_string()), at, "u16 u16", &["duration", "count"], 1));
            s.push(data(&name(&format!("{k}.lines")), at + 4, "u16", &[], n));
            at += 4 + 2 * n as u32;
        }
    }
}

/// Player sub-objects: one pointer per slot to the 8-byte rows of each character
/// (animation, x and z offsets, flip, behind), an animation script per weapon number and the
/// eight palettes that flicker under bonus. The row tables follow one another from 0x1147c6 to 0x11bffa (start of the
/// collision boxes): rows, then a padding `nop` word.
const OVERLAY_BODY: u32 = 0x395d4;
const OVERLAY_SHIELD: u32 = 0x39604;
const OVERLAY_ANIMS: u32 = 0x396e8;
const OVERLAY_PALETTES: u32 = 0x397e0;
const OVERLAY_ROWS: (u32, u32) = (0x1147c6, 0x11bffa);

fn overlays(rom: &Program, s: &mut Vec<Spec>) -> Region {
    s.push(data("overlay_body", OVERLAY_BODY, "addr", &[], 12));
    s.push(data("overlay_shield", OVERLAY_SHIELD, "addr", &[], 12));
    s.push(data("overlay_anims", OVERLAY_ANIMS, "addr", &[], 48));
    s.push(data("overlay_palettes", OVERLAY_PALETTES, "x16*16", &[], 8));
    let mut starts: Vec<u32> = (0..12)
        .flat_map(|i| [rom.rd32(OVERLAY_BODY + 4 * i), rom.rd32(OVERLAY_SHIELD + 4 * i)])
        .collect();
    starts.sort();
    starts.dedup();
    starts.push(OVERLAY_ROWS.1);
    assert_eq!(starts[0], OVERLAY_ROWS.0);
    for w in starts.windows(2) {
        let rows = (w[1] - w[0] - 2) / 8;
        assert_eq!((w[1] - w[0] - 2) % 8, 0, "rows of {:#x}", w[0]);
        s.push(data(&format!("overlay_rows/{:#x}", w[0]), w[0], "u16 i16 i16 u8 u8", &["anim", "dx", "dz", "flip", "behind"], rows as usize));
        assert_eq!(rom.rd16(w[0] + 8 * rows), 0x4e71);
        s.push(data(&format!("overlay_pad/{:#x}", w[0]), w[0] + 8 * rows, "x16", &[], 1));
    }
    Region { name: "overlay_rows".into(), start: OVERLAY_ROWS.0, end: OVERLAY_ROWS.1, program: false }
}

pub fn specs(rom: &Program) -> (Vec<Region>, Vec<Spec>) {
    let mut s = Vec::new();
    // cell scripts: entries, then each script (blocks: duration, columns, rows, one long per cell;
    // a negative word ends it)
    let mut scripts = std::collections::BTreeSet::new();
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
        s.push(prog(&format!("tile_anim_entries/{name}"), table, TILE_ENTRY, TILE_FIELDS, n as usize));
    }
    for script in scripts {
        let (mut at, mut k) = (script, 0);
        loop {
            let duration = rom.rd16(at);
            if duration & 0x8000 != 0 {
                s.push(data(&format!("tile_anim/{script:#x}.end"), at, "x16", &[], 1));
                break;
            }
            let (cols, rows) = (rom.rd16(at + 2) as usize, rom.rd16(at + 4) as usize);
            s.push(data(&format!("tile_anim/{script:#x}.{k}"), at, "u16*3", &["duration", "cols", "rows"], 1));
            if cols * rows > 0 {
                s.push(data(&format!("tile_anim/{script:#x}.{k}.cells"), at + 6, "u32", &[], cols * rows));
            }
            at += 6 + 4 * (cols * rows) as u32;
            k += 1;
        }
    }
    palette_scripts(rom, &mut s);
    s.push(prog("aura_offsets", AURA_OFFSETS, "u16", &[], AURA_SLOTS));
    for (name, at, n) in [("aura_b", AURA_B, 8 * AURA_SLOTS), ("aura_c", AURA_C, 8 * AURA_SLOTS), ("aura_normal", AURA_NORMAL, AURA_SLOTS)] {
        s.push(data(name, at, "x16*16", &[], n));
    }
    s.push(data("task_stacks", 0x534, "addr", &[], 16));
    s.push(data("sound_pan", 0xee04e, "u8", &[], 48));
    let region = overlays(rom, &mut s);
    (vec![region], s)
}
