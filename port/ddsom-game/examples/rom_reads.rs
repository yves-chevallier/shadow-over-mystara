//! Reads of the program ROM (0x000000-0x3fffff) done as data by the Rust code, outside the execution
//! of 68000 instructions: `prog8`/`prog16`/`prog32` (decrypted program space), `rd8`/`data16`/`data32`
//! and bus reads (data space).
//!
//!   cargo build --release -p ddsom-game --example rom_reads --features rom-trace
//!   rom_reads <roms> <eeprom> <output.txt> <scenario>:<frames> ...
//!
//! Variables: `DDSOM_MASQUE=n` shifts the scenario numbers of the mask by n bits (to merge several
//! runs); `DDSOM_COUVERT=file` (written by `couverture_rom`): for each word read outside the covered
//! zones, the output adds a line `s d|p address file:line;...`: the first three callers in
//! `ddsom-game/src` of the first read.
//!
//! Each scenario is replayed on the machine without 68000 (natives only: every read counts; a stop for lack of a
//! native ends the scenario at that frame, reported on the error output). The output gives, for each zone whose words are all read by the same scenarios:
//! `d|p start end mask reads` (hexadecimal, `d`: data space, `p`: program space; `end` excluded;
//! `mask`: bit n = scenario n). The zones are compared with the resources by the example `couverture_rom`
//! de `ddsom-assets` (docs/assets.md).

use std::fmt::Write as _;
use std::path::Path;

thread_local! {
    static COVERED: std::cell::RefCell<Vec<u8>> = const { std::cell::RefCell::new(Vec::new()) };
    static ORIGINALS: std::cell::RefCell<std::collections::BTreeMap<(bool, u32), String>> = const { std::cell::RefCell::new(std::collections::BTreeMap::new()) };
}

/// Called at the first read of a word: if the resources do not cover it, records who read it.
fn original(program: bool, address: u32) {
    let covered = COVERED.with(|c| {
        let c = c.borrow();
        c.is_empty() || c[program as usize * 0x200000 + (address >> 1) as usize] != 0
    });
    if covered {
        return;
    }
    let trace = std::backtrace::Backtrace::force_capture().to_string();
    let mut sites = Vec::new();
    for l in trace.lines() {
        if let Some(at) = l.trim().strip_prefix("at ") {
            if let Some(i) = at.find("ddsom-game/src/") {
                let site = at[i + "ddsom-game/src/".len()..].trim_end_matches(|c: char| c.is_ascii_digit() || c == ':');
                let line = at.rsplitn(3, ':').nth(1).unwrap_or("");
                sites.push(format!("{site}:{line}"));
                if sites.len() == 6 {
                    break;
                }
            }
        }
    }
    ORIGINALS.with(|o| o.borrow_mut().insert((program, address), sites.join(";")));
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 5 {
        eprintln!("usage: rom_reads <roms> <eeprom> <output.txt> <scenario>:<frames> ...");
        std::process::exit(2);
    }
    let ee = std::fs::read(&a[2]).ok();
    let base: usize = std::env::var("DDSOM_MASQUE").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    if let Some(f) = std::env::var_os("DDSOM_COUVERT") {
        COVERED.with(|c| *c.borrow_mut() = std::fs::read(f).expect("coverage file"));
        cps2::bus::rom_trace::set_hook(original);
    }
    ddsom_game::refuse_hybrid();
    let mut mask = vec![[0u32; 2]; 0x200000];
    let mut reads = vec![[0u64; 2]; 0x200000];
    for (n, spec) in a[4..].iter().enumerate() {
        let (path, frames) = spec.rsplit_once(':').expect("scenario:frames");
        let mut m = cps2::Machine::load_ddsom(Path::new(&a[1]), ee.as_deref()).expect("roms");
        ddsom_game::install_no_68k(&mut m);
        let script = ddsom_game::scenario::Scenario::load(Path::new(path)).expect("scenario");
        cps2::bus::rom_trace::take();
        for f in 0..frames.parse::<usize>().unwrap() {
            script.apply(f, &mut m);
            if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| m.run_frame())) {
                let msg = e.downcast_ref::<String>().cloned().unwrap_or_default();
                eprintln!("{path}: STOP at frame {f}: {}", msg.chars().take(300).collect::<String>());
                break;
            }
        }
        let r = cps2::bus::rom_trace::take();
        for v in 0..2 {
            for (w, &c) in r.0[v].iter().enumerate().filter(|(_, &c)| c > 0) {
                mask[w][v] |= 1 << (base + n);
                reads[w][v] += c as u64;
            }
        }
        eprintln!("{path}: {} frames", frames);
    }
    let mut out = String::new();
    for (v, name) in [(0, 'd'), (1, 'p')] {
        let mut w = 0;
        while w < mask.len() {
            if mask[w][v] == 0 {
                w += 1;
                continue;
            }
            let (start, m, mut n) = (w, mask[w][v], 0);
            while w < mask.len() && mask[w][v] == m {
                n += reads[w][v];
                w += 1;
            }
            writeln!(out, "{name} {:06x} {:06x} {m:x} {n}", 2 * start, 2 * w).unwrap();
        }
    }
    ORIGINALS.with(|o| {
        for (&(p, address), sites) in o.borrow().iter() {
            writeln!(out, "s {} {address:06x} {sites}", if p { 'p' } else { 'd' }).unwrap();
        }
    });
    std::fs::write(&a[3], out).unwrap();
}
