//! What the Rust sound driver reads in the Z80 region, by kind of data of `sound/music.json`.
//!
//! sound_reads <roms> <assets> <interrupts> [<eeprom> <scenario>:<frames> ...]
//!
//! Every sound of the table is played alone for `<interrupts>` 250 Hz periods (driver without machine), then
//! each scenario on the game (without 68000, Rust driver). The bytes read are counted per entry of
//! `music.json` (header, sound definitions, tracks, instrument banks, sample records, named tables); a byte
//! read outside all of them is listed.

use std::collections::BTreeMap;
use std::path::Path;

use ddsom_assets::music::Music;
use ddsom_sound::{Driver, Rom, SHARED_SIZE};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 4 {
        eprintln!("usage: sound_reads <roms> <assets> <interrupts> [<eeprom> <scenario>:<frames> ...]");
        std::process::exit(2);
    }
    let mut rom = Rom::load(Path::new(&a[1])).expect("sound rom");
    rom.trace_reads();
    let music = Music::load(&Path::new(&a[2]).join("sound/music.json")).expect("music.json");
    let irqs: usize = a[3].parse().expect("interrupts");

    // every sound alone
    let count = rom.sound_header().count;
    for n in 0..count {
        let mut driver = Driver::new(rom.clone());
        let mut shared = vec![0u8; SHARED_SIZE];
        let mut out = Vec::new();
        driver.reset(&mut shared, &mut out);
        for k in 0..irqs {
            if k == 10 {
                shared[0xfff] = 0xff;
                shared[0xffd] = 0x88;
                shared[0xffe] = 1;
                shared[0xf] = 0xff;
            }
            if k == 20 {
                shared[..16].fill(0);
                shared[0] = (n >> 8) as u8;
                shared[1] = n as u8;
                shared[0xf] = 0;
            }
            driver.irq_request(&mut shared, &mut out);
            out.clear();
        }
    }
    let alone = rom.read_bytes().len();

    // the scenarios
    if a.len() > 5 {
        let ee = std::fs::read(&a[4]).ok();
        for spec in &a[5..] {
            let (scenario, frames) = spec.rsplit_once(':').expect("<scenario>:<frames>");
            let mut m = cps2::Machine::load_ddsom(Path::new(&a[1]), ee.as_deref()).expect("roms");
            ddsom_game::install_no_68k(&mut m);
            m.rendering = false;
            m.sound.use_rust_driver();
            m.sound.driver = Some(Driver::new(rom.clone()));
            let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scenario");
            for f in 0..frames.parse::<usize>().expect("frames") {
                script.apply(f, &mut m);
                m.run_frame();
                m.sound.audio.clear();
            }
        }
    }

    // byte ranges described by music.json (linear addresses)
    let mut kinds: Vec<(String, u32, u32)> = Vec::new();
    kinds.push(("pointer to the sound header".into(), music.header_ptr, music.header_ptr + 2));
    kinds.push(("pointer to the sample table".into(), music.samples_ptr, music.samples_ptr + 2));
    kinds.push(("instrument bank table".into(), music.banks_table, music.banks_table + 2 * music.banks.len() as u32));
    kinds.push(("sound header and table".into(), music.header, music.header + 6 + 4 * music.songs.len() as u32));
    for s in music.songs.iter().flatten() {
        kinds.push(("sound definitions".into(), s.rom, s.rom + s.bytes().len() as u32));
    }
    for b in &music.code {
        kinds.push(("tracks".into(), b.rom, b.rom + b.bytes().map_or(0, |b| b.len() as u32)));
    }
    for b in &music.banks {
        kinds.push(("instruments".into(), b.rom, b.rom + 8 * b.instruments.len() as u32));
    }
    kinds.push(("sample records".into(), music.samples_rom, music.samples_rom + 8 * music.samples.len() as u32));
    for t in &music.tables {
        kinds.push((format!("table {}", t.name), t.rom, t.rom + t.bytes().len() as u32));
    }
    let mut per_kind: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for (name, from, to) in &kinds {
        per_kind.entry(name.clone()).or_default().1 += (to - from) as usize;
    }
    let mut outside = Vec::new();
    for i in rom.read_bytes() {
        let linear = if i < 0x8000 { i as u32 } else { i as u32 - 0x8000 };
        match kinds.iter().find(|(_, from, to)| (*from..*to).contains(&linear)) {
            Some((name, _, _)) => per_kind.get_mut(name).unwrap().0 += 1,
            None => outside.push(linear),
        }
    }
    println!("{count} sounds alone ({irqs} interrupts each): {alone} bytes read; with the scenarios: {}", rom.read_bytes().len());
    println!("{:32} {:>8} {:>8}", "music.json", "read", "size");
    for (name, (read, size)) in &per_kind {
        println!("{name:32} {read:>8} {size:>8}");
    }
    println!("outside music.json: {} bytes", outside.len());
    for chunk in outside.chunk_by(|x, y| y - x == 1) {
        println!("  {:#06x}-{:#06x}", chunk[0], chunk[chunk.len() - 1]);
    }
}
