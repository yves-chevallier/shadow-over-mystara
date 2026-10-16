//! Profil : cycles 68000 par fonction d'origine sur un scénario.
//! profile <roms> <eeprom> <scenario> <images> <decomp/index.tsv>

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

thread_local! {
    static CYCLES: RefCell<HashMap<u32, u64>> = RefCell::new(HashMap::new());
}

fn run(pc: u32, c: &mut m68k::Cpu, b: &mut cps2::Cps2Bus, dl: u64) -> Option<u32> {
    let start = c.cycles;
    let r = ddsom_rc::run_block(pc, c, b, dl);
    CYCLES.with(|m| *m.borrow_mut().entry(pc).or_default() += c.cycles - start);
    r
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(&a[1]), ee.as_deref()).expect("roms");
    m.executor = Some(run);
    let script = ddsom_game::scenario::Scenario::load(Path::new(&a[3])).unwrap();
    let frames: usize = a[4].parse().unwrap();
    for f in 0..frames {
        script.apply(f, &mut m);
        m.run_frame();
    }
    // fonctions : (adresse, nom, taille)
    let mut funcs: Vec<(u32, String, u32)> = std::fs::read_to_string(&a[5]).unwrap().lines().skip(1)
        .filter_map(|l| {
            let v: Vec<&str> = l.split('\t').collect();
            Some((u32::from_str_radix(v[0], 16).ok()?, v[1].to_string(), v[2].parse().ok()?))
        })
        .collect();
    funcs.sort();
    let mut per: HashMap<u32, u64> = HashMap::new();
    let total: u64 = CYCLES.with(|m| {
        let m = m.borrow();
        for (&pc, &cy) in m.iter() {
            let i = funcs.partition_point(|f| f.0 <= pc);
            let f = if i > 0 { funcs[i - 1].0 } else { 0 };
            *per.entry(f).or_default() += cy;
        }
        m.values().sum()
    });
    let mut v: Vec<(u32, u64)> = per.into_iter().collect();
    v.sort_by_key(|x| std::cmp::Reverse(x.1));
    println!("{} fonctions exécutées, {} cycles recompilés", v.len(), total);
    for (f, cy) in v.iter().take(40) {
        let name = funcs.iter().find(|x| x.0 == *f).map(|x| x.1.as_str()).unwrap_or("?");
        println!("{:06x} {:30} {:5.2} %", f, name, 100.0 * *cy as f64 / total as f64);
    }
}
