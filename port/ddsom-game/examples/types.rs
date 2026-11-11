//! Coverage of the object types: number of calls of each handler of the dispatch tables
//! of the ITEM, BOX and EFFCT families over a scenario (original code only).
//!   types <roms> <eeprom> <scenario> <frames>
//! `TYPES_ETATS=1`: calls per state; `TYPES_SCENES=1`: calls per scene (`FF8034`) of each handler.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;

use m68k::Bus;

/// (family name, jump table, number of types)
const FAMILIES: &[(&str, u32, u32)] = &[("ITEM", 0x39f5a, 10), ("BOX", 0x39e76, 4), ("EFFCT", 0x39ce0, 64), ("ENEMY", 0x39a78, 29), ("PSHL", 0x39908, 25), ("PEFF", 0x39992, 48)];

/// (handler, type, states `+4`, `+5`, `+6`).
type StateKey = (u32, u8, u8, u8, u8);

thread_local! {
    static CALLS: RefCell<BTreeMap<u32, u64>> = const { RefCell::new(BTreeMap::new()) };
    static IMAGE: RefCell<u32> = const { RefCell::new(0) };
    static FIRST: RefCell<BTreeMap<u32, u32>> = const { RefCell::new(BTreeMap::new()) };
    /// (handler, variant, states +4 +5 +6) -> calls
    /// handler -> scene (`FF8034`) -> calls
    static SCENES: RefCell<BTreeMap<u32, BTreeMap<u8, u64>>> = const { RefCell::new(BTreeMap::new()) };
    static STATES: RefCell<BTreeMap<StateKey, u64>> = const { RefCell::new(BTreeMap::new()) };
}

fn run(pc: u32, c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus, dl: u64) -> Option<u32> {
    CALLS.with(|m| {
        if let Some(n) = m.borrow_mut().get_mut(&pc) {
            *n += 1;
            let o = c.a[0];
            let k = (pc, ddsom_game::hw::mem::rd8(b, o + 3), ddsom_game::hw::mem::rd8(b, o + 4), ddsom_game::hw::mem::rd8(b, o + 5), ddsom_game::hw::mem::rd8(b, o + 6));
            STATES.with(|e| *e.borrow_mut().entry(k).or_insert(0) += 1);
            let scene = ddsom_game::hw::mem::rd8(b, 0xff8034);
            SCENES.with(|s| *s.borrow_mut().entry(pc).or_default().entry(scene).or_insert(0) += 1);
            FIRST.with(|p| { p.borrow_mut().entry(pc).or_insert(IMAGE.with(|i| *i.borrow())); });
        }
    });
    ddsom_rc::run_block(pc, c, b, dl)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, frames] = &a[..] else {
        eprintln!("usage: types <roms> <eeprom> <scenario> <frames>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    // TYPES_OVERCLOCK=16: same rate as `validate` (no slowdowns)
    if let Some(f) = std::env::var("TYPES_OVERCLOCK").ok().and_then(|v| v.parse().ok()) {
        m.set_overclock(f);
    }
    let mut inputs = Vec::new();
    for &(name, table, n) in FAMILIES {
        for t in 0..n {
            let a = table + 4 * t;
            let h = (m.bus.read_prog16(a) as u32) << 16 | m.bus.read_prog16(a + 2) as u32;
            inputs.push((name, t, h));
            CALLS.with(|c| c.borrow_mut().insert(h, 0));
        }
    }
    m.executor = Some(run);
    let script = ddsom_game::core::scenario::Scenario::load(Path::new(scenario)).expect("scenario");
    for f in 0..frames.parse().unwrap() {
        IMAGE.with(|i| *i.borrow_mut() = f as u32);
        script.apply(f, &mut m);
        m.run_frame();
    }
    if std::env::var_os("TYPES_ETATS").is_some() {
        for ((h, variant, e4, e5, e6), n) in STATES.with(|e| e.borrow().clone()) {
            let (name, t, _) = inputs.iter().find(|x| x.2 == h).copied().unwrap();
            println!("ETAT {name} {t} variant {variant:02x} states {e4:02x} {e5:02x} {e6:02x}: {n}");
        }
        return;
    }
    if std::env::var_os("TYPES_SCENES").is_some() {
        for (name, t, h) in inputs {
            let scenes: Vec<String> = SCENES.with(|s| s.borrow().get(&h).map(|m| m.iter().map(|(sc, n)| format!("{sc:02x}:{n}")).collect()).unwrap_or_default());
            if !scenes.is_empty() {
                println!("{name} {t} {h:06x} scenes {}", scenes.join(" "));
            }
        }
        return;
    }
    for (name, t, h) in inputs {
        println!("{name} {t} {h:06x} {} {}", CALLS.with(|c| c.borrow()[&h]), FIRST.with(|p| p.borrow().get(&h).copied().unwrap_or(0)));
    }
}
