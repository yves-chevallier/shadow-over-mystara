//! Miscellaneous data: `data/misc.json` — bytes and names of the sprite viewer scripts (hidden test menu),
//! recorded inputs of the attract demos, ending credits.

use ddsom_assets::data::{Region, TextBlock};

use crate::data::{data, prog, region, text, Spec};
use crate::rom::Program;

/// Demo inputs: 30 sequences of 14-byte records (duration in frames, then 3 input bytes
/// per player, compared and copied to `+0x64a..+0x64c` of the four players; recorder 0xe0698).
/// Each sequence starts with a header (duration 0, inputs 0xff) and ends with a duration 0xffff; between
/// sequences, padding words (`0x4e71`, once 0) and a filler record (0xff).
const DEMO: (u32, u32) = (0x2b04de, 0x2b45d8);
const DEMO_FORMAT: &str = "u16 x8*12";
const DEMO_FIELDS: &[&str] =
    &["frames", "p1a", "p1b", "p1c", "p2a", "p2b", "p2c", "p3a", "p3b", "p3c", "p4a", "p4b", "p4c"];

fn demo(rom: &Program, s: &mut Vec<Spec>) {
    let bytes = |a: u32, n: u32| (a..a + n).map(|x| rom.rd8(x)).collect::<Vec<u8>>();
    let header: Vec<u8> = [0, 0].into_iter().chain([0xff; 12]).collect();
    let (mut p, mut k) = (DEMO.0, 0);
    while p < DEMO.1 {
        if bytes(p, 14) == header {
            let start = p;
            p += 14;
            while rom.rd16(p) != 0xffff {
                p += 14;
            }
            p += 14;
            s.push(data(&format!("demo/{k}"), start, DEMO_FORMAT, DEMO_FIELDS, ((p - start) / 14) as usize));
            k += 1;
        } else if bytes(p, 14) == [0xff; 14] {
            s.push(data(&format!("filler/{p:#x}"), p, DEMO_FORMAT, DEMO_FIELDS, 1));
            p += 14;
        } else {
            assert!(matches!(rom.rd16(p), 0x4e71 | 0), "demos: unexpected {p:#x}");
            s.push(data(&format!("pad/{p:#x}"), p, "x16", &[], 1));
            p += 2;
        }
    }
    assert_eq!(p, DEMO.1);
}

/// Demo scripts (`modes/demo.rs`): interpreter A (two alternating scripts, table 0x1f348)
/// and B (0x1f756). Operation: negative byte = end; 0 = scene (scene, duration, four players of 6 bytes,
/// address of the recorded inputs); 2 = sound command (index, skipped byte) then scene; other =
/// character presentation (3 bytes). B only knows scenes. A 0xff byte separates the scripts.
const SCENE_FIELDS: &[&str] = &[
    "op", "scene", "frames", "c1", "s1", "w1", "h1", "i1", "u1", "c2", "s2", "w2", "h2", "i2", "u2", "c3", "s3", "w3",
    "h3", "i3", "u3", "c4", "s4", "w4", "h4", "i4", "u4", "inputs",
];
const SOUND_FIELDS: &[&str] = &[
    "op", "sound", "skipped", "scene", "frames", "c1", "s1", "w1", "h1", "i1", "u1", "c2", "s2", "w2", "h2", "i2",
    "u2", "c3", "s3", "w3", "h3", "i3", "u3", "c4", "s4", "w4", "h4", "i4", "u4", "inputs",
];

fn demo_scripts(rom: &Program, s: &mut Vec<Spec>) -> Region {
    s.push(prog("demo_scripts", 0x1f348, "addr", &[], 2));
    let mut p = 0x1f3c2;
    for (k, b_mode) in [(0, false), (1, false), (2, true)] {
        let mut n = 0;
        loop {
            let op = rom.rd8(p);
            let name = format!("demo_script/{k}.{n}");
            n += 1;
            if op & 0x80 != 0 {
                s.push(data(&name, p, "x8", &[], 1));
                p += 1;
                break;
            }
            if b_mode || op == 0 {
                s.push(data(&name, p, "u8 u8 u16 u8*24 addr", SCENE_FIELDS, 1));
                p += 32;
            } else if op == 2 {
                s.push(data(&name, p, "u8 u8 u8 u8 u16 u8*24 addr", SOUND_FIELDS, 1));
                p += 34;
            } else {
                s.push(data(&name, p, "u8*4", &["op", "present", "kind", "character"], 1));
                p += 4;
            }
        }
        // padding byte before the next script
        s.push(data(&format!("pad/{p:#x}"), p, "x8", &[], 1));
        p += 1;
    }
    region("demo_scripts", 0x1f3c2, p)
}

pub fn specs(rom: &Program) -> (Vec<Region>, Vec<Spec>, Vec<TextBlock>) {
    let mut s = vec![data("viewer_counts", 0x2a85fc, "u8", &[], 2512)];
    demo(rom, &mut s);
    let texts = vec![
        text(rom, "viewer_names", 0x2a8fcc, 0x2ae81e, 0xff),
        text(rom, "credits", 0xd01de, 0xd06e5, b'/'),
    ];
    let mut regions = vec![region("viewer", 0x2a85fc, 0x2ae81e), region("demo", DEMO.0, DEMO.1)];
    regions.push(demo_scripts(rom, &mut s));
    (regions, s, texts)
}
