//! Differential fuzz of the zone change of the exit door (SET 7, 0xcb29c): the paths that cede control
//! to the fade 0x1460, which `fuzz families` does not reach (all players must be ready at the same time).
//!
//!   fuzz_door <roms> <eeprom> <scenario> <warm-up> <trials>
//!
//! `DDSOM_PORTE=29`: the passage door (SET 29, 0xd8dce) instead, which the scenarios do not reach: state 2
//! with active players drawn near the door (exit kind `+0x48` drawn, 0 to 3), or state 4.
//!
//! Each trial starts from the game state after the warm-up, places a door of drawn variant, already initialised,
//! in state 2 with all players ready (exit kind `+0x48` drawn, 0 to 3), or in state 4 (return
//! from zone, remembered fields drawn). The original code runs alone (interpreter) until it returns or
//! until the first yield of the fade 0x1460 (`jsr 0xa40`); the native likewise (it calls the fade in Rust,
//! which yields: `Unwound`). Both memories are compared (except the task table and stacks), with A0 and D7 of the
//! context stored by the yield, and the continuation stored in the pending calls (the door's step) with the
//! return address of the original call. Then each finishes the fade without waiting: the original by
//! returning immediately from each `jsr 0xa40`, the native by executing the task's steps until the handler
//! returns. The final memories are compared.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

use cps2::Cps2Bus;
use ddsom_game::mem::{rd32, wr16, wr32, wr8, A5};
use ddsom_game::tasks::table::{self, Resume, Next};
use m68k::Cpu;

/// Entries of the two doors (SET types 7 and 29).
const EXIT: u32 = 0xcb29c;
const PASSAGE: u32 = 0xd8dce;
const FADING: u32 = 0x1460;
/// Yield wrapper (`jsr 0xa40`) called by each step of the fade.
const CESSION: u32 = 0xa40;
const SENTINEL: u32 = 0xfffe;
/// Task table FF0064 and stacks: specific to each machine.
const IGNORED: std::ops::Range<u32> = 0xff_0000..0xff_0e66;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn among<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.next() as usize % v.len()]
    }
}

/// Original code alone until the return to the sentinel, or until `stop`. True if `stop` is reached. With
/// `without_cession`, each `jsr 0xa40` returns immediately (the fade runs without waiting).
fn original(c: &mut Cpu, b: &mut Cps2Bus, stop: u32, without_cession: bool) -> Option<bool> {
    for _ in 0..2_000_000 {
        if c.pc == SENTINEL {
            return Some(false);
        }
        if c.pc == stop {
            return Some(true);
        }
        if without_cession && c.pc == CESSION {
            c.pc = rd32(b, c.a[7]);
            c.a[7] = c.a[7].wrapping_add(4);
            continue;
        }
        c.step(b);
    }
    None
}

/// The step where the current task, which has yielded, will resume (user-mode SR, like `kernel::launch`).
fn resume(c: &mut Cpu, b: &mut Cps2Bus) -> Option<ddsom_game::tasks::Step> {
    let t = table::table(b);
    let Resume::Step(step) = t.slots[t.current].resume else { return None };
    c.set_sr(0);
    Some(step)
}

fn calls(c: &mut Cpu, b: &mut Cps2Bus, address: u32) {
    c.a[7] = c.a[7].wrapping_sub(4);
    wr32(b, c.a[7], SENTINEL);
    c.pc = address;
}

/// Differences between the two memories (outside the ignored zone), and between A0 / D7.
fn gaps(c1: &Cpu, b1: &Cps2Bus, c2: &Cpu, b2: &Cps2Bus) -> Vec<String> {
    let mut d = Vec::new();
    ddsom_game::mem::diff_bus(b1, b2, IGNORED, &mut d);
    if c1.a[0] != c2.a[0] {
        d.push(format!("a0={:08x} instead of {:08x}", c1.a[0], c2.a[0]));
    }
    if c1.d[7] as u16 != c2.d[7] as u16 {
        d.push(format!("d7={:08x} instead of {:08x}", c1.d[7], c2.d[7]));
    }
    d
}

