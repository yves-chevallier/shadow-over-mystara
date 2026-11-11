//! Number of passes per frame at the given PC (for example 1c86e, the world update: one per frame in normal time,
//! about a hundred in the "step storm", see docs/couverture.md). Natives disabled by `DDSOM_SANS=` (empty: all),
//! otherwise the PC is seen only if it is executed outside a native.
//!
//! pc_hits <roms> <eeprom> <scenario> <start frame> <end frame> <pc hex>
use std::cell::Cell;
use std::rc::Rc;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let m = cps2::Machine::load_ddsom(std::path::Path::new(&a[1]), ee.as_deref()).unwrap();
    let mut m = ddsom_game::install(m);
    m.set_overclock(std::env::var("DDSOM_OVERCLOCK").ok().and_then(|v| v.parse().ok()).unwrap_or(16.0));
    let s = ddsom_game::scenario::Scenario::load(std::path::Path::new(&a[3])).unwrap();
    let (from, to): (usize, usize) = (a[4].parse().unwrap(), a[5].parse().unwrap());
    let target = u32::from_str_radix(&a[6], 16).unwrap();
    let count = Rc::new(Cell::new(0u32));
    let c2 = count.clone();
    for f in 0..to {
        s.apply(f, &mut m);
        if f + 1 >= from {
            m.executor = Some(ddsom_game::run_native_only);
            let c3 = c2.clone();
            m.step_hook = Some(Box::new(move |_: &m68k::Cpu, _: &cps2::Cps2Bus, pc: u32| {
                if pc == target {
                    c3.set(c3.get() + 1);
                }
            }));
        } else {
            m.executor = Some(ddsom_game::run);
            m.step_hook = None;
        }
        count.set(0);
        m.run_frame();
        if f + 1 >= from {
            println!("{} : {}", f + 1, count.get());
        }
    }
}
