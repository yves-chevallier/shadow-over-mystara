//! Census of the type handlers of the object families: for each entry of the dispatch tables
//! (type → handler), tells whether a native routine covers it, then exercises it on synthetic
//! objects and counts the 68000 code the native still executes (delegated sub-states,
//! original subroutines), the out-of-contract calls (the original runs instead) and the yields.
//!
//!   census <roms> <eeprom> <scenario> <warm-up> <trajectories> [CLASS,...]
//!
//! Classes: PSHL PEFF ESHL SET SOBJ ITEM BOX (all by default). `DDSOM_ENTREE=address`: a single
//! handler; `DDSOM_ETATS=1`: half of the trajectories start from drawn `+4..+7` states. A trajectory creates a fresh object
//! (state 0, variant 0, or drawn among the first `DDSOM_VARIANTES`) then calls the handler up to 120 times, perturbing the inputs it reads; every other one
//! also draws scene variables (game, transition, game mode; marked "drawn").
//! Output: one line per entry, then the 68000 ranges executed per entry.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ddsom_game::mem::{rd8, wr16, wr32, wr8, A5};
use ddsom_game::objects::pools::{self, Pool};

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

/// (name, pool, table, number of entries): the table stops at the next dispatch loop.
const CLASSES: &[(&str, &Pool, u32, u32)] = &[
    ("PSHL", &pools::PSHL, 0x39908, (0x3996c - 0x39908) / 4),
    ("PEFF", &pools::PEFF, 0x39992, (0x39a52 - 0x39992) / 4),
    ("ESHL", &pools::ESHL, 0x39b96, (0x39cba - 0x39b96) / 4),
    ("BOX", &pools::BOX, 0x39e76, 4),
    ("SET", &pools::SET, 0x39eac, 34),
    ("ITEM", &pools::ITEM, 0x39f5a, 10),
    ("SOBJ", &pools::SOBJ, 0x39fa8, 16),
];

#[derive(Default)]
struct Summary {
    calls: u64,
    calls_68k: u64,
    instructions: u64,
    outside_contract: u64,
    cessions: u64,
    panics: BTreeSet<String>,
    /// 68000 addresses executed: (legitimate trajectory, drawn states)
    pcs: BTreeMap<u32, (u64, u64)>,
}

static PROGRESS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

thread_local! {
    static ACTIVE: RefCell<bool> = const { RefCell::new(false) };
    static SEEN: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    static DEPTH: RefCell<u32> = const { RefCell::new(0) };
    static CALLED_NATIVES: RefCell<u32> = const { RefCell::new(0) };
}

/// Depth of nested natives: an inconsistent synthetic object can make the code jump to
/// address 0 (restart), which calls itself back endlessly.
fn hook(_pc: u32, entry: bool) {
    let total = CALLED_NATIVES.with(|n| {
        *n.borrow_mut() += 1;
        *n.borrow()
    });
    if total > 2_000_000 {
        CALLED_NATIVES.with(|n| *n.borrow_mut() = 0);
        panic!("endless native loop");
    }
    let p = DEPTH.with(|p| {
        let mut p = p.borrow_mut();
        if entry { *p += 1 } else { *p = p.saturating_sub(1) }
        *p
    });
    if p > 400 && entry {
        DEPTH.with(|p| *p.borrow_mut() = 0);
        CALLED_NATIVES.with(|n| *n.borrow_mut() = 0);
        panic!("endless nested natives (jump outside the code)");
    }
}

fn trace(pc: u32) {
    if ACTIVE.with(|a| *a.borrow()) {
        let n = SEEN.with(|v| {
            let mut v = v.borrow_mut();
            v.push(pc);
            v.len()
        });
        if n > 300_000 {
            ACTIVE.with(|a| *a.borrow_mut() = false);
            panic!("endless 68000 loop");
        }
    }
}

/// Contiguous ranges (within 0x10 bytes) of a set of addresses.
fn ranges(pcs: impl Iterator<Item = u32>) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = Vec::new();
    for pc in pcs {
        match out.last_mut() {
            Some((_, end)) if pc <= *end + 0x10 => *end = pc,
            _ => out.push((pc, pc)),
        }
    }
    out
}

fn main() {
    // nested natives (original code calls that call natives again) need a large stack
    // watchdog: a trajectory stuck for more than a minute stops everything (diagnose under gdb)
    std::thread::spawn(|| loop {
        let before = PROGRESS.load(std::sync::atomic::Ordering::Relaxed);
        std::thread::sleep(std::time::Duration::from_secs(60));
        if PROGRESS.load(std::sync::atomic::Ordering::Relaxed) == before {
            eprintln!("trajectory stuck");
            std::process::abort();
        }
    });
    std::thread::Builder::new().stack_size(1 << 30).spawn(principal).unwrap().join().unwrap();
}

