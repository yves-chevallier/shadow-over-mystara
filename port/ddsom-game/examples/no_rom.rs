//! The game without ROM: does a machine built from the extracted resources alone play like the one with the ROMs?
//!
//!   no_rom <roms> <eeprom> <assets> <scenario>:<frames> ...
//!
//! Two machines without 68000 (`ddsom_game::install_no_68k`) with the Rust sound driver replay each scenario:
//!   R: contents read from the ROMs (`cps2::Contents::from_roms`);
//!   A: contents rebuilt from the assets folder (`ddsom_assets::rebuild::contents`), no ROM file
//!       opened: 68000 program reduced to its tables, Z80 region without program, DSP reduced to its tables.
//! At every frame: the displayed frame, the work RAM, the GFX RAM, the object RAM, the palette, the shared RAM
//! and the audio samples must be identical. A stop of the game without 68000 (address without native) on both
//! machines at the same frame ends the scenario without counting it as a failure (a hole of the no-68000 mode, not of the
//! resources). Output: "IDENTICAL" or the first difference, per scenario.

use std::path::Path;

use cps2::{Contents, Machine};

fn machine(contents: Contents, eeprom: Option<&[u8]>) -> Machine {
    let mut m = Machine::ddsom(contents, eeprom);
    ddsom_game::install_no_68k(&mut m);
    m.sound.use_rust_driver();
    m
}

/// A frame, or the stop message of the game without 68000.
fn image(m: &mut Machine) -> Result<Vec<[u8; 3]>, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| m.run_frame())).map_err(|e| {
        let msg = e.downcast_ref::<String>().cloned().unwrap_or_default();
        match msg.strip_prefix(ddsom_game::no68k::STOP) {
            Some(m) => m.chars().take(200).collect(),
            None => std::panic::resume_unwind(e),
        }
    })
}

/// First part of the state that differs between the two machines.
fn gap(r: &Machine, a: &Machine) -> Option<String> {
    let first = |x: &[u16], y: &[u16]| x.iter().zip(y).position(|(p, q)| p != q);
    if let Some(i) = first(&r.bus.ram, &a.bus.ram) {
        return Some(format!("RAM {:06x}: {:04x} instead of {:04x}", 0xff0000 + 2 * i, a.bus.ram[i], r.bus.ram[i]));
    }
    if let Some(i) = first(&r.bus.gfxram, &a.bus.gfxram) {
        return Some(format!("GFX RAM {:06x}", 0x900000 + 2 * i));
    }
    for k in 0..2 {
        if first(&r.bus.objram[k], &a.bus.objram[k]).is_some() {
            return Some(format!("RAM d'objets, banque {k}"));
        }
    }
    if first(&r.bus.palette, &a.bus.palette).is_some() {
        return Some("palette".into());
    }
    if let Some(i) = r.bus.qram.iter().zip(&a.bus.qram).position(|(p, q)| p != q) {
        return Some(format!("shared RAM {i:03x}"));
    }
    None
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 5 {
        eprintln!("usage: no_rom <roms> <eeprom> <assets> <scenario>:<frames> ...");
        std::process::exit(2);
    }
    let ee = std::fs::read(&a[2]).ok();
    let mut failures = 0;
    for spec in &a[4..] {
        let (path, frames) = spec.rsplit_once(':').expect("scenario:frames");
        let frames: usize = frames.parse().expect("frames");
        let script = ddsom_game::scenario::Scenario::load(Path::new(path)).expect("scenario");
        let roms = Contents::from_roms(Path::new(&a[1]), None, None).expect("roms");
        let assets = ddsom_assets::rebuild::contents(Path::new(&a[3])).expect("assets");
        let (mut mr, mut ma) = (machine(roms, ee.as_deref()), machine(assets, ee.as_deref()));
        let t0 = std::time::Instant::now();
        let mut verdict = format!("IDENTICAL over {frames} frames");
        for f in 0..frames {
            script.apply(f, &mut mr);
            script.apply(f, &mut ma);
            let (ir, ia) = (image(&mut mr), image(&mut ma));
            let (sr, sa): (Vec<_>, Vec<_>) = (std::mem::take(&mut mr.sound.audio), std::mem::take(&mut ma.sound.audio));
            let diff = match (&ir, &ia) {
                (Err(_), Err(_)) => {
                    verdict = format!("IDENTICAL over {f} frames (then both stop without 68000: {})", ir.unwrap_err());
                    break;
                }
                (Ok(x), Ok(y)) if x != y => Some("displayed frame".to_string()),
                (Ok(_), Ok(_)) if sr != sa => Some("audio samples".to_string()),
                (Ok(_), Ok(_)) => gap(&mr, &ma),
                (Err(m), Ok(_)) => Some(format!("stop of the ROM machine alone: {m}")),
                (Ok(_), Err(m)) => Some(format!("stop of the assets machine alone: {m}")),
            };
            if let Some(d) = diff {
                verdict = format!("DIFFERENT at frame {}: {d}", f + 1);
                failures += 1;
                break;
            }
        }
        println!("{path} : {verdict} ({:.1?})", t0.elapsed());
    }
    std::process::exit(if failures == 0 { 0 } else { 1 });
}
