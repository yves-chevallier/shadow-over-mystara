//! 68000 instruction coverage (pure interpreter, original code):
//!   cov <roms> <eeprom> <scenario> <frames> <output.bin>
//! Writes a bitmap (1 bit per program word 0x000000-0x0FFFFF, bit = instruction executed at that
//! address) and prints the number of distinct instructions executed. Merge and report: tools/cov_report.py.
use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(&a[1]), ee.as_deref()).unwrap();
    // like validate: overclocked 68000 (DDSOM_OC, default 16; 1 = arcade speed)
    m.set_overclock(std::env::var("DDSOM_OC").ok().and_then(|v| v.parse().ok()).unwrap_or(16.0));
    // same reference machine as validate (step ceiling, shared fixes), but interpreted
    // instruction by instruction (without the recompiled code) so the hook sees every PC
    ddsom_game::install_reference(&mut m);
    m.executor = None;
    let s = ddsom_game::core::scenario::Scenario::load(Path::new(&a[3])).unwrap();
    let frames: usize = a[4].parse().unwrap();
    let bits = Rc::new(RefCell::new(vec![0u8; 0x100000 / 2 / 8]));
    let b2 = bits.clone();
    m.step_hook = Some(Box::new(move |_, _, pc| {
        let w = (pc as usize & 0xfffff) >> 1;
        b2.borrow_mut()[w >> 3] |= 1 << (w & 7);
    }));
    for f in 0..frames {
        s.apply(f, &mut m);
        m.run_frame();
    }
    let n: u32 = bits.borrow().iter().map(|b| b.count_ones()).sum();
    std::fs::write(&a[5], &*bits.borrow()).unwrap();
    println!("{n} distinct instructions executed over {frames} frames");
}