fn principal() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, warmup, rounds, ..] = &a[..] else {
        eprintln!("usage: census <roms> <eeprom> <scenario> <warm-up> <trajectories> [CLASS,...]");
        std::process::exit(2);
    };
    let wanted: Option<Vec<&str>> = a.get(6).map(|s| s.split(',').collect());
    let filter: Option<u32> = std::env::var("DDSOM_ENTREE").ok().map(|s| u32::from_str_radix(s.trim_start_matches("0x"), 16).unwrap());
    // DDSOM_SAUTER=address,...: handlers not exercised (synthetic objects that make them loop)
    let jump: Vec<u32> = std::env::var("DDSOM_SAUTER")
        .map(|s| s.split(',').filter_map(|x| u32::from_str_radix(x.trim_start_matches("0x"), 16).ok()).collect())
        .unwrap_or_default();
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    m.set_overclock(16.0);
    let mut m = ddsom_game::install(m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scenario");
    for f in 0..warmup.parse::<usize>().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    ddsom_game::set_trace(trace);
    ddsom_game::set_native_hook(hook);
    if std::env::var_os("DDSOM_PANIQUES").is_none() {
        std::panic::set_hook(Box::new(|_| {}));
    }
    let rounds: u32 = rounds.parse().unwrap();
    let mut rng = Rng(0x1234_5678_9abc_def1);
    let (c0, b0) = (m.cpu.clone(), m.bus.to_bus());
    let mut b = b0.clone();
    let mut summaries: BTreeMap<u32, Summary> = BTreeMap::new();
    let mut lines = Vec::new();
    for &(name, pool, table, n) in CLASSES {
        if wanted.as_ref().is_some_and(|v| !v.contains(&name)) {
            continue;
        }
        let mut views = BTreeSet::new();
        for kind in 0..n {
            let entry = (b.fetch16(table + 4 * kind) as u32) << 16 | b.fetch16(table + 4 * kind + 2) as u32;
            let native = ddsom_game::all_natives().find(|nat| nat.entry == entry);
            let duplicate = !views.insert(entry);
            lines.push((name, kind, entry, native.map(|n| n.name), duplicate));
            if native.is_none() || duplicate || filter.is_some_and(|f| f != entry) || jump.contains(&entry) {
                continue;
            }
            if std::env::var_os("DDSOM_VERBOSE").is_some() {
                eprintln!("{name} {kind} {entry:06x}");
            }
            let summary = summaries.entry(entry).or_default();
            for t in 0..rounds {
                let tires = t % 2 == 1;
                trajectory(&c0, &b0, pool, kind as u8, entry, tires, &mut rng, summary);
            }
        }
    }
    println!("# dispatch table entries");
    println!("class type address native calls calls_68k instr_68k outside_contract cessions");
    for &(name, kind, entry, native, duplicate) in &lines {
        let b = summaries.get(&entry);
        let detail = match (native, b) {
            (None, _) => "68000".to_string(),
            (Some(_), _) if duplicate => "(same handler as a previous type)".to_string(),
            (Some(n), Some(b)) => format!(
                "{n} {} {} {} {} {}{}",
                b.calls,
                b.calls_68k,
                b.instructions,
                b.outside_contract,
                b.cessions,
                if b.panics.is_empty() { String::new() } else { format!(" paniques: {:?}", b.panics) }
            ),
            (Some(n), None) => format!("{n} (not exercised)"),
        };
        println!("{name} {kind} {entry:06x} {detail}");
    }
    println!("# 68000 executed by the natives (range: legitimate/drawn states)");
    for (entry, b) in &summaries {
        if b.pcs.is_empty() {
            continue;
        }
        let list: Vec<String> = ranges(b.pcs.keys().copied())
            .into_iter()
            .map(|(d, f)| {
                let (l, t) = b.pcs.range(d..=f).fold((0, 0), |(l, t), (_, &(x, y))| (l + x, t + y));
                format!("{d:06x}-{f:06x}:{l}/{t}")
            })
            .collect();
        println!("{entry:06x} {}", list.join(" "));
    }
}

