//! Tables read by the game in Rust (`ddsom-game`) that the other files do not cover: `data/rest.json`.
//! Measurement and list of the remaining zones: docs/assets.md "program ROM reads by the Rust code".
//!
//! * player sub-objects, weapon and shield (`player/overlays.rs`);
//! * character aura palettes (`player/effects.rs`);
//! * initial task stacks (`startup.rs`) and the sound panning table (`sound.rs`).

use ddsom_assets::data::Region;

use crate::data::{data, prog, Spec};
use crate::rom::Program;

/// Aura palettes: offset (program space) of each character slot, then 8 blocks of 16
/// colors per slot for each of the two auras, and the normal palette.
const AURA_OFFSETS: u32 = 0x378aa;
const AURA_B: u32 = 0x378c2;
const AURA_C: u32 = 0x384c2;
const AURA_NORMAL: u32 = 0x390c2;
const AURA_SLOTS: usize = 12;

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
    s.push(prog("aura_offsets", AURA_OFFSETS, "u16", &[], AURA_SLOTS));
    for (name, at, n) in [("aura_b", AURA_B, 8 * AURA_SLOTS), ("aura_c", AURA_C, 8 * AURA_SLOTS), ("aura_normal", AURA_NORMAL, AURA_SLOTS)] {
        s.push(data(name, at, "x16*16", &[], n));
    }
    s.push(data("task_stacks", 0x534, "addr", &[], 16));
    s.push(data("sound_pan", 0xee04e, "u8", &[], 48));
    let region = overlays(rom, &mut s);
    (vec![region], s)
}
