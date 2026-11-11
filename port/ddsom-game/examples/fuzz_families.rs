//! Fuzzing of the ITEM / BOX / EFFCT family handlers on perturbed real states.
//!
//! The scenario runs the game with the original code alone (the timeline is the game's). At the
//! calls of a rewritten handler (one in `period`), the real state is taken and `tests`
//! copies are drawn from it with a few bytes perturbed (fields of the object, of its player, scene flags),
//! then the original code and the native routine are run on each, and compared (RAM, registers).
//! This exercises branches that the scenarios do not reach. Absurd states are
//! harmless: the parts not rewritten run as original code in both versions.
//!
//! The types that the scenarios never exercise are tested by *forcing* them: the last argument
//! (`ITEM:2,ITEM:3,EFFCT:7`) lists types whose native routine is tried on copies of real objects
//! of the same family whose type byte (`+2`) is replaced, with variant and states drawn at
//! random.
//!
//!   fuzz_families <roms> <eeprom> <scenario> <frames> [period [trials [seed [FAMILY:type,...]]]]

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

use cps2::Cps2Bus;
use m68k::Bus;
use m68k::Cpu;

use ddsom_game::mem::{rd16, rd32, rd8, wr16, wr8};

#[derive(Default)]
struct Summary {
    tests: u64,
    gaps: u64,
    panics: u64,
    abandons: u64,
    examples: Vec<String>,
}

/// (family name, jump table, number of types)
const FAMILIES: &[(&str, u32, u32)] = &[("ITEM", 0x39f5a, 10), ("BOX", 0x39e76, 4), ("EFFCT", 0x39ce0, 92)];

thread_local! {
    /// entry address of a handler -> family
    static INPUTS: RefCell<BTreeMap<u32, usize>> = const { RefCell::new(BTreeMap::new()) };
    /// forced types: (family, type, entry address)
    static FORCES: RefCell<Vec<(usize, u32, u32)>> = const { RefCell::new(Vec::new()) };
    static FAMILY_CALLS: RefCell<BTreeMap<usize, u64>> = const { RefCell::new(BTreeMap::new()) };
    static SUMMARY: RefCell<BTreeMap<&'static str, Summary>> = const { RefCell::new(BTreeMap::new()) };
    static CALLS: RefCell<BTreeMap<u32, u64>> = const { RefCell::new(BTreeMap::new()) };
    static RNG: RefCell<u64> = const { RefCell::new(0x9e37_79b9_7f4a_7c15) };
    static PARAMS: RefCell<(u64, u32)> = const { RefCell::new((16, 8)) };
}

fn rng() -> u64 {
    RNG.with(|a| {
        let mut x = *a.borrow();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        *a.borrow_mut() = x;
        x
    })
}

fn chooses<T: Copy>(v: &[T]) -> T {
    v[(rng() % v.len() as u64) as usize]
}

/// Bytes of the object that the handlers test (offset, plausible values).
const OBJECT_BYTES: &[(u32, &[u8])] = &[
    (0x01, &[0, 1, 2, 3]),
    (0x03, &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0x0a, 0x10, 0x11]),
    (0x04, &[0, 2, 4, 6]),
    (0x05, &[0, 2, 4, 6, 8, 0x0a]),
    (0x06, &[0, 2, 4]),
    (0x14, &[0, 1]),
    (0x15, &[0, 1, 2, 0x0c]),
    (0x16, &[0, 1]),
    (0x22, &[0, 1]),
    (0x2c, &[0, 0x20]),
    (0x2d, &[0, 1, 2]),
    (0x2f, &[0, 1, 2, 3, 4]),
    (0x33, &[0, 1]),
    (0x39, &[0, 1, 2, 3, 4, 5, 6, 7]),
    (0x40, &[0, 1]),
    (0x43, &[0, 1]),
    (0x53, &[0, 1, 0x80]),
    (0x57, &[0, 1, 2, 3]),
    (0x58, &[0, 1, 2]),
    (0x59, &[0, 1, 2]),
    (0x5a, &[0, 1, 2, 9]),
    (0x5b, &[0, 1]),
    (0x60, &[0, 1, 2, 3, 4]),
    (0x61, &[0, 1]),
    (0x62, &[0, 1, 2, 3]),
    (0x6b, &[0, 1, 2, 3]),
    (0x6e, &[0, 1]),
    (0x6f, &[0, 1, 2, 8]),
    (0x70, &[0, 1]),
    (0x71, &[0, 1]),
    (0x76, &[0, 1]),
    (0x77, &[0, 1]),
    (0x7c, &[0, 1]),
    (0x7d, &[0, 1]),
    (0x7e, &[0, 1]),
    (0x7f, &[0, 1]),
    (0x30, &[0, 1, 2]),
    (0x31, &[0, 1]),
];

