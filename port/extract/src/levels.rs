//! Cartes des décors (scroll 2 et scroll 3) : banques de blocs, bandes, tables de bandes des scènes.
//! Mêmes tables que les caméras du jeu (`ddsom-game/src/scroll.rs`) et `examples/cartes.rs`.

use std::io;
use std::path::Path;

use ddsom_assets::json;
use ddsom_assets::levels::{Bank, Layer, Levels, StripTable};

use crate::rom::Program;

/// Nombre de scènes de la table des pointeurs (0 à 114).
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
    /// En-tête de la première table de bandes.
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
    json::save(path, &Levels { layers }, 6)
}

fn bad(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

/// Tables de bandes : elles se suivent en ROM ; chacune est un mot de largeur `W` puis des lignes de `W` mots,
/// et le pointeur de scène désigne sa ligne d'ordonnée 0. Le nombre de lignes sous 0 est celui qui amène à un
/// en-tête suivant cohérent avec le pointeur suivant (même lecture que `examples/cartes.rs`) ; la dernière
/// table s'arrête à la table des banques, qui suit.
fn tables(rom: &Program, g: &Geometry) -> io::Result<(Vec<StripTable>, Vec<usize>)> {
    let scene_ptrs: Vec<u32> = (0..SCENES).map(|s| rom.prog32(g.scenes + 4 * s)).collect();
    let mut ptrs = scene_ptrs.clone();
    ptrs.sort();
    ptrs.dedup();
    let valid = |head: u32, p: u32| {
        let w = rom.rd16(head) as u32;
        (1..=64).contains(&w) && p >= head + 2 && (p - head - 2) % (2 * w) == 0
    };
    let mut tables = Vec::new();
    let mut head = g.first_table;
    for (i, &p) in ptrs.iter().enumerate() {
        if !valid(head, p) {
            return Err(bad(format!("{} : table de bandes illisible en {head:#x}", g.name)));
        }
        let w = rom.rd16(head) as u32;
        let below = (0..=8u32)
            .find(|t| ptrs.get(i + 1).map_or(p + 2 * w * (1 + t) == g.banks, |&q| valid(p + 2 * w * (1 + t), q)))
            .ok_or_else(|| bad(format!("{} : fin de la table {head:#x} introuvable", g.name)))?;
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
    // bandes employées (sans le remplissage ni la sentinelle 0xfe de fin de ligne du scroll 2)
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
    // blocs : autant que le plus grand numéro employé par les bandes de chaque banque
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
    eprintln!("{} : {} tables de bandes, {} bandes, blocs par banque {:?}", g.name, layer.tables.len(), layer.strips.len(), counts);
    Ok(layer)
}