#[allow(clippy::too_many_arguments)]
fn trajectory(
    c0: &m68k::Cpu,
    b0: &cps2::Cps2Bus,
    pool: &Pool,
    kind: u8,
    entry: u32,
    tires: bool,
    rng: &mut Rng,
    summary: &mut Summary,
) {
    let (mut c, mut b) = (c0.clone(), b0.clone());
    b.irq_horizon = u64::MAX;
    let mut owners: Vec<u32> = pools::ENEMY.iter().filter(|o| rd8(&mut b, o.0) != 0).map(|o| o.0).collect();
    owners.extend((0..4).map(|j| 0xff85de + 0x200 * j));
    let player = 0xff85de + 0x200 * (rng.next() % 4);
    let owner = if pool.class == 6 || pool.class == 0x12 { player } else { rng.among(&owners) };
    let obj = pool.slot(rng.next() % pool.slots);
    for off in (0..pool.stride).step_by(2) {
        wr16(&mut b, obj.0 + off, 0);
    }
    wr8(&mut b, obj.0, 1);
    wr8(&mut b, obj.0 + 2, kind);
    // variant: 0 by default (beyond its table, a type reads anything and may loop forever)
    let variants: u32 = std::env::var("DDSOM_VARIANTES").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    wr8(&mut b, obj.0 + 3, (rng.next() % variants) as u8);
    wr8(&mut b, obj.0 + 0x28, pool.class);
    wr8(&mut b, obj.0 + 0x2c, (rng.next() & 1) as u8);
    for off in [0x50u32, 0x46, 0x5a, 0x5c, 0x42, 0x54, 0x1e] {
        if !rng.next().is_multiple_of(4) {
            wr16(&mut b, obj.0 + off, owner as u16);
        }
    }
    for off in [0x8u32, 0xc, 0x10] {
        let v = (ddsom_game::mem::rd16(&mut b, owner + off) as u32) << 16 | (rng.next() & 0xffff);
        wr32(&mut b, obj.0 + off, v);
    }
    wr16(&mut b, obj.0 + 0xc, (rng.next() % 0x60) as u16);
    wr16(&mut b, A5 + pool.free, (pool.slots / 2) as u16);
    if std::env::var_os("DDSOM_ETATS").is_some() && rng.next().is_multiple_of(2) {
        // drawn starting states (pairs): reaches the delegated sub-states; may leave the original's tables
        for off in [4u32, 5, 6, 7] {
            wr8(&mut b, obj.0 + off, rng.among(&[0, 0, 2, 4, 6, 8, 0xa]));
        }
    }
    if tires {
        // scene variables read by the scenery and system objects: game, transition, mode, players
        for (off, vals) in [(0x35u32, &[0u8, 1, 2, 4][..]), (0x36, &[0, 1]), (0x42, &[0, 1]), (0x44, &[0, 1]), (0x50, &[0, 1]), (0x4a, &[0, 1]), (0x2c, &[0, 4, 8, 12])] {
            wr8(&mut b, A5 + off, rng.among(vals));
        }
    }
    for step in 0..120u32 {
        if rd8(&mut b, obj.0) == 0 {
            break;
        }
        if step > 0 {
            if rng.next().is_multiple_of(6) {
                wr8(&mut b, obj.0 + 0x57, rng.among(&[0x08, 0x20, 0x28, 0x10, 0x02, 0x40]));
            }
            if rng.next().is_multiple_of(8) {
                wr8(&mut b, obj.0 + 0x33, rng.next() as u8 & 1);
            }
            if rng.next().is_multiple_of(10) {
                wr8(&mut b, owner + 4, (rng.next() % 8) as u8);
            }
            if rng.next().is_multiple_of(5) {
                for off in [0x53u32, 0xa0, 0xa1, 0xa8, 0xa9, 0x3e, 0x2f, 0x42, 0x5d] {
                    if rng.next().is_multiple_of(3) {
                        wr8(&mut b, owner + off, rng.among(&[0, 1, 2, 3, 0xff]));
                    }
                }
            }
        }
        wr8(&mut b, A5 + 0x3d, rng.next().is_multiple_of(10) as u8);
        wr8(&mut b, obj.0 + 0x40, rng.next().is_multiple_of(40) as u8);
        c.a[0] = obj.0;
        c.a[5] = A5;
        c.d[7] = step;
        c.d[0] = 4 * kind as u32;
        c.a[1] = entry;
        summary.calls += 1;
        PROGRESS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        SEEN.with(|v| v.borrow_mut().clear());
        ACTIVE.with(|a| *a.borrow_mut() = true);
        DEPTH.with(|p| *p.borrow_mut() = 0);
        CALLED_NATIVES.with(|n| *n.borrow_mut() = 0);
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ddsom_game::call(&mut c, &mut b, entry)));
        ACTIVE.with(|a| *a.borrow_mut() = false);
        let seen = SEEN.with(|v| std::mem::take(&mut *v.borrow_mut()));
        if !seen.is_empty() {
            summary.calls_68k += 1;
            summary.instructions += seen.len() as u64;
            for pc in seen {
                let e = summary.pcs.entry(pc).or_default();
                if tires {
                    e.1 += 1;
                } else {
                    e.0 += 1;
                }
            }
        }
        match r {
            Ok(()) => {}
            // a yield inside a call by address stops `call` (panic)
            Err(e) if e.downcast_ref::<String>().is_some_and(|m| m.contains("ceded control")) => {
                summary.cessions += 1;
                break;
            }
            Err(e) => {
                let msg = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()));
                summary.panics.insert(msg.unwrap_or_default().chars().take(80).collect());
                break;
            }
        }
    }
}
