//! Validation of the rewrite over a scenario, frame by frame.
//!
//! Two machines replay the same inputs:
//!   A: the game, without 68000 (natives only, kernel frame loop, `ddsom_game::install_no_68k`); an
//!       address without a native stops it (« FAILED: stop without 68000 »);
//!   B: original code alone, overclocked 68000 (it always finishes its work within the frame: it is then
//!       deterministic frame by frame).
//! At every frame, the game state (work RAM outside stacks, GFX RAM, sprite table, palette)
//! must be identical, and the game's Rust task table must equal that of the original's RAM. `DDSOM_CHECK=1`
//! additionally enables the call-by-call check of the native routines (diagnostic: registers and memory compared to the original on a copy).
//!
//! The hybrid mode (`DDSOM_HYBRIDE=1`) is removed.
//!
//! `DDSOM_ASSETS=<dir>`: machine A is built from the extracted resources alone
//! (`ddsom_assets::rebuild::contents`, no ROM read: game without ROM, docs/sans-rom.md), with the Rust sound driver;
//! machine B keeps the ROMs and the Z80.
//!
//! validate <roms> <eeprom> <scenario> <frames>

use std::path::Path;

use ddsom_game::tasks::table::{gaps, read, kernel_ram};

/// 68000 frequency, as a multiple of the arcade's.
const OVERCLOCK: f64 = 16.0;

/// RAM words with no meaning of their own, or that the rewritten game no longer keeps in RAM:
/// - the kernel (`kernel_ram`): scheduler stack, current task, stack heap, task records. The
///   game keeps them in its Rust task table, compared separately with that of the original machine's RAM
///   (`gaps`);
/// - the task stacks (FF01A6-FF0DA5) and the supervisor's (up to FF0E65), in the same zone: the game no longer
///   saves there the registers of the tasks that cede control (what they read back is in their steps) and the rest
///   is only return addresses and register saves; FF0E66 and beyond (game variables relative to A5)
///   are compared;
/// - the phantom save of the sound wrappers (FF297E-FF2A19, `sound::play_wrapper`).
fn ignored(word: usize) -> bool {
    let a = 2 * word;
    kernel_ram(a as u32) || GHOST_SAVE.contains(&a)
}

/// Accepted mismatch: each sound wrapper of the original pushes D0-D7/A0-A4 (52 bytes) under its private stack pointer
/// (FF2A1A, which equals itself when the stack is empty) and pops them; the copy stays in RAM.
/// The game no longer writes it (a sound no longer depends on its caller's registers). Read trace on the original machine, all
/// scenarios (docs/son-68000.md): these words are only read back by the wrappers' own restore and by the test
/// of the TEST menu RAM (0x2d096, which puts back the value read); at most three dispatch levels, i.e. FF297E-FF2A19.
/// (Up to batch S, the whole FF2900-FF2A19 was ignored: the end of the sound queue, its heads and the alternation of
/// voices are now compared.)
const GHOST_SAVE: std::ops::Range<usize> = 0x297e..0x2a1a;

/// Startup frames (1 to 5): the original code still tests the RAM there (`ddsom_game::startup`). This test is non
/// destructive but, at the end of a frame, the word under test may carry a pattern (0x8888, 0x4444...) instead of its
/// value; the native version has no intermediate state. During these frames, at most one differing word is tolerated,
/// in work RAM or in GFX RAM; everything else is compared. In practice only frames 1 and 2 (GFX RAM) have one.
const STARTUP: usize = 5;

