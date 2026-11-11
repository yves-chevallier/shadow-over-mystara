//! Census of the original code still reachable from the EFFCT handlers (table `0x39ce0`).
//!
//! For each type whose handler is native, an object of that type is created in the EFFCT pool (variant
//! and states drawn on a grid, owner = a player or an active enemy), then the handler is called
//! frame after frame on the hybrid machine, recording every 68000 instruction executed
//! during these calls. Output: per type and variant, the Rust sites that still execute 68000
//! (`file:line routine`), with an example state; then the addresses of the EFFCT zone executed.
//!
//!   census_effct <roms> <eeprom> <scenario> <warm-up frames> [comma-separated types]
//!
//! `RECENS_VARIANTES=n` (16 by default), `RECENS_PAS=n` (steps per trajectory, 40), `RECENS_TOURS=n` (8).
//! Combinations outside the original's tables (a variant or state the table does not provide for) make
//! the original code jump into data: they show up too, to be sorted by hand.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ddsom_game::mem::{rd16, rd8, wr16, wr8, A5};
use ddsom_game::objects::pools::{ENEMY, EFFCT};

/// Code zone of the EFFCT handlers.
const ZONE: std::ops::Range<u32> = 0xa8594..0xba310;
/// At most this many 68000 instructions per call (beyond that, the original has gone off the rails).
const BUDGET: u64 = 100_000;

thread_local! {
    static PCS: RefCell<BTreeSet<u32>> = const { RefCell::new(BTreeSet::new()) };
    static SITES: RefCell<BTreeSet<(String, u32)>> = const { RefCell::new(BTreeSet::new()) };
    static COUNT: RefCell<u64> = const { RefCell::new(0) };
}

fn trace(pc: u32) {
    let n = COUNT.with(|c| {
        *c.borrow_mut() += 1;
        *c.borrow()
    });
    if n > BUDGET {
        std::panic::panic_any("budget");
    }
    if ZONE.contains(&pc) {
        PCS.with(|p| p.borrow_mut().insert(pc));
    }
    let site = ddsom_game::call_site().map_or(("outside call".to_string(), 0), |(l, r)| (format!("{}:{}", l.file().rsplit('/').next().unwrap(), l.line()), r));
    SITES.with(|s| s.borrow_mut().insert(site));
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.next() as usize % v.len()]
    }
}

fn main() {
    // nested calls (natives, original code) consume a lot of stack
    std::thread::Builder::new().stack_size(1 << 30).spawn(principal).unwrap().join().unwrap();
}

