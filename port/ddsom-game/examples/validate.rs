//! Validation of the rewrite over a scenario, frame by frame.
//!
//! Two machines replay the same inputs:
//!   A: the game, without 68000 (natives only, kernel frame loop, `ddsom_game::install_no_68k`); an
//!       address without a native stops it ("FAILED: stop without 68000");
//!   B: original code alone, overclocked 68000 (it always finishes its work within the frame: it is then
//!       deterministic frame by frame).
//! At every frame, the game state (work RAM outside stacks, GFX RAM, sprite table, palette)
//! must be identical, and the game's Rust task table must equal that of the original's RAM. `DDSOM_CHECK=1`
//! additionally compares each object handler called by the game with its original on a copy of the machine (memory;
//! diagnostic, `ddsom_game::tools::check_handler`); it fails if it compares nothing.
//!
//! The hybrid mode (`DDSOM_HYBRIDE=1`) is removed.
//!
//! Sound: machine A has the game's sound driver (Rust, `ddsom-sound`), machine B the original Z80 program. Each QSound
//! register and each shared RAM byte must receive the same values from both drivers, in the same order, up to a few
//! frames of delay (`SoundStreams`), and the shared RAM must be identical at every frame (except the pan report byte,
//! compared in the sequences).
//!
//! Machine A has no ROM (`cps2::Machine::game`): its data are extracted in memory from the ROMs
//! (`extract::install_game`), or with `DDSOM_ASSETS=<dir>` loaded from the extracted resources alone
//! (`extract::install_game_assets`, no ROM read: game without ROM, docs/sans-rom.md); machine B keeps the ROMs.
//!
//! validate <roms> <eeprom> <scenario> <frames>

use std::collections::{BTreeMap, VecDeque};
use std::path::Path;

use cps2::sound::Output;
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

/// Shared RAM byte where the sound driver reports the pan of the last voice set (docs/son.md): the original Z80 often
/// finishes a sequencer tick after the next interrupt, so the byte may change one frame later or earlier than with the
/// Rust driver, which does the whole tick at once. Its writes are compared in the sequences instead.
const PAN_REPORT: usize = 0x005;

/// Mailbox flag of the shared RAM: acknowledged (0xFF) by the interrupt routine, whose writes the original
/// interleaves with those of the main loop depending on the Z80's execution time. Left out of the sequences; the byte
/// itself is compared at every frame with the rest of the shared RAM.
const MAILBOX_FLAG: u16 = 0x00f;

/// Frames a driver write may wait for its counterpart from the other driver: the Z80 lags behind the Rust driver by
/// the time it takes to write a tick (at most one frame measured over all the scenarios).
const SOUND_MAX_LAG: usize = 4;

/// The writes of the two sound drivers (game: Rust driver; reference: Z80). Each QSound register, and each shared
/// RAM byte, must receive the same sequence of values from both, up to a delay of `SOUND_MAX_LAG` frames.
///
/// The order between registers is not required: when the Z80 is several periods late (after starting a music, which
/// takes it up to two periods), a command that arrives meanwhile is processed before the sequencer tick still pending,
/// while the Rust driver, which has no execution time, has already done that tick (docs/son.md §13, « Écart
/// restant »). The voices the command starts then begin one tick later than with the Z80. The global order is still
/// measured (`in_order`, `reordered_at`).
#[derive(Default)]
struct SoundStreams {
    /// Values not yet matched, per destination: (frame, value).
    game: BTreeMap<Dest, VecDeque<(usize, u16)>>,
    reference: BTreeMap<Dest, VecDeque<(usize, u16)>>,
    compared: u64,
    max_lag: usize,
    /// Global order: the writes of each side not yet matched in order, and the first frame where it differs.
    order: (VecDeque<Output>, VecDeque<Output>),
    in_order: u64,
    reordered_at: Option<usize>,
    /// Comparison suspended since this frame (the Z80 reordered its work while late), until both drivers are in the
    /// same state again (`resync`).
    desynced: Option<usize>,
    /// Suspensions, and the longest one in frames.
    desyncs: usize,
    longest_desync: usize,
    /// Last frame that ended with work pending in the Z80 (it is late), and whether the order broke while it was.
    z80_late_at: Option<usize>,
    reordered_late: bool,
}

