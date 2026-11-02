//! Differential validation without the game: the original Z80 alone and the Rust driver receive
//! the same synthetic commands in the shared RAM, and their writes are compared. This exercises
//! the whole ROM (all songs and effects, all system commands) and not only what the game
//! scenario sends.
//!
//! sound_fuzz <roms> songs <first> <last> <interrupts>  one run per song/effect number
//! sound_fuzz <roms> random <seed> <runs> <interrupts>   random command sequences
//! sound_fuzz <roms> synth <seed> <runs> <interrupts>    synthetic sounds added to the ROM
//! sound_fuzz <roms> short <seed> <runs> <interrupts>    same with short tracks
//! sound_fuzz <roms> one songs <number> <interrupts> [from to]           one run, with details
//! sound_fuzz <roms> one random|synth|short <seed> <run> <interrupts> [from to]
//!
//! Environment variables: FUZZ_STATE=1 also compares the Z80 RAM with the driver state (when
//! the Z80 is at rest); FUZZ_COVERAGE=file writes the coverage of the Z80 program;
//! FUZZ_SHOW, FUZZ_PROFILE, FUZZ_BREAK=address, FUZZ_WATCH=address: debugging aids for `one`.

#[path = "support/mod.rs"]
mod support;

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use support::session::*;
use support::{Report, SoundRom as Rom};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 4 {
        eprintln!("usage: sound_fuzz <roms> songs|random|synth|short|one ... (see the file header)");
        std::process::exit(2);
    }
    let rom = Rom::load(Path::new(&a[1])).expect("sound rom");
    let num = |i: usize| a[i].parse::<usize>().expect("number");
    match a[2].as_str() {
        "songs" => {
            let (first, last, irqs) = (num(3), num(4), num(5));
            run_many(&rom, first..=last, |n| song_session(&rom, n, irqs));
        }
        "random" => {
            let (seed, count, irqs) = (num(3), num(4), num(5));
            run_many(&rom, 0..=count.saturating_sub(1), |i| random_session(&rom, seed, i, irqs));
        }
        "synth" => {
            let (seed, count, irqs) = (num(3), num(4), num(5));
            run_many(&rom, 0..=count.saturating_sub(1), |i| synth_session(&rom, seed, i, irqs));
        }
        "short" => {
            let (seed, count, irqs) = (num(3), num(4), num(5));
            run_many(&rom, 0..=count.saturating_sub(1), |i| synth_short_session(&rom, seed, i, irqs));
        }
        "one" => {
            // one songs <number> <interrupts> [from to] | one random <seed> <run> <interrupts> [from to]
            let dump_at = |i| (a.len() > i + 1).then(|| (num(i), num(i + 1)));
            let (s, dump) = match a[3].as_str() {
                "songs" => (song_session(&rom, num(4), num(5)), dump_at(6)),
                "synth" => (synth_session(&rom, num(4), num(5), num(6)), dump_at(7)),
                "short" => (synth_short_session(&rom, num(4), num(5), num(6)), dump_at(7)),
                _ => (random_session(&rom, num(4), num(5), num(6)), dump_at(7)),
            };
            for (at, c) in &s.commands {
                println!("interrupt {at}: command {c:02x?}");
            }
            if std::env::var("FUZZ_SHOW").is_ok() {
                for (at, bytes) in s.patches.iter().filter(|p| p.0 >= 0x48000) {
                    println!("data at region {at:#x} (linear address {:#x}): {}", at - 0x8000, bytes.iter().map(|b| format!("{b:02x}")).collect::<Vec<_>>().join(" "));
                }
            }
            summary(&run_session(&rom, &s, dump, None));
        }
        _ => eprintln!("unknown mode"),
    }
}

fn summary(r: &Report) {
    println!(
        "Z80 {} writes, Rust {}, identical {} (+{} acknowledgements), spills {}",
        r.real.total, r.drv.total, r.matched_main, r.matched_acks, r.spill_intervals
    );
    match &r.first_divergence {
        None if r.identical() => println!("IDENTICAL"),
        None => println!("identical prefix, different lengths"),
        Some(d) => println!("DIVERGENCE : {d}"),
    }
}

/// Runs the `range` runs in parallel and summarises.
fn run_many(rom: &Rom, range: std::ops::RangeInclusive<usize>, make: impl Fn(usize) -> Session + Sync) {
    let next = AtomicUsize::new(*range.start());
    let last = *range.end();
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let results = std::sync::Mutex::new(Vec::new());
    let coverage = std::sync::Mutex::new(vec![false; 0x10000]);
    std::thread::scope(|sc| {
        for _ in 0..threads {
            sc.spawn(|| loop {
                let i = next.fetch_add(1, Ordering::Relaxed);
                if i > last {
                    break;
                }
                let s = make(i);
                let mut cov = vec![false; 0x10000];
                let r = run_session(rom, &s, None, Some(&mut cov));
                for (c, x) in coverage.lock().unwrap().iter_mut().zip(&cov) {
                    *c |= x;
                }
                results.lock().unwrap().push((i, r.identical(), r.matched_main + r.matched_acks, r.first_divergence, r.slow));
            });
        }
    });
    if let Ok(path) = std::env::var("FUZZ_COVERAGE") {
        let bytes: Vec<u8> = coverage.into_inner().unwrap().iter().map(|&c| c as u8).collect();
        std::fs::write(path, bytes).unwrap();
    }
    let mut results = results.into_inner().unwrap();
    results.sort_by_key(|r| r.0);
    let slow = results.iter().filter(|r| r.4 && r.3.is_none()).count();
    let ok = results.iter().filter(|r| r.1 || (r.4 && r.3.is_none())).count() - slow;
    let writes: usize = results.iter().map(|r| r.2).sum();
    for (i, identical, _, div, slow) in &results {
        if !identical && !(*slow && div.is_none()) {
            println!("run {i}: {}", div.as_deref().unwrap_or("different lengths"));
        }
    }
    println!(
        "{ok} identical runs out of {} ({writes} writes compared); {slow} abandoned: the Z80 stayed busy for more than {MAX_BUSY} interrupts (execution time not modelled)",
        results.len()
    );
}