fn main() {
    std::thread::Builder::new().stack_size(1 << 30).spawn(principal).unwrap().join().unwrap();
}

fn principal() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, warmup, tests, ..] = &a[..] else {
        eprintln!("usage: fuzz_door <roms> <eeprom> <scenario> <warm-up> <trials>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    m.set_overclock(16.0);
    ddsom_game::install(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scenario");
    for f in 0..warmup.parse::<usize>().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    ddsom_game::set_trace(|_| {});
    if std::env::var_os("DDSOM_PANIQUES").is_none() {
        std::panic::set_hook(Box::new(|_| {}));
    }
    let passage = std::env::var("DDSOM_PORTE").is_ok_and(|v| v == "29");
    let (entry, kind, variants) = if passage { (PASSAGE, 29u8, 3) } else { (EXIT, 7, 0x38) };
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let (mut summary, mut seen_gaps, mut fadings, mut abandons) = (0u32, 0u32, 0u32, 0u32);
    for test in 0..tests.parse::<u32>().unwrap() {
        let (mut c, mut b) = (m.cpu.clone(), m.bus.clone());
        b.irq_horizon = u64::MAX;
        let door = ddsom_game::objects::pools::SET.slot(rng.next() % 32).0;
        for off in (0..0x50).step_by(2) {
            wr16(&mut b, door + off, 0);
        }
        wr8(&mut b, door, 1);
        wr8(&mut b, door + 2, kind);
        wr8(&mut b, door + 3, (rng.next() % variants) as u8);
        wr8(&mut b, door + 0x28, 0x10);
        // initialisation (state 0) by the native, on the copy: fields of the door table
        c.a[0] = door;
        c.a[5] = A5;
        for (off, v) in [(0x3d, 0), (0x50, 0), (0x42, 0)] {
            wr8(&mut b, A5 + off, v);
        }
        wr16(&mut b, A5 + 0x52, 0);
        ddsom_game::call(&mut c, &mut b, entry);
        let state4 = rng.next().is_multiple_of(3);
        if state4 {
            wr8(&mut b, door + 4, 4);
            for off in [0x30u32, 0x34, 0x38, 0x3c, 0x4a] {
                wr32(&mut b, door + off, rng.next() << 4);
            }
            for off in [0x18u32, 0x22, 0x14, 0x1c] {
                wr16(&mut b, door + off, rng.next() as u16 & 0x3ff);
            }
            for off in [0x49u32, 0x25, 0x26] {
                wr8(&mut b, door + off, rng.among(&[0, 1, 2]));
            }
            for j in 0..4u32 {
                let p = 0xff_85de + 0x200 * j;
                wr8(&mut b, p + 4, rng.among(&[0, 2, 2, 4]));
            }
        } else if passage {
            wr8(&mut b, door + 4, 2);
            wr8(&mut b, door + 0x40, 0);
            wr8(&mut b, door + 0x48, rng.among(&[0, 0, 1, 1, 2, 3]));
            // players active or not, near the door (footprint 0x60 x 0x40) or a bit beyond
            for j in 0..4u32 {
                let p = 0xff_85de + 0x200 * j;
                wr8(&mut b, p, rng.among(&[0, 1, 1, 2]));
                for (field, door_field, range_) in [(8u32, 8u32, 0x30u32), (0x10, 0x10, 0x20)] {
                    let gap = (rng.next() % (4 * range_)) as u16;
                    let v = ddsom_game::mem::rd16(&mut b, door + door_field).wrapping_add(gap).wrapping_sub(2 * range_ as u16);
                    wr16(&mut b, p + field, v);
                }
            }
        } else {
            wr8(&mut b, door + 4, 2);
            wr8(&mut b, door + 0x48, rng.among(&[0, 0, 1, 1, 2, 3]));
            wr8(&mut b, door + 0x17, 0xf);
            wr8(&mut b, door + 0x1a, 0xf);
            wr8(&mut b, door + 0x16, rng.among(&[4, 4, 1, 0]));
            wr8(&mut b, door + 0x24, rng.next() as u8 & 3);
            wr8(&mut b, A5 + 0x35, rng.among(&[1, 2, 3, 4]));
            wr16(&mut b, door + 0x20, 5);
        }
        wr16(&mut b, A5 + 0x52, if rng.next().is_multiple_of(2) { 0 } else { door as u16 });
        // a few objects of the other pools suspended or not (wake-up, release)
        for pool in ddsom_game::objects::pools::POOLS {
            for i in 0..pool.slots {
                let o = pool.slot(i).0;
                if ddsom_game::mem::rd8(&mut b, o) != 0 && rng.next().is_multiple_of(3) {
                    wr8(&mut b, o + 0x40, rng.among(&[0, 1, 2]));
                }
            }
        }
        c.a[0] = door;
        c.a[5] = A5;
        c.d[7] = rng.next() & 0x1f;
        c.d[0] = kind as u32 * 4;
        c.a[1] = entry;
        let (mut c2, mut b2) = (c.clone(), b.clone());
        // the original, up to the entry of the fade (return address at the top of the stack), then up to its yield
        calls(&mut c2, &mut b2, entry);
        let Some(fading) = original(&mut c2, &mut b2, FADING, false) else {
            abandons += 1;
            continue;
        };
        let ret = rd32(&mut b2, c2.a[7]);
        if fading && original(&mut c2, &mut b2, CESSION, false) != Some(true) {
            abandons += 1;
            continue;
        }
        // the native: the fade cedes control (`call` then stops on a panic), or it returns
        let r = catch_unwind(AssertUnwindSafe(|| ddsom_game::call(&mut c, &mut b, entry)));
        let native_fading = match r {
            Ok(()) => false,
            Err(e) if e.downcast_ref::<String>().is_some_and(|m| m.contains("ceded control")) => true,
            Err(_) => {
                println!("trial {test}: native panicked");
                seen_gaps += 1;
                continue;
            }
        };
        summary += 1;
        if fading != native_fading {
            println!("trial {test}: fade {fading} (original) / {native_fading} (native)");
            seen_gaps += 1;
            continue;
        }
        let step = if fading { resume(&mut c, &mut b) } else { None };
        let d = gaps(&c, &b, &c2, &b2);
        if !d.is_empty() {
            println!("trial {test} state4={state4} variant {:02x}: before the fade {}", ddsom_game::mem::rd8(&mut b2, door + 3), d.join(", "));
            seen_gaps += 1;
            continue;
        }
        if !fading {
            continue;
        }
        fadings += 1;
        // the rest of the fade: the door's step, at the return address of the original call
        let next = table::table(&mut b).frames().top().map(|k| k.next);
        if !matches!(next, Some(Next::Step(e)) if e.address() == ret) {
            println!("trial {test}: continuation {next:x?} instead of {ret:06x}");
            seen_gaps += 1;
            continue;
        }
        // end of the fade without waiting: each step of the task, until the handler returns
        let mut step = step;
        let r = catch_unwind(AssertUnwindSafe(|| {
            for _ in 0..64 {
                let Some(e) = step else { break };
                ddsom_game::tasks::resume_step(e, &mut b);
                ddsom_game::tools::frames::apply_tools_return(&mut c, &mut b);
                if c.pc == SENTINEL {
                    return true;
                }
                step = resume(&mut c, &mut b);
            }
            false
        }));
        if r.ok() != Some(true) || original(&mut c2, &mut b2, SENTINEL, true) != Some(false) {
            println!("trial {test}: resumption interrupted");
            seen_gaps += 1;
            continue;
        }
        let d = gaps(&c, &b, &c2, &b2);
        if !d.is_empty() {
            println!("trial {test} state4={state4} variant {:02x}: after the fade {}", ddsom_game::mem::rd8(&mut b2, door + 3), d.join(", "));
            seen_gaps += 1;
        }
    }
    println!("{summary} trials compared, {fadings} with fade, {abandons} abandons, {seen_gaps} mismatches");
    println!("{}", if seen_gaps == 0 { "FUZZ OK" } else { "FUZZ MISMATCHES" });
}
