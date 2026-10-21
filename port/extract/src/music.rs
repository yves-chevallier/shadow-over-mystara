//! Musiques et effets : table des sons, pistes, instruments, échantillons et tables du pilote Z80, lus
//! comme les lit `ddsom-sound` (adresses linéaires de `ddsom_sound::Rom`).

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use ddsom_assets::json;
use ddsom_assets::music::{decode_op, Bank, CodeBlock, DriverTable, Music, Song};
use ddsom_sound::Rom;

const HEADER_PTR: u32 = 0x39e1;
const SAMPLES_PTR: u32 = 0x39e3;
const BANKS_TABLE: u32 = 0x39e5;
/// Bancs : deux (le premier commence juste après la table, à 0x39e9), 256 fiches chacun.
const BANKS: u32 = 2;
const INSTRUMENTS: u32 = 256;
/// Échantillons : 290 fiches (comme `sounds/`).
const SAMPLES: u32 = 290;
/// Tables du pilote dans la ROM fixe : (nom, adresse, mots petit-boutistes, nombre de valeurs).
const TABLES: [(&str, u32, bool, u32); 14] = [
    ("qsound_init_regs", 0xd5, false, 12),
    ("qsound_init_values", 0xe1, true, 12),
    ("durations", 0x1119, true, 24),
    ("octave_offsets", 0x1149, false, 16),
    ("semitone_octave", 0x1fc6, false, 108),
    ("octave_start", 0x2032, false, 9),
    ("rates", 0x203b, true, 108),
    ("fine", 0x2113, false, 12 * 256),
    ("amplitudes", 0x33e1, true, 128),
    ("levels", 0x34e1, true, 128),
    ("attack", 0x35e1, true, 64),
    ("decay", 0x3661, true, 64),
    ("lfo_rates", 0x36e1, true, 128),
    ("vibrato", 0x37e1, true, 128),
];
const TREMOLO: (&str, u32, bool, u32) = ("tremolo", 0x38e1, true, 128);

fn bad(msg: String) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg)
}

pub fn extract(roms: &Path, path: &Path) -> io::Result<()> {
    let rom = Rom::load(roms)?;
    let be = |p: u32| u16::from(rom.byte(p)) << 8 | u16::from(rom.byte(p + 1));
    let header = u32::from(rom.word(HEADER_PTR));
    let count = be(header);
    let mut songs = Vec::new();
    for i in 0..u32::from(count) {
        let e = header + 6 + 4 * i;
        if rom.byte(e + 3) != 0 {
            return Err(bad(format!("son {i} : quatrième octet non nul")));
        }
        let at = u32::from(rom.byte(e)) << 16 | u32::from(be(e + 1));
        if at == 0 {
            songs.push(None);
            continue;
        }
        let kind = rom.byte(at);
        let mut s = Song { rom: at, kind, tracks: Vec::new(), body: Vec::new() };
        match kind {
            0..=0x7f => s.tracks = (0..16).map(|c| be(at + 1 + 2 * c)).collect(),
            0x80..=0x8f => s.body = (1..=10).map(|k| rom.byte(at + k)).collect(),
            0x90..=0x92 => s.body = (1..=7).map(|k| rom.byte(at + k)).collect(),
            _ => return Err(bad(format!("son {i} : type {kind:#x}"))),
        }
        songs.push(Some(s));
    }
    // instructions atteintes depuis les pistes, en suivant les sauts
    let mut ops: BTreeMap<u32, (String, u32)> = BTreeMap::new();
    let mut todo: Vec<u32> = songs.iter().flatten().flat_map(|s| s.track_starts().map(|(_, a)| a).collect::<Vec<_>>()).collect();
    while let Some(mut p) = todo.pop() {
        while !ops.contains_key(&p) {
            let bytes: Vec<u8> = (0..4).map(|k| rom.byte(p + k)).collect();
            let (tok, n) = decode_op(&bytes).ok_or_else(|| bad(format!("piste illisible en {p:#x}")))?;
            let n = n as u32;
            let op = bytes[0];
            ops.insert(p, (tok, n));
            let target = |off_at: u32| (off_at + 2).wrapping_add(be(off_at) as i16 as u32);
            match op {
                22 => {
                    todo.push(target(p + 1));
                    break;
                }
                23 => break,
                14..=21 => todo.push(target(p + 2)),
                _ => {}
            }
            p += n;
            if p >= 0x40000 {
                return Err(bad("piste hors de la ROM".into()));
            }
        }
    }
    // blocs d'instructions jointives ; une instruction ne doit pas en chevaucher une autre
    let mut code: Vec<CodeBlock> = Vec::new();
    let mut next = 0;
    for (&a, (tok, n)) in &ops {
        if a < next {
            return Err(bad(format!("instructions qui se chevauchent en {a:#x}")));
        }
        match code.last_mut() {
            Some(b) if a == next => {
                b.ops.push(' ');
                b.ops.push_str(tok);
            }
            _ => code.push(CodeBlock { rom: a, ops: tok.clone() }),
        }
        next = a + n;
    }
    let banks = (0..BANKS)
        .map(|k| {
            let at = u32::from(rom.word(BANKS_TABLE + 2 * k));
            Bank { rom: at, instruments: (0..INSTRUMENTS).map(|i| std::array::from_fn(|j| rom.byte(at + 8 * i + j as u32))).collect() }
        })
        .collect();
    let samples_rom = u32::from(rom.word(SAMPLES_PTR));
    let samples = (0..SAMPLES).map(|i| std::array::from_fn(|j| rom.byte(samples_rom + 8 * i + j as u32))).collect();
    let tables = TABLES
        .iter()
        .chain([&TREMOLO])
        .map(|&(name, at, words, n)| DriverTable {
            name: name.into(),
            rom: at,
            words,
            values: (0..n).map(|i| if words { rom.word(at + 2 * i) } else { u16::from(rom.byte(at + i)) }).collect(),
        })
        .collect();
    eprintln!(
        "musique : {} sons ({} définis), {} instructions de pistes en {} blocs",
        count,
        songs.iter().flatten().count(),
        ops.len(),
        code.len()
    );
    let music = Music {
        header_ptr: HEADER_PTR,
        header,
        count,
        music_factor: rom.byte(header + 2),
        sfx_volume: rom.byte(header + 3),
        header_rest: [rom.byte(header + 4), rom.byte(header + 5)],
        songs,
        code,
        banks_table: BANKS_TABLE,
        banks,
        samples_ptr: SAMPLES_PTR,
        samples_rom,
        samples,
        tables,
    };
    std::fs::create_dir_all(path.parent().unwrap())?;
    json::save(path, &music, 2)
}