/// The Z80 RAM (F000-FFFF) and the Rust driver state (`Driver::ram_image`) are the same, except for what the driver does
/// not model: current bank, temporaries, stack, unused bytes of the voices, stale fields of free voices
/// (same rules as the `sound_fuzz` tool, FUZZ_STATE).
fn same_driver_state(z80: &[u8], image: &[u8]) -> bool {
    let ignored = |a: usize| {
        matches!(a, 0xf005 | 0xf006 | 0xf007 | 0xf00d | 0xf01a)
            || (0xf050..0xf200).contains(&a)
            || (0xff00..0x10000).contains(&a)
            || (0xf200..0xfc00).contains(&a) && matches!((a - 0xf200) % 0x50, 0x21..=0x24 | 0x32 | 0x45)
    };
    (0..0x1000).all(|i| {
        let a = 0xf000 + i;
        if z80[i] == image[i] || ignored(a) {
            return true;
        }
        if (0xf200..0xfc00).contains(&a) {
            let start = i - (a - 0xf200) % 0x50;
            if z80[start] == 0 && image[start] == 0 {
                return !matches!((a - 0xf200) % 0x50, 0x00 | 0x40);
            }
        }
        if (0xfc00..0xfc48).contains(&a) {
            let start = i - (a - 0xfc00) % 0x18;
            if z80[start + 2] | z80[start + 3] == 0 && image[start + 2] | image[start + 3] == 0 {
                return !matches!((a - 0xfc00) % 0x18, 0x02 | 0x03 | 0x10);
            }
        }
        false
    })
}

/// Destination of a driver write.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Dest {
    Qsound(u8),
    Shared(u16),
}

impl SoundStreams {
    /// End of frame `frame`: compares the drivers' writes, then suspends or resumes the comparison. A mismatch is
    /// accepted only after the Z80, being late, reordered its work (the order differs since a frame where it was
    /// late); the comparison then waits until both drivers are in the same state again. Otherwise it is an error.
    fn end_frame(&mut self, frame: usize, game: &mut cps2::Machine, reference: &mut cps2::Machine) -> Result<(), String> {
        let writes = |m: &mut cps2::Machine| std::mem::take(m.sound.output.as_mut().unwrap());
        let (a, b) = (writes(game), writes(reference));
        let z80 = &reference.sound;
        let pending_work = z80.ram[1] | z80.ram[2] | z80.ram[3] != 0;
        if self.desynced.is_none() {
            let before = self.reordered_at;
            let result = self.push(frame, a, b);
            if before.is_none() && self.reordered_at.is_some() {
                self.reordered_late = self.z80_late_at.is_some_and(|l| frame - l <= 2) || pending_work;
            }
            if let Err(e) = result {
                if !(self.reordered_at.is_some() && self.reordered_late) {
                    return Err(e);
                }
                self.desynced = Some(frame);
                self.desyncs += 1;
                self.game.clear();
                self.reference.clear();
            }
        }
        if pending_work {
            self.z80_late_at = Some(frame);
        }
        // the Z80 at rest in its main loop, nothing pending, and the same state as the Rust driver: in sync again
        let idle = (0x017d..0x019a).contains(&z80.z80.pc) && !pending_work;
        if (self.desynced.is_some() || self.reordered_at.is_some()) && idle {
            if let Some(driver) = game.sound.driver.as_ref() {
                if same_driver_state(&z80.ram, &driver.ram_image()) {
                    if let Some(since) = self.desynced.take() {
                        self.longest_desync = self.longest_desync.max(frame - since);
                    }
                    self.reordered_at = None;
                    self.order.0.clear();
                    self.order.1.clear();
                }
            }
        }
        Ok(())
    }

