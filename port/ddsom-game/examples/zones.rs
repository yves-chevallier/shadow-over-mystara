//! Coverage of the original code by address zone: number of recompiled-block entries per address.
//!   zones <roms> <eeprom> <scenario> <frames> <start-hex> <end-hex> [...other pairs]
//! Used to spot the player states and actions actually exercised by a scenario.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;

thread_local! {
    static ZONES: RefCell<Vec<(u32, u32)>> = RefCell::new(Vec::new());
    static HITS: RefCell<BTreeMap<u32, u64>> = RefCell::new(BTreeMap::new());
}

fn run(pc: u32, c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus, dl: u64) -> Option<u32> {
    if ZONES.with(|z| z.borrow().iter().any(|&(lo, hi)| pc >= lo && pc < hi)) {
        HITS.with(|h| *h.borrow_mut().entry(pc).or_default() += 1);
    }
    ddsom_rc::run_block(pc, c, b, dl)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(&a[1]), ee.as_deref()).expect("roms");
    let zones = a[5..].chunks(2).map(|p| {
        (u32::from_str_radix(&p[0], 16).unwrap(), u32::from_str_radix(&p[1], 16).unwrap())
    });
    ZONES.with(|z| z.borrow_mut().extend(zones));
    m.executor = Some(run);
    let script = ddsom_game::scenario::Scenario::load(Path::new(&a[3])).unwrap();
    for f in 0..a[4].parse().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    HITS.with(|h| for (pc, n) in h.borrow().iter() { println!("{pc:06x} {n}") });
}