fn principal() {
    let a: Vec<String> = std::env::args().collect();
    let (Some(roms), Some(eeprom), Some(scenario), Some(warmup)) = (a.get(1), a.get(2), a.get(3), a.get(4)) else {
        eprintln!("usage: census_effct <roms> <eeprom> <scenario> <warm-up frames> [types]");
        std::process::exit(2);
    };
    let env = |k: &str, d: u32| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    let n_variants = env("RECENS_VARIANTES", 16);
    let step = env("RECENS_PAS", 40);
    let rounds = env("RECENS_TOURS", 8);
    let wanted: Option<Vec<u32>> = a.get(5).map(|s| s.split(',').map(|x| x.parse().unwrap()).collect());
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
    std::panic::set_hook(Box::new(|_| {}));
    let (c0, b0) = (m.cpu.clone(), m.bus.to_bus());
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let natives: BTreeSet<u32> = ddsom_game::all_natives().map(|n| n.entry).collect();
    for t in 0..92u32 {
        if wanted.as_ref().is_some_and(|v| !v.contains(&t)) {
            continue;
        }
        let entry = {
            let mut b = b0.clone();
            (m68k::Bus::read_prog16(&mut b, 0x39ce0 + 4 * t) as u32) << 16 | m68k::Bus::read_prog16(&mut b, 0x39ce2 + 4 * t) as u32
        };
        if !natives.contains(&entry) {
            println!("TYPE {t} {entry:06x}: no native");
            continue;
        }
        // (variant, site, routine) -> (number of trajectories, example)
        let mut reading: BTreeMap<(u32, String, u32), (u32, String)> = BTreeMap::new();
        let mut zone: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
        let mut panics = 0;
        for v in 0..n_variants {
            for tour in 0..rounds {
                for s5 in [0u8, 2, 4, 6, 8, 0xa, 0xc, 0xe] {
                    let (mut c, mut b) = (c0.clone(), b0.clone());
                    b.irq_horizon = u64::MAX;
                    let mut owners: Vec<u32> = ENEMY.iter().filter(|o| rd8(&mut b, o.0) != 0).map(|o| o.0).collect();
                    owners.extend((0..4).map(|j| 0xff85de + 0x200 * j));
                    let obj = EFFCT.slot(rng.next() % EFFCT.slots).0;
                    for off in (0..EFFCT.stride).step_by(2) {
                        wr16(&mut b, obj + off, 0);
                    }
                    let owner = rng.pick(&owners);
                    wr8(&mut b, obj, 1);
                    wr8(&mut b, obj + 2, t as u8);
                    wr8(&mut b, obj + 3, v as u8);
                    // first round: birth (state 0); following ones: active state, sub-state of the grid
                    // every other trajectory starts from birth (state 0), the others from the active state
                    let birth = tour % 2 == 0;
                    let s4 = if birth { 0 } else { 2 };
                    wr8(&mut b, obj + 4, s4);
                    wr8(&mut b, obj + 5, if birth { 0 } else { s5 });
                    wr8(&mut b, obj + 6, if !birth && tour >= 2 { rng.pick(&[0, 2, 4, 6]) } else { 0 });
                    wr8(&mut b, obj + 0x28, EFFCT.class);
                    for off in [0x46u32, 0x50, 0x54, 0x5a, 0x5c, 0x1e] {
                        wr16(&mut b, obj + off, owner as u16);
                    }
                    for off in [0x8u32, 0xc, 0x10] {
                        let w = rd16(&mut b, owner + off);
                        wr16(&mut b, obj + off, w);
                    }
                    if birth && s5 != 0 {
                        // birth with varied creation fields
                        for off in [0x2fu32, 0x30, 0x31, 0x42, 0x43, 0x58, 0x59] {
                            wr8(&mut b, obj + off, rng.pick(&[0, 1, 2, 3, 4, 8]));
                        }
                    }
                    if rng.next().is_multiple_of(2) {
                        // owner's action and counters, read by the bubbles and the linked effects
                        wr8(&mut b, owner + 0x3d, 2 * (rng.next() % 0x1a) as u8);
                        for off in [0x10du32, 0x10e, 0x77, 0x86, 0x43] {
                            wr8(&mut b, owner + off, rng.pick(&[0, 1, 2, 3, 4, 5]));
                        }
                    }
                    wr16(&mut b, A5 + EFFCT.free, (EFFCT.slots / 2) as u16);
                    let start = format!("v{v:x} {s4:x}/{:x}/{:x}", rd8(&mut b, obj + 5), rd8(&mut b, obj + 6));
                    for k in 0..step {
                        if rd8(&mut b, obj) == 0 || rd8(&mut b, obj + 2) != t as u8 {
                            break;
                        }
                        if k > 0 && rng.next().is_multiple_of(4) {
                            // the owner and the world move
                            wr8(&mut b, owner + 4, rng.pick(&[0, 2, 4, 6]));
                            wr8(&mut b, owner + 0x3d, (rng.next() % 0x30) as u8);
                            wr8(&mut b, obj + 0x33, rng.pick(&[0, 1]));
                            wr8(&mut b, obj + 0x57, rng.pick(&[0, 8, 0x20]));
                        }
                        c.a[0] = obj;
                        c.a[5] = A5;
                        c.d[7] = k;
                        c.d[0] = 4 * t;
                        c.a[1] = entry;
                        PCS.with(|p| p.borrow_mut().clear());
                        SITES.with(|s| s.borrow_mut().clear());
                        COUNT.with(|x| *x.borrow_mut() = 0);
                        let state = format!("{:x}/{:x}/{:x}", rd8(&mut b, obj + 4), rd8(&mut b, obj + 5), rd8(&mut b, obj + 6));
                        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ddsom_game::call(&mut c, &mut b, entry)));
                        let sites = SITES.with(|s| std::mem::take(&mut *s.borrow_mut()));
                        let pcs = PCS.with(|p| std::mem::take(&mut *p.borrow_mut()));
                        // suspicious call: the original went off the rails (exception, odd address, budget exhausted)
                        let suspect = r.is_err() || sites.iter().any(|(f, a)| f.starts_with("boot_") || a & 1 != 0);
                        for (site, routine) in sites {
                            let site = if suspect { format!("? {site}") } else { site };
                            let e = reading.entry((v, site, routine)).or_insert((0, format!("start {start} step {k} state {state}")));
                            e.0 += 1;
                        }
                        zone.entry(v).or_default().extend(pcs);
                        if r.is_err() {
                            panics += 1;
                            break;
                        }
                    }
                }
            }
        }
        println!("TYPE {t} {entry:06x} : {panics} trajectoires interrompues");
        for ((v, site, routine), (n, ex)) in &reading {
            println!("  v{v:x} {site} {routine:06x} ×{n} ({ex})");
        }
        for (v, pcs) in &zone {
            if pcs.is_empty() {
                continue;
            }
            // contiguous ranges
            let mut ranges: Vec<(u32, u32)> = Vec::new();
            for &p in pcs {
                match ranges.last_mut() {
                    Some(r) if p - r.1 <= 0x10 => r.1 = p,
                    _ => ranges.push((p, p)),
                }
            }
            let s: Vec<String> = ranges.iter().map(|(a, b)| format!("{a:06x}-{b:06x}")).collect();
            println!("  ZONE v{v:x} : {}", s.join(" "));
        }
    }
}