fn first_diff(a: &[u16], b: &[u16]) -> Option<usize> {
    a.iter().zip(b).position(|(x, y)| x != y)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, frames] = &a[..] else {
        eprintln!("usage: validate <roms> <eeprom> <scenario> <frames>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let load = || {
        let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
        m.set_overclock(std::env::var("DDSOM_OVERCLOCK").ok().and_then(|v| v.parse().ok()).unwrap_or(OVERCLOCK));
        m
    };
    ddsom_game::refuse_hybrid();
    let mut ma = match std::env::var_os("DDSOM_ASSETS") {
        Some(dir) => {
            let contents = ddsom_assets::rebuild::contents(Path::new(&dir)).expect("assets");
            let mut m = cps2::Machine::ddsom(contents, ee.as_deref());
            m.sound.use_rust_driver();
            m
        }
        None => load(),
    };
    let mut mb = load();
    ddsom_game::install_no_68k(&mut ma);
    ddsom_game::install_reference(&mut mb);
    // the frame is not compared (only the state): no rendering, it was the bulk of a frame's time
    ma.rendering = false;
    mb.rendering = false;
    mb.idle = Some(ddsom_game::idle);
    let check = std::env::var("DDSOM_CHECK").is_ok();
    let all_tasks = std::env::var("DDSOM_TACHES_TOUT").is_ok();
    ddsom_game::set_check(check);
    ddsom_game::set_check_all(std::env::var("DDSOM_CHECK_ALL").is_ok());
    ddsom_game::set_count(true);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scenario");
    let frames: usize = frames.parse().unwrap();
    let t0 = std::time::Instant::now();
    let mut failure = None;
    let mut stop: Option<String> = None;
    let lent: Option<u128> = std::env::var("DDSOM_LENT").ok().and_then(|v| v.parse().ok());
    // DDSOM_CHECK_FRAME=n: check of all registers of each native call during frame n
    // DDSOM_DIFF_MAX=n: number of differing RAM words printed (10 by default)
    let diff_max: usize = std::env::var("DDSOM_DIFF_MAX").ok().and_then(|v| v.parse().ok()).unwrap_or(10);
    let check_frame: Option<usize> = std::env::var("DDSOM_CHECK_FRAME").ok().and_then(|v| v.parse().ok());
    for f in 0..frames {
        if check_frame == Some(f + 1) {
            ddsom_game::set_check(true);
            ddsom_game::set_check_all(true);
        }
        script.apply(f, &mut ma);
        script.apply(f, &mut mb);
        let t = std::time::Instant::now();
        if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ma.run_frame())) {
            // stop of the game without 68000 (address without native...); any other panic runs its course
            match e.downcast_ref::<String>().and_then(|m| m.strip_prefix(ddsom_game::no68k::STOP)) {
                Some(m) => {
                    stop = Some(m.to_owned());
                    failure = Some(f + 1);
                    break;
                }
                None => std::panic::resume_unwind(e),
            }
        }
        let ta = t.elapsed();
        mb.run_frame();
        let tb = t.elapsed() - ta;
        // DDSOM_LENT=ms: reports frames longer than ms milliseconds (per machine)
        if lent.is_some() && (f + 1) % 1000 == 0 {
            println!("frame {}: {:.1?}", f + 1, t0.elapsed());
        }
        if let Some(threshold) = lent {
            if ta.as_millis() > threshold || tb.as_millis() > threshold {
                println!("frame {} slow: rewritten {:.0?}, original {:.0?}", f + 1, ta, tb);
            }
        }
        let ram: Vec<usize> = (0..0x8000).filter(|&i| ma.bus.ram[i] != mb.bus.ram[i] && !ignored(i)).collect();
        let gfx = ma.bus.gfxram.iter().zip(&mb.bus.gfxram).filter(|(x, y)| x != y).count();
        let word_under_test = f < STARTUP && ram.len() + gfx <= 1;
        let mut what = Vec::new();
        for &i in ram.iter().take(diff_max).filter(|_| !word_under_test) {
            what.push(format!("ram {:06x} : {:04x} au lieu de {:04x}", 0xff0000 + 2 * i, ma.bus.ram[i], mb.bus.ram[i]));
        }
        if let Some(i) = first_diff(&ma.bus.gfxram, &mb.bus.gfxram).filter(|_| !word_under_test) {
            what.push(format!("gfxram {:06x} : {:04x} au lieu de {:04x}", 0x900000 + 2 * i, ma.bus.gfxram[i], mb.bus.gfxram[i]));
        }
        if let Some(i) = first_diff(&ma.obj_buffer, &mb.obj_buffer) {
            what.push(format!("sprites, mot {i}"));
        }
        // the two banks of the object RAM (the bank read by the game is that of the previous frame)
        for k in 0..2 {
            if let Some(i) = first_diff(&ma.bus.objram[k], &mb.bus.objram[k]) {
                what.push(format!("RAM d'objets, banque {k}, mot {i} : {:04x} au lieu de {:04x}", ma.bus.objram[k][i], mb.bus.objram[k][i]));
            }
        }
        if let Some(i) = first_diff(&ma.bus.palette, &mb.bus.palette) {
            what.push(format!("palette, mot {i}"));
        }
        // differential test of the task table (DDSOM_TACHES_TOUT=1: SR and saved registers too)
        if let Some(t) = read(&ma.bus) {
            what.extend(gaps(t, &mb.bus.ram, all_tasks).into_iter().take(10));
        }
        if !what.is_empty() {
            println!("frame {}: {} different RAM words", f + 1, ram.len());
            for w in &what {
                println!("  {w}");
            }
            failure = Some(f + 1);
            break;
        }
    }
    println!("{frames} frames in {:.1?}", t0.elapsed());
    let report = ddsom_game::report();
    for (name, st) in &report {
        let errors = if check {
            format!(" {:>5} mismatches", st.mismatches)
        } else {
            String::new()
        };
        println!("  {name:28} {:>8} calls{errors}{}", st.calls, st.first.as_ref().map(|f| format!(" — {f}")).unwrap_or_default());
    }
    if std::env::var("DDSOM_CHECK_ALL").is_ok() {
        // output registers that differ from the original, per native routine
        for (name, st) in &report {
            let list: Vec<String> = st
                .regs
                .iter()
                .enumerate()
                .filter(|(_, &n)| n > 0)
                .map(|(i, n)| format!("{}{}:{n}", if i < 8 { 'd' } else { 'a' }, i % 8))
                .collect();
            if !list.is_empty() {
                println!("REGS {name} {} calls: {}", st.calls, list.join(" "));
            }
        }
    }
    let (is_empty, capped) = ddsom_game::no68k::summary();
    println!("without 68000: {is_empty} frames ended on a busy wait, {capped} stopped by the native ceiling");
    if let Some(m) = &stop {
        println!("STOP {m}");
    }
    match failure {
        None => println!("VALID: identical state over {frames} frames"),
        Some(f) if stop.is_some() => println!("FAILED: stop without 68000 at frame {f}"),
        Some(f) => println!("FAILED: first difference at frame {f}"),
    }
}