    /// Adds the writes of frame `frame` and compares what both sides have; an error describes the first mismatch.
    fn push(&mut self, frame: usize, game: Vec<Output>, reference: Vec<Output>) -> Result<(), String> {
        let kept = |w: &&Output| !matches!(w, Output::Shared { offset: MAILBOX_FLAG, .. });
        let split = |w: &Output| match *w {
            Output::Qsound { reg, data } => (Dest::Qsound(reg), data),
            Output::Shared { offset, value } => (Dest::Shared(offset), u16::from(value)),
        };
        if self.reordered_at.is_none() {
            self.order.0.extend(game.iter().filter(kept));
            self.order.1.extend(reference.iter().filter(kept));
            while let (Some(a), Some(b)) = (self.order.0.front(), self.order.1.front()) {
                if a != b {
                    self.reordered_at = Some(frame + 1);
                    break;
                }
                self.order.0.pop_front();
                self.order.1.pop_front();
                self.in_order += 1;
            }
        }
        let mut touched = Vec::new();
        for w in game.iter().filter(kept) {
            let (d, v) = split(w);
            self.game.entry(d).or_default().push_back((frame, v));
            touched.push(d);
        }
        for w in reference.iter().filter(kept) {
            let (d, v) = split(w);
            self.reference.entry(d).or_default().push_back((frame, v));
        }
        touched.extend(reference.iter().filter(kept).map(|w| split(w).0));
        touched.sort();
        touched.dedup();
        for d in touched {
            let (Some(a), Some(b)) = (self.game.get_mut(&d), self.reference.get_mut(&d)) else { continue };
            while let (Some(&(fa, va)), Some(&(fb, vb))) = (a.front(), b.front()) {
                if va != vb {
                    let next = |q: &VecDeque<(usize, u16)>| q.iter().take(4).map(|(f, v)| format!("{v:x}@{}", f + 1)).collect::<Vec<_>>().join(" ");
                    return Err(format!(
                        "sound {d:x?}: {va:x} (frame {}) instead of {vb:x} (frame {}); next game {}; next reference {}",
                        fa + 1,
                        fb + 1,
                        next(a),
                        next(b)
                    ));
                }
                self.max_lag = self.max_lag.max(fa.abs_diff(fb));
                a.pop_front();
                b.pop_front();
                self.compared += 1;
            }
        }
        for (side, streams) in [("game", &self.game), ("reference", &self.reference)] {
            for (d, q) in streams {
                if let Some(&(f, v)) = q.front().filter(|(f, _)| frame - f > SOUND_MAX_LAG) {
                    return Err(format!("sound {d:x?}: {v:x} written by the {side} at frame {} without counterpart after {SOUND_MAX_LAG} frames", f + 1));
                }
            }
        }
        Ok(())
    }