/// Words of the object (offset, plausible values).
const OBJECT_WORDS: &[(u32, &[u16])] = &[
    (0x0c, &[0, 0x10, 0xfff0]),
    (0x0e, &[0, 0x8000]),
    (0x42, &[0, 1, 2, 0x3c, 0x3d, 0x100, 0x4b0, 0xffff]),
    (0x50, &[0x85de, 0x87de, 0x89de, 0x8bde]),
    (0x54, &[0x85de, 0x87de, 0x89de, 0x8bde]),
    (0x46, &[0x85de, 0x87de]),
    (0x1e, &[0x85de, 0x87de]),
    (0x56, &[0, 1, 2, 0x3c]),
    (0x58, &[0, 1, 2, 0x30]),
    (0x5c, &[0, 1, 2, 0x85de, 0x87de]),
    (0x14, &[0, 0x100, 0xff00, 0x380]),
    (0x18, &[0, 0x300, 0xfd00]),
    (0x1a, &[0, 0xffd8, 0xffc0]),
    (0x1c, &[0, 0x40]),
    (0x30, &[1, 2, 3]),
    (0x32, &[0, 0xfe00]),
    (0x8, &[0, 0x40, 0x100, 0x200, 0x300]),
    (0x10, &[0, 0x40, 0x80, 0xc0]),
];

/// Bytes of the owner player.
const PLAYER_BYTES: &[(u32, &[u8])] = &[
    (0x00, &[0, 1, 1, 1, 2]),
    (0x02, &[0, 1, 2, 3, 4, 5]),
    (0x38, &[0, 1, 2, 3, 4, 5]),
    (0x182, &[0, 1]),
    (0x03, &[0, 2, 4, 6, 8, 0x0a, 0x0b]),
    (0x04, &[0, 2, 4, 6]),
    (0x22, &[0, 1]),
    (0x23, &[0, 1, 2]),
    (0x2c, &[0, 0x20]),
    (0x33, &[0, 1]),
    (0x7d, &[0, 1, 2, 3]),
    (0x140, &[0x56, 0x23, 0x20, 0x29, 1, 2, 3, 4, 5]),
    (0x141, &[0x56, 0x23, 0x29]),
    (0x142, &[0x56, 0x2c]),
    (0x143, &[0x56, 0x30]),
    (0x144, &[0x56]),
    (0x145, &[0x56]),
    (0x148, &[0, 1, 5, 8, 9]),
    (0x149, &[0, 1, 9]),
    (0x14a, &[0, 9]),
    (0x180, &[0, 1]),
];

/// Scene variables (offset from A5, plausible values).
const SCENE_BYTES: &[(i32, &[u8])] = &[
    (0x2c, &[0, 1, 2, 3, 4, 7, 8, 9]),
    (0x36, &[0, 1]),
    (0x3d, &[0, 1]),
    (0x42, &[0, 1]),
    (0x44, &[0, 1]),
    (0x45, &[0, 1, 8, 9]),
    (0x4f, &[0, 1]),
    (0x6f, &[0, 1, 2, 3]),
    (-0x5ac4, &[0, 1]),
    (-0x5ac6, &[0, 1, 2, 3]),
    (-0x55c8, &[0, 4, 0x0c, 0x10]),
];

/// Perturbs the copied state; returns the description of the changes.
fn perturbs(b: &mut Cps2Bus, obj: u32) -> String {
    let mut notes = Vec::new();
    let n = 1 + rng() % 4;
    let owner = {
        let p = rd16(b, obj + 0x50);
        0xff_0000 | p as u32
    };
    for _ in 0..n {
        match rng() % 10 {
            0..=4 => {
                let (off, vals) = chooses(OBJECT_BYTES);
                let v = chooses(vals);
                wr8(b, obj + off, v);
                notes.push(format!("obj+{off:02x}={v:02x}"));
            }
            5 | 6 => {
                let (off, vals) = chooses(OBJECT_WORDS);
                let v = chooses(vals);
                wr16(b, obj + off, v);
                notes.push(format!("obj+{off:02x}.w={v:04x}"));
            }
            7 | 8 => {
                if (0xff_8000..0x100_0000).contains(&owner) {
                    let (off, vals) = chooses(PLAYER_BYTES);
                    let v = chooses(vals);
                    wr8(b, owner + off, v);
                    notes.push(format!("joueur+{off:02x}={v:02x}"));
                }
            }
            9 if rng().is_multiple_of(2) => {
                // the object is placed on a player (box overlap, proximity...)
                let player = 0xff_85de + 0x200 * (rng() % 4) as u32;
                let delta = chooses(&[0u16, 0, 4, 0xfffc, 0x10, 0xfff0, 0x40, 0xffc0]);
                for off in [0x08u32, 0x0c, 0x10] {
                    let v = rd16(b, player + off).wrapping_add(if rng().is_multiple_of(3) { delta } else { 0 });
                    wr16(b, obj + off, v);
                }
                notes.push(format!("pos=joueur@{player:06x}{delta:+#x}"));
            }
            _ => {
                let (off, vals) = chooses(SCENE_BYTES);
                let v = chooses(vals);
                wr8(b, 0xff_8000u32.wrapping_add(off as u32), v);
                notes.push(format!("A5{off:+#x}={v:02x}"));
            }
        }
    }
    notes.join(" ")
}

