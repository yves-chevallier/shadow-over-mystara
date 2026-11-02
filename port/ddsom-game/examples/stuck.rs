//! A frame that never ends: runs a single machine (rewritten game, or original code with
//! `original`) up to the given frame; if a frame executes more than 20 million blocks, prints
//! the most frequent PCs and stops.
//!
//! stuck <roms> <eeprom> <scenario> <frame> [origine|reecrit] [deux]

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

thread_local! {
    static BLOCKS: Cell<u64> = const { Cell::new(0) };
    static PCS: RefCell<HashMap<u32, u64>> = RefCell::new(HashMap::new());
    static ORIGINAL: Cell<bool> = const { Cell::new(false) };
}

fn count(pc: u32, c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus, dl: u64) -> Option<u32> {
    let n = BLOCKS.with(|x| {
        x.set(x.get() + 1);
        x.get()
    });
    if n > 20_000_000 {
        PCS.with(|p| *p.borrow_mut().entry(pc).or_default() += 1);
        if n > 20_200_000 {
            let mut v: Vec<_> = PCS.with(|p| p.borrow().iter().map(|(&a, &k)| (a, k)).collect());
            v.sort_by_key(|&(_, k)| std::cmp::Reverse(k));
            println!("stuck: {:?}", c);
            for (a, k) in v.iter().take(25) {
                println!("  {a:06x} : {k}");
            }
            std::process::exit(1);
        }
    }
    if ORIGINAL.with(|o| o.get()) { ddsom_rc::run_block(pc, c, b, dl) } else { ddsom_game::run(pc, c, b, dl) }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(std::path::Path::new(&a[1]), ee.as_deref()).unwrap();
    let original = a.get(5).is_some_and(|s| s == "origine");
    if original {
        ddsom_game::install_reference(&mut m);
    } else {
        ddsom_game::install(&mut m);
    }
    m.set_overclock(16.0);
    ORIGINAL.with(|o| o.set(original));
    if std::env::var("COMPTE").is_ok() {
        ddsom_game::set_count(true);
    }
    m.executor = Some(count);
    let s = ddsom_game::scenario::Scenario::load(std::path::Path::new(&a[3])).unwrap();
    let n: usize = a[4].parse().unwrap();
    // `deux`: a second machine (original code) runs in the same process, as in validate
    let mut other = a.get(6).is_some_and(|s| s == "deux").then(|| {
        let mut m2 = cps2::Machine::load_ddsom(std::path::Path::new(&a[1]), ee.as_deref()).unwrap();
        m2.set_overclock(16.0);
        ddsom_game::install_reference(&mut m2);
        m2
    });
    for f in 0..n {
        BLOCKS.with(|x| x.set(0));
        s.apply(f, &mut m);
        if let Some(m2) = other.as_mut() {
            s.apply(f, m2);
        }
        m.run_frame();
        if let Some(m2) = other.as_mut() {
            m2.run_frame();
        }
    }
    println!("{n} frames without blocking");
}
