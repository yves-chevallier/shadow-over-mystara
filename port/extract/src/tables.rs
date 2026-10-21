//! Tables des objets liées aux sprites : boîtes de collision (0x11bffa...), de visibilité (0x1ca538),
//! marques au sol (0x1e45c), listes d'images lues par le code (0x9e8bc, 0x17efca) et tables du tir ESHL 5.

use std::io;
use std::path::Path;

use ddsom_assets::json;
use ddsom_assets::tables::{BoxBlock, BoxTable, Bounds, FrameList, GroundMark, HitBox, Shot5, SpriteTables};

use crate::rom::Program;

/// Tables de jeux de boîtes : 72 jeux chacune, à la suite ; les boîtes suivent la dernière.
const BOX_TABLES: [(&str, u32); 3] = [("body", 0x11bffa), ("push", 0x11c11a), ("attack", 0x11c23a)];
const BOX_SETS: u32 = 72;
/// Fin des boîtes : la dernière (0x1235f8) est suivie d'un mot de bourrage `0x4e71` et d'autres données.
const BOX_END: u32 = 0x123600;
/// Boîtes de visibilité : 128 entrées (des données d'une autre nature suivent).
const BOUNDS: u32 = 0x1ca538;
const BOUNDS_COUNT: u32 = 128;
/// Marques au sol : 97 entrées, du code suit (0x1ea6c).
const MARKS: u32 = 0x1e45c;
const MARKS_COUNT: u32 = 97;
/// Table des images (une adresse par image).
const FRAMES: u32 = 0x293000;
const FRAME_COUNT: u32 = 21887;
/// Liste lue par 0xb48c0 (`+0x33` du porteur).
const LIST_B48C0: u32 = 0x17efca;
/// Tir ESHL 5.
const SHOT5_BOXES: u32 = 0x9e55c;
const SHOT5_BOXES_COUNT: u32 = 15;
const SHOT5_EVEN: u32 = 0x9e638;
const SHOT5_ODD: u32 = 0x9e6a4;
const SHOT5_KINDS: u32 = 12;
const SHOT5_MIRRORED: u32 = 0x9e6bc;
const SHOT5_MIRRORED_COUNT: u32 = 256;

fn bad(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

pub fn extract(roms: &Path, path: &Path) -> io::Result<()> {
    let rom = Program::load(roms)?;
    // boîtes de collision
    let box_tables: Vec<BoxTable> = BOX_TABLES
        .iter()
        .map(|&(name, at)| BoxTable { name: name.into(), rom: at, sets: (0..BOX_SETS).map(|s| rom.rd32(at + 4 * s)).collect() })
        .collect();
    let mut starts: Vec<u32> = box_tables.iter().flat_map(|t| t.sets.iter().copied()).collect();
    starts.sort();
    starts.dedup();
    if starts[0] != BOX_TABLES[2].1 + 4 * BOX_SETS {
        return Err(bad(format!("boîtes : premier bloc en {:#x}", starts[0])));
    }
    let box_blocks = starts
        .iter()
        .enumerate()
        .map(|(i, &at)| {
            let end = starts.get(i + 1).copied().unwrap_or(BOX_END);
            let bytes: Vec<u8> = (at..end).map(|a| rom.rd8(a)).collect();
            let n = bytes.len() / 8;
            BoxBlock { rom: at, boxes: bytes[..8 * n].chunks_exact(8).map(HitBox::from_bytes).collect(), tail: bytes[8 * n..].to_vec() }
        })
        .collect::<Vec<_>>();
    let bounds = (0..BOUNDS_COUNT)
        .map(|i| {
            let w = |k: u32| rom.rd16(BOUNDS + 8 * i + 2 * k);
            Bounds(w(0) as i16, w(1), w(2) as i16, w(3))
        })
        .collect();
    let marks = (0..MARKS_COUNT).map(|i| GroundMark::from_words(std::array::from_fn(|k| rom.rd16(MARKS + 16 * i + 2 * k as u32)))).collect();
    // listes d'images
    let mut frame_of: Vec<(u32, u32)> = (0..FRAME_COUNT).map(|i| (rom.rd32(FRAMES + 4 * i), i)).collect();
    frame_of.sort();
    let index = |a: u32| frame_of.binary_search_by_key(&a, |&(r, _)| r).ok().map(|k| frame_of[k].1);
    let list = |at: u32, end: Option<u32>| -> FrameList {
        let mut frames = Vec::new();
        let mut a = at;
        while end.map_or(true, |e| a < e) {
            match index(rom.rd32(a)) {
                Some(f) => frames.push(f),
                None if end.is_none() => break,
                None => panic!("liste d'images {at:#x} : {:#x} n'est pas une image", rom.rd32(a)),
            }
            a += 4;
        }
        FrameList { rom: at, frames }
    };
    let even: Vec<i16> = (0..SHOT5_KINDS).map(|k| rom.prog16(SHOT5_EVEN + 2 * k) as i16).collect();
    let odd: Vec<i16> = (0..SHOT5_KINDS).map(|k| rom.prog16(SHOT5_ODD + 2 * k) as i16).collect();
    let mut targets: Vec<u32> = even
        .iter()
        .map(|&d| SHOT5_EVEN.wrapping_add(d as u32))
        .chain(odd.iter().map(|&d| SHOT5_ODD.wrapping_add(d as u32)))
        .collect();
    targets.sort();
    targets.dedup();
    let mut frame_lists: Vec<FrameList> = targets.iter().enumerate().map(|(i, &t)| list(t, targets.get(i + 1).copied())).collect();
    frame_lists.push(list(LIST_B48C0, None));
    let shot5 = Shot5 {
        boxes_rom: SHOT5_BOXES,
        boxes: (0..SHOT5_BOXES_COUNT).map(|i| std::array::from_fn(|k| rom.prog8(SHOT5_BOXES + 4 * i + k as u32))).collect(),
        even_rom: SHOT5_EVEN,
        even,
        odd_rom: SHOT5_ODD,
        odd,
        mirrored_rom: SHOT5_MIRRORED,
        mirrored: (0..SHOT5_MIRRORED_COUNT).map(|i| [rom.rd8(SHOT5_MIRRORED + 2 * i), rom.rd8(SHOT5_MIRRORED + 2 * i + 1)]).collect(),
    };
    eprintln!(
        "tables : {} blocs de boîtes, {} boîtes de visibilité, {} marques au sol, {} listes d'images ({} images)",
        box_blocks.len(),
        BOUNDS_COUNT,
        MARKS_COUNT,
        frame_lists.len(),
        frame_lists.iter().map(|l| l.frames.len()).sum::<usize>()
    );
    let tables = SpriteTables { box_tables, box_blocks, box_end: BOX_END, bounds_rom: BOUNDS, bounds, marks_rom: MARKS, marks, frame_lists, shot5 };
    json::save(path, &tables, 3)
}
