//! Coverage of the native routines: how many times each native entry point is reached over a
//! scenario (one "address name calls" per line, including zero calls).
//!   coverage <roms> <eeprom> <scenario> <frames> [index.tsv]
//! With the function index (decomp/index.tsv): original code only, calls of each function.
//! Used to flag the routines not exercised (hence not validated) by the scenarios.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;

thread_local! {
    static CALLS: RefCell<BTreeMap<u32, u64>> = RefCell::new(BTreeMap::new());
}

fn run(pc: u32, c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus, dl: u64) -> Option<u32> {
    CALLS.with(|m| {
        if let Some(n) = m.borrow_mut().get_mut(&pc) {
            *n += 1;
        }
    });
    ddsom_game::run(pc, c, b, dl)
}

/// Original code only (to count the calls of the original functions, native or not).
fn run_original(pc: u32, c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus, dl: u64) -> Option<u32> {
    CALLS.with(|m| {
        if let Some(n) = m.borrow_mut().get_mut(&pc) {
            *n += 1;
        }
    });
    ddsom_rc::run_block(pc, c, b, dl)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (Some(roms), Some(eeprom), Some(scenario), Some(frames)) = (a.get(1), a.get(2), a.get(3), a.get(4)) else {
        eprintln!("usage: coverage <roms> <eeprom> <scenario> <frames> [index.tsv]");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    CALLS.with(|c| {
        for n in ddsom_game::all_natives() {
            c.borrow_mut().insert(n.entry, 0);
        }
    });
    if let Some(index) = a.get(5) {
        for l in std::fs::read_to_string(index).expect("index").lines().skip(1) {
            if let Some(addr) = l.split('\t').next().and_then(|x| u32::from_str_radix(x, 16).ok()) {
                CALLS.with(|c| {
                    c.borrow_mut().entry(addr).or_insert(0);
                });
            }
        }
    }
    m.executor = Some(if a.get(5).is_some() { run_original } else { run });
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scenario");
    for f in 0..frames.parse().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    for (entry, n) in CALLS.with(|c| c.borrow().clone()) {
        println!("{entry:06x} {n}");
    }
}
