//! Scenery maps (scroll 2 and scroll 3): block banks, strips, scene strip tables.
//! Same tables as the game cameras (`ddsom-game/src/scroll.rs`) and `examples/cards.rs`.

use std::io;
use std::path::Path;

use ddsom_assets::json;
use ddsom_assets::levels::{Bank, Layer, Levels, Scroll1, StripTable};

use crate::rom::Program;

/// Number of scenes in the pointer table (0 to 114).
const SCENES: u32 = 115;

struct Geometry {
    name: &'static str,
    case: u16,
    tile_offset: u16,
    palette_base: u16,
    filler: u16,
    banks: u32,
    strips: u32,
    scenes: u32,
    /// Header of the first strip table.
    first_table: u32,
}

const LAYERS: [Geometry; 2] = [
    Geometry {
        name: "scroll2",
        case: 16,
        tile_offset: 0x1800,
        palette_base: 0x40,
        filler: 0xfc,
        banks: 0x320f6,
        strips: 0x1f9ba0,
        scenes: 0x2ff94,
        first_table: 0x318a2,
    },
    Geometry {
        name: "scroll3",
        case: 32,
        tile_offset: 0x1f00,
        palette_base: 0x60,
        filler: 0xfd,
        banks: 0x33a9c,
        strips: 0x2459c0,
        scenes: 0x32d7c,
        first_table: 0x33564,
    },
];

pub fn extract(roms: &Path, path: &Path) -> io::Result<()> {
    let rom = Program::load(roms)?;
    let layers = LAYERS.iter().map(|g| layer(&rom, g)).collect::<io::Result<Vec<_>>>()?;
    let scroll1 = scroll1(&rom);
    json::save(path, &Levels { layers, scroll1 }, 6)
}

fn bad(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

/// Strip tables: they follow one another in ROM; each is a width word `W` then rows of `W` words,
/// and the scene pointer designates its row of ordinate 0. The number of rows below 0 is the one that leads to a
/// following header consistent with the next pointer (same reading as `examples/cards.rs`); the last
/// table stops at the bank table, which follows.
fn tables(rom: &Program, g: &Geometry) -> io::Result<(Vec<StripTable>, Vec<usize>)> {
    let scene_ptrs: Vec<u32> = (0..SCENES).map(|s| rom.prog32(g.scenes + 4 * s)).collect();
    let mut ptrs = scene_ptrs.clone();
    ptrs.sort();
    ptrs.dedup();
    let valid = |head: u32, p: u32| {
        let w = rom.rd16(head) as u32;
        (1..=64).contains(&w) && p >= head + 2 && (p - head - 2).is_multiple_of(2 * w)
    };
    let mut tables = Vec::new();
    let mut head = g.first_table;
    for (i, &p) in ptrs.iter().enumerate() {
        if !valid(head, p) {
            return Err(bad(format!("{}: unreadable strip table at {head:#x}", g.name)));
        }
        let w = rom.rd16(head) as u32;
        let below = (0..=8u32)
            .find(|t| ptrs.get(i + 1).map_or(p + 2 * w * (1 + t) == g.banks, |&q| valid(p + 2 * w * (1 + t), q)))
            .ok_or_else(|| bad(format!("{}: end of table {head:#x} not found", g.name)))?;
        let zero = (p - head - 2) / (2 * w);
        let rows = (0..zero + 1 + below).map(|r| (0..w).map(|x| rom.rd16(head + 2 + 2 * (w * r + x))).collect()).collect();
        tables.push(StripTable { rom: head, rows, zero: zero as usize });
        head = p + 2 * w * (1 + below);
    }
    let scenes = scene_ptrs.iter().map(|p| ptrs.binary_search(p).unwrap()).collect();
    Ok((tables, scenes))
}

fn layer(rom: &Program, g: &Geometry) -> io::Result<Layer> {
    let (tables, scenes) = tables(rom, g)?;
    // strips in use (without the filler nor the 0xfe end-of-row sentinel of scroll 2)
    let sentinel = |r: &Vec<u16>| (g.case == 16 && r.last() == Some(&0xfe)) as usize;
    let used = tables
        .iter()
        .flat_map(|t| t.rows.iter().flat_map(|r| r[..r.len() - sentinel(r)].iter().copied()))
        .filter(|&s| s != g.filler)
        .max()
        .unwrap_or(0) as usize;
    let mut layer = Layer {
        name: g.name.into(),
        case: g.case,
        tile_offset: g.tile_offset,
        palette_base: g.palette_base,
        fillers: vec![g.filler],
        banks_rom: g.banks,
        banks: Vec::new(),
        strips_rom: g.strips,
        strips: Vec::new(),
        scenes_rom: g.scenes,
        scenes,
        tables,
    };
    let side = layer.strip_cases() / 2;
    let words = (side * side) as u32;
    layer.strips = (0..=used as u32).map(|s| (0..words).map(|k| rom.rd16(g.strips + 2 * (s * words + k))).collect()).collect();
    // blocks: as many as the largest number used by the strips of each bank
    let mut counts = vec![0usize; layer.bank_of(used) + 1];
    for (s, strip) in layer.strips.iter().enumerate() {
        let b = layer.bank_of(s);
        counts[b] = counts[b].max(strip.iter().map(|&i| i as usize + 1).max().unwrap_or(0));
    }
    layer.banks = counts
        .iter()
        .enumerate()
        .map(|(b, &n)| {
            let at = rom.rd32(g.banks + 4 * b as u32);
            let blocks = (0..n as u32)
                .map(|i| std::array::from_fn(|k| rom.rd16(at + 16 * i + 2 * k as u32)))
                .collect();
            Bank { rom: at, blocks }
        })
        .collect();
    eprintln!("{}: {} strip tables, {} strips, blocks per bank {:?}", g.name, layer.tables.len(), layer.strips.len(), counts);
    Ok(layer)
}

/// Scroll 1: frame grids (0x260c, `0x1f1b60 + 0x80 D0`) and 4x4-cell blocks (`0x1f3b60 + 64 i`),
/// which end where the scroll 2 strips begin.
const SCROLL1_GRIDS: u32 = 0x1f1b60;
const SCROLL1_BLOCKS: u32 = 0x1f3b60;

fn scroll1(rom: &Program) -> Scroll1 {
    let grid = |g: u32| (0..64).map(|k| rom.rd16(SCROLL1_GRIDS + 0x80 * g + 2 * k)).collect::<Vec<u16>>();
    // grids up to the last non-null one
    let count = (0..(SCROLL1_BLOCKS - SCROLL1_GRIDS) / 0x80).filter(|&g| grid(g).iter().any(|&v| v != 0)).max().map_or(0, |g| g + 1);
    let grids: Vec<Vec<u16>> = (0..count).map(grid).collect();
    let used = grids.iter().flatten().copied().max().unwrap_or(0) as u32;
    let blocks = (0..=used).map(|i| (0..32).map(|k| rom.rd16(SCROLL1_BLOCKS + 64 * i + 2 * k)).collect()).collect();
    eprintln!("scroll1 : {} images, {} blocs", grids.len(), used + 1);
    Scroll1 { tile_offset: 0xf800, grids_rom: SCROLL1_GRIDS, grids, blocks_rom: SCROLL1_BLOCKS, blocks }
}