    /// Writes still waiting for their counterpart (game, reference).
    fn pending(&self) -> (usize, usize) {
        (self.game.values().map(VecDeque::len).sum(), self.reference.values().map(VecDeque::len).sum())
    }
}

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
    // the game: a machine without ROM, its data extracted from the ROMs (or loaded from the assets)
    let contents = match std::env::var_os("DDSOM_ASSETS") {
        Some(dir) => extract::install_game_assets(Path::new(&dir)).expect("assets"),
        None => extract::install_game(Path::new(roms)).expect("roms"),
    };
    let mut ma = cps2::Machine::game(contents, ee.as_deref());
    let mut mb = load();
    ma.sound.output = Some(Vec::new());
    mb.sound.output = Some(Vec::new());
    let mut sound = SoundStreams::default();
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
    let slow: Option<u128> = std::env::var("DDSOM_LENT").ok().and_then(|v| v.parse().ok());
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
        if slow.is_some() && (f + 1) % 1000 == 0 {
            println!("frame {}: {:.1?}", f + 1, t0.elapsed());
        }
        if let Some(threshold) = slow {
            if ta.as_millis() > threshold || tb.as_millis() > threshold {
                println!("frame {} slow: rewritten {:.0?}, original {:.0?}", f + 1, ta, tb);
            }
        }
        let ram: Vec<usize> = (0..0x8000).filter(|&i| ma.bus.ram[i] != mb.bus.ram[i] && !ignored(i)).collect();
        let gfx = ma.bus.gfxram.iter().zip(&mb.bus.gfxram).filter(|(x, y)| x != y).count();
        let word_under_test = f < STARTUP && ram.len() + gfx <= 1;
        let mut what = Vec::new();
        for &i in ram.iter().take(diff_max).filter(|_| !word_under_test) {
            what.push(format!("ram {:06x}: {:04x} instead of {:04x}", 0xff0000 + 2 * i, ma.bus.ram[i], mb.bus.ram[i]));
        }
        if let Some(i) = first_diff(&ma.bus.gfxram, &mb.bus.gfxram).filter(|_| !word_under_test) {
            what.push(format!("gfxram {:06x}: {:04x} instead of {:04x}", 0x900000 + 2 * i, ma.bus.gfxram[i], mb.bus.gfxram[i]));
        }
        if let Some(i) = first_diff(&ma.obj_buffer, &mb.obj_buffer) {
            what.push(format!("sprites, mot {i}"));
        }
        // the two banks of the object RAM (the bank read by the game is that of the previous frame)
        for k in 0..2 {
            if let Some(i) = first_diff(&ma.bus.objram[k], &mb.bus.objram[k]) {
                what.push(format!("object RAM, bank {k}, word {i}: {:04x} instead of {:04x}", ma.bus.objram[k][i], mb.bus.objram[k][i]));
            }
        }
        if let Some(i) = first_diff(&ma.bus.palette, &mb.bus.palette) {
            what.push(format!("palette, mot {i}"));
        }
        if let Some(i) = (0..ma.bus.qram.len()).find(|&i| i != PAN_REPORT && ma.bus.qram[i] != mb.bus.qram[i]) {
            what.push(format!("shared RAM {i:03x}: {:02x} instead of {:02x}", ma.bus.qram[i], mb.bus.qram[i]));
        }
        if let Err(e) = sound.end_frame(f, &mut ma, &mut mb) {
            what.push(e);
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
    let (pending_game, pending_reference) = sound.pending();
    println!(
        "sound: {} driver writes identical (largest delay {} frames, {} game and {} reference writes pending); {} in the global order; {} suspensions (Z80 late), the longest {} frames{}",
        sound.compared,
        sound.max_lag,
        pending_game,
        pending_reference,
        sound.in_order,
        sound.desyncs,
        sound.longest_desync,
        sound.desynced.map(|f| format!(", suspended since frame {}", f + 1)).unwrap_or_default()
    );
    let report = ddsom_game::report();
    for (name, st) in &report {
        let errors = if check {
            format!(" {:>5} mismatches {:>5} skipped", st.mismatches, st.skipped)
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
    // DDSOM_CHECK compares the object handlers with their original (`tools::check_handler`): it must compare something
    let compared: u64 = report.values().map(|s| s.calls).sum();
    let mismatches: u64 = report.values().map(|s| s.mismatches).sum();
    if check {
        println!("DDSOM_CHECK: {compared} calls compared with the original, {mismatches} mismatches");
        if compared == 0 {
            println!("FAILED: DDSOM_CHECK compared nothing");
            std::process::exit(1);
        }
    }
    let (is_empty, capped) = ddsom_game::no68k::summary();
    println!("without 68000: {is_empty} frames ended on a busy wait, {capped} stopped by the native ceiling");
    let residue = ddsom_game::data::residue();
    println!(
        "residue: {} reads of the program ROM outside the typed tables{}",
        residue.iter().map(|r| r.1).sum::<u64>(),
        residue.iter().map(|(m, n)| format!(", {} {:#x} ×{n}", m.table, m.key)).collect::<String>()
    );
    if let Some(m) = &stop {
        println!("STOP {m}");
    }
    match failure {
        None => println!("VALID: identical state over {frames} frames"),
        Some(f) if stop.is_some() => println!("FAILED: stop without 68000 at frame {f}"),
        Some(f) => println!("FAILED: first difference at frame {f}"),
    }
}
