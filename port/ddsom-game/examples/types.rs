//! Couverture des types d'objets : nombre d'appels de chaque gestionnaire des tables de dispatch
//! des familles ITEM, BOX et EFFCT sur un scénario (code d'origine seul).
//!   types <roms> <eeprom> <scénario> <images>
//! `TYPES_ETATS=1` : appels par état ; `TYPES_SCENES=1` : appels par scène (`FF8034`) de chaque gestionnaire.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;

use m68k::Bus;

/// (nom de la famille, table de sauts, nombre de types)
const FAMILLES: &[(&str, u32, u32)] = &[("ITEM", 0x39f5a, 10), ("BOX", 0x39e76, 4), ("EFFCT", 0x39ce0, 64), ("ENEMY", 0x39a78, 29)];

thread_local! {
    static APPELS: RefCell<BTreeMap<u32, u64>> = RefCell::new(BTreeMap::new());
    static IMAGE: RefCell<u32> = const { RefCell::new(0) };
    static PREMIERE: RefCell<BTreeMap<u32, u32>> = RefCell::new(BTreeMap::new());
    /// (gestionnaire, variante, états +4 +5 +6) -> appels
    /// gestionnaire -> scène (`FF8034`) -> appels
    static SCENES: RefCell<BTreeMap<u32, BTreeMap<u8, u64>>> = RefCell::new(BTreeMap::new());
    static ETATS: RefCell<BTreeMap<(u32, u8, u8, u8, u8), u64>> = RefCell::new(BTreeMap::new());
}

fn run(pc: u32, c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus, dl: u64) -> Option<u32> {
    APPELS.with(|m| {
        if let Some(n) = m.borrow_mut().get_mut(&pc) {
            *n += 1;
            let o = c.a[0];
            let k = (pc, ddsom_game::mem::rd8(b, o + 3), ddsom_game::mem::rd8(b, o + 4), ddsom_game::mem::rd8(b, o + 5), ddsom_game::mem::rd8(b, o + 6));
            ETATS.with(|e| *e.borrow_mut().entry(k).or_insert(0) += 1);
            let scene = ddsom_game::mem::rd8(b, 0xff8034);
            SCENES.with(|s| *s.borrow_mut().entry(pc).or_default().entry(scene).or_insert(0) += 1);
            PREMIERE.with(|p| { p.borrow_mut().entry(pc).or_insert(IMAGE.with(|i| *i.borrow())); });
        }
    });
    ddsom_rc::run_block(pc, c, b, dl)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, frames] = &a[..] else {
        eprintln!("usage : types <roms> <eeprom> <scénario> <images>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    let mut entrees = Vec::new();
    for &(nom, table, n) in FAMILLES {
        for t in 0..n {
            let a = table + 4 * t;
            let h = (m.bus.read_prog16(a) as u32) << 16 | m.bus.read_prog16(a + 2) as u32;
            entrees.push((nom, t, h));
            APPELS.with(|c| c.borrow_mut().insert(h, 0));
        }
    }
    m.executor = Some(run);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    for f in 0..frames.parse().unwrap() {
        IMAGE.with(|i| *i.borrow_mut() = f as u32);
        script.apply(f, &mut m);
        m.run_frame();
    }
    if std::env::var_os("TYPES_ETATS").is_some() {
        for ((h, variante, e4, e5, e6), n) in ETATS.with(|e| e.borrow().clone()) {
            let (nom, t, _) = entrees.iter().find(|x| x.2 == h).copied().unwrap();
            println!("ETAT {nom} {t} variante {variante:02x} états {e4:02x} {e5:02x} {e6:02x} : {n}");
        }
        return;
    }
    if std::env::var_os("TYPES_SCENES").is_some() {
        for (nom, t, h) in entrees {
            let scenes: Vec<String> = SCENES.with(|s| s.borrow().get(&h).map(|m| m.iter().map(|(sc, n)| format!("{sc:02x}:{n}")).collect()).unwrap_or_default());
            if !scenes.is_empty() {
                println!("{nom} {t} {h:06x} scènes {}", scenes.join(" "));
            }
        }
        return;
    }
    for (nom, t, h) in entrees {
        println!("{nom} {t} {h:06x} {} {}", APPELS.with(|c| c.borrow()[&h]), PREMIERE.with(|p| p.borrow().get(&h).copied().unwrap_or(0)));
    }
}
