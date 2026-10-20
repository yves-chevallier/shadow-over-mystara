//! Cycles à l'entrée de chaque routine native pendant une image (pour régler les coûts des natives).
//!   cyc <roms> <eeprom> <scénario> <image> <natives 0|1>
//! Mode 1 : machine hybride ; mode 0 : code d'origine seul (mêmes points d'entrée). Chaque ligne :
//! nom, cycles de la machine à l'entrée, horizon de l'IRQ (et coût annoncé en mode 1).

use std::cell::RefCell;
use std::collections::HashMap;

thread_local! {
    static NAMES: RefCell<HashMap<u32, &'static str>> = RefCell::new(HashMap::new());
    static ON: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn trace(pc: u32, c: &m68k::Cpu, b: &cps2::Cps2Bus) {
    if ON.with(|o| o.get()) {
        if let Some(n) = NAMES.with(|m| m.borrow().get(&pc).copied()) {
            println!("{n:28} cycles {:>9} horizon {:>9}", c.cycles, b.irq_horizon);
        }
    }
}

fn run_hybrid(pc: u32, c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus, dl: u64) -> Option<u32> {
    trace(pc, c, b);
    ddsom_game::run(pc, c, b, dl)
}

fn run_original(pc: u32, c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus, dl: u64) -> Option<u32> {
    trace(pc, c, b);
    ddsom_rc::run_block(pc, c, b, dl)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(std::path::Path::new(&a[1]), ee.as_deref()).unwrap();
    let hybrid = a[5] == "1";
    if hybrid {
        ddsom_game::install(&mut m);
        m.executor = Some(run_hybrid);
    } else {
        ddsom_game::install_reference(&mut m);
        m.executor = Some(run_original);
    }
    m.set_overclock(16.0);
    NAMES.with(|n| {
        for x in ddsom_game::all_natives() {
            n.borrow_mut().insert(x.entry, x.name);
        }
    });
    let s = ddsom_game::scenario::Scenario::load(std::path::Path::new(&a[3])).unwrap();
    let n: usize = a[4].parse().unwrap();
    for f in 0..n - 1 {
        s.apply(f, &mut m);
        m.run_frame();
    }
    s.apply(n - 1, &mut m);
    ON.with(|o| o.set(true));
    m.run_frame();
}