/// Runs the original code until it returns; false if the routine does not return fast enough.
fn run_original_code(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    let ret = rd32(b, c.a[7]);
    let sp_after = c.a[7].wrapping_add(4);
    let mut guard = 0;
    while !(c.pc == ret && c.a[7] == sp_after) {
        if ddsom_rc::run_block(c.pc, c, b, u64::MAX).is_none() {
            c.step(b);
        }
        guard += 1;
        if guard > 200_000 || c.cycles > 5_000_000_000 {
            return false;
        }
    }
    true
}

fn test(n: &'static ddsom_game::Native, c: &Cpu, b: &Cps2Bus, perturbation: bool, force: Option<u32>) {
    let mut c1 = c.clone();
    let mut b1 = b.clone();
    let mut notes = String::new();
    if let Some(t) = force {
        // synthetic object: the same real object, of another type
        let obj = c.a[0];
        wr8(&mut b1, obj + 2, t as u8);
        wr8(&mut b1, obj + 3, chooses(&[0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0x0a, 0x0c, 0x10, 0x11, 0x14]));
        wr8(&mut b1, obj + 4, chooses(&[0, 0, 2, 2, 4, 6]));
        wr8(&mut b1, obj + 5, chooses(&[0, 0, 2, 4, 6, 8, 0x0a]));
        wr8(&mut b1, obj + 6, chooses(&[0, 0, 2, 4]));
        c1.pc = n.entry;
        wr16(&mut b1, obj + 0x50, chooses(&[0x85de, 0x87de, 0x89de, 0x8bde]));
        wr16(&mut b1, obj + 0x54, chooses(&[0x85de, 0x87de, 0x89de, 0x8bde, obj as u16]));
        wr16(&mut b1, obj + 0x5c, chooses(&[0x85de, 0x87de, obj as u16]));
        notes = format!(
            "type {t} variant {:02x} states {:02x} {:02x} {:02x}; ",
            rd8(&mut b1, obj + 3),
            rd8(&mut b1, obj + 4),
            rd8(&mut b1, obj + 5),
            rd8(&mut b1, obj + 6)
        );
    }
    if perturbation {
        notes += &perturbs(&mut b1, c.a[0]);
    }
    let (mut c2, mut b2) = (c1.clone(), b1.clone());
    let (c_start, b_start) =
        if std::env::var_os("FUZZ_TRACE").is_some() { (Some(c1.clone()), Some(b1.clone())) } else { (None, None) };
    let obj = c.a[0];
    let before: Vec<u8> = (0..0x60).map(|i| rd8(&mut b2, obj + i)).collect();
    if !run_original_code(&mut c2, &mut b2) {
        SUMMARY.with(|m| m.borrow_mut().entry(n.name).or_default().abandons += 1);
        return;
    }
    // native routine on the perturbed copy (same starting state as the original)
    let result = catch_unwind(AssertUnwindSafe(|| n.execute(&mut c1, &mut b1)));
    let mut diffs = Vec::new();
    if let Err(e) = result {
        let msg = e
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default();
        SUMMARY.with(|m| {
            let mut m = m.borrow_mut();
            let summary = m.entry(n.name).or_default();
            summary.tests += 1;
            summary.panics += 1;
            if summary.examples.len() < 3 {
                summary.examples.push(format!("PANIQUE {msg} ; {notes} ; objet {obj:06x} {}", dump(&before)));
            }
        });
        return;
    }
    if c1.d[7] != c2.d[7] {
        diffs.push(format!("d7={:08x} instead of {:08x}", c1.d[7], c2.d[7]));
    }
    if c1.a[0] != c2.a[0] {
        diffs.push(format!("a0={:08x} instead of {:08x}", c1.a[0], c2.a[0]));
    }
    let sp = c2.a[7] & 0xff_ffff;
    ddsom_game::mem::diff_bus(&b1, &b2, sp.saturating_sub(0x400)..sp, &mut diffs);
    SUMMARY.with(|m| {
        let mut m = m.borrow_mut();
        let summary = m.entry(n.name).or_default();
        summary.tests += 1;
        if !diffs.is_empty() {
            summary.gaps += 1;
            if summary.examples.len() < 3 {
                if let (Some(mut ct), Some(mut bt)) = (c_start, b_start) {
                    // trace of the original (pc and duration of each instruction) for diagnosis
                    let ret = rd32(&mut bt, ct.a[7]);
                    let sp_after = ct.a[7].wrapping_add(4);
                    let mut trace = String::new();
                    let mut n = 0;
                    while !(ct.pc == ret && ct.a[7] == sp_after) && n < 3000 {
                        let (pc, before) = (ct.pc, ct.cycles);
                        ct.step(&mut bt);
                        trace += &format!(" {pc:06x}+{}", ct.cycles - before);
                        n += 1;
                    }
                    summary.examples.push(format!("TRACE{trace}"));
                }
                summary.examples.push(format!("{} ; {notes} ; objet {obj:06x} {}", diffs.join(", "), dump(&before)));
            }
        }
    });
}

fn dump(bytes: &[u8]) -> String {
    bytes.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join("")
}

/// Native routine of a family handler (written on `Ctx` or on `Cx`).
fn handler(entry: u32) -> Option<&'static ddsom_game::Native> {
    ddsom_game::families::NATIVES.iter().chain(ddsom_game::families::natives()).find(|n| n.entry == entry)
}

fn run(pc: u32, c: &mut Cpu, b: &mut Cps2Bus, dl: u64) -> Option<u32> {
    // forced types: trials on synthetic objects at each (periodic) call of a handler
    // of the same family
    if let Some(family) = INPUTS.with(|e| e.borrow().get(&pc).copied()) {
        let (period, tests) = PARAMS.with(|p| *p.borrow());
        let k = FAMILY_CALLS.with(|m| {
            let mut m = m.borrow_mut();
            let e = m.entry(family).or_insert(0);
            *e += 1;
            *e
        });
        if k % period == 1 {
            let forces: Vec<_> = FORCES.with(|f| f.borrow().iter().filter(|x| x.0 == family).copied().collect());
            for (_, type_force, entry) in forces {
                if let Some(n) = handler(entry) {
                    for _ in 0..tests * 2 {
                        test(n, c, b, true, Some(type_force));
                    }
                }
            }
        }
    }
    let family = INPUTS.with(|e| e.borrow().contains_key(&pc));
    if let Some(n) = handler(pc).filter(|_| family) {
        let (period, tests) = PARAMS.with(|p| *p.borrow());
        let k = CALLS.with(|m| {
            let mut m = m.borrow_mut();
            let e = m.entry(pc).or_insert(0);
            *e += 1;
            *e
        });
        if k % period == 1 {
            // the real state first (without perturbation), then perturbed variants
            test(n, c, b, false, None);
            for _ in 0..tests {
                test(n, c, b, true, None);
            }
        }
    }
    ddsom_rc::run_block(pc, c, b, dl)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (Some(roms), Some(eeprom), Some(scenario), Some(frames)) = (a.get(1), a.get(2), a.get(3), a.get(4)) else {
        eprintln!("usage: fuzz_families <roms> <eeprom> <scenario> <frames> [period [trials [seed]]]");
        std::process::exit(2);
    };
    let period: u64 = a.get(5).map_or(16, |s| s.parse().unwrap());
    let tests: u32 = a.get(6).map_or(8, |s| s.parse().unwrap());
    if let Some(g) = a.get(7) {
        RNG.with(|x| *x.borrow_mut() = 0x9e37_79b9_7f4a_7c15 ^ g.parse::<u64>().unwrap().wrapping_mul(0x1000_0000_01b3));
    }
    PARAMS.with(|p| *p.borrow_mut() = (period, tests));
    std::panic::set_hook(Box::new(|_| {})); // panics are counted, not printed
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    let wanted = a.get(8).cloned().unwrap_or_default();
    for (i, &(name, table, n)) in FAMILIES.iter().enumerate() {
        for t in 0..n {
            let address = table + 4 * t;
            let entry = (m.bus.read_prog16(address) as u32) << 16 | m.bus.read_prog16(address + 2) as u32;
            INPUTS.with(|e| e.borrow_mut().insert(entry, i));
            if wanted.split(',').any(|x| x == format!("{name}:{t}")) {
                FORCES.with(|f| f.borrow_mut().push((i, t, entry)));
            }
        }
    }
    m.executor = Some(run);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scenario");
    for f in 0..frames.parse().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    let mut total_gaps = 0;
    for (name, summary) in SUMMARY.with(|m| std::mem::take(&mut *m.borrow_mut())) {
        println!(
            "  {name:24} {:>7} trials {:>5} mismatches {:>4} panics {:>5} abandons",
            summary.tests, summary.gaps, summary.panics, summary.abandons
        );
        total_gaps += summary.gaps + summary.panics;
        for e in summary.examples {
            println!("    {e}");
        }
    }
    println!("{}", if total_gaps == 0 { "FUZZ OK" } else { "FUZZ MISMATCHES" });
}
