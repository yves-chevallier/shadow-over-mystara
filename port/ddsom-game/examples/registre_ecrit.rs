//! Qui a laissé ce registre ? Pendant l'image donnée, à chaque passage en <pc>, montre A0, D<r> et les douze
//! derniers changements de D<r> (PC de l'instruction suivante, valeur) : pour trouver le gestionnaire qui laisse un
//! registre résiduel relu par un autre (machine de référence ; `1` en dernier argument : machine hybride).
//!
//!   registre_ecrit <roms> <eeprom> <scénario> <image> <pc hex> <n° de registre D> [1]
use std::sync::Mutex;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(std::path::Path::new(&a[1]), ee.as_deref()).unwrap();
    let natives = a.get(7).map_or(false, |v| v == "1");
    if natives { ddsom_game::install(&mut m) } else { ddsom_game::install_reference(&mut m) }
    m.set_overclock(16.0);
    m.idle = Some(ddsom_game::idle);
    let s = ddsom_game::scenario::Scenario::load(std::path::Path::new(&a[3])).unwrap();
    let n: usize = a[4].parse().unwrap();
    let target = u32::from_str_radix(&a[5], 16).unwrap();
    let r: usize = a[6].parse().unwrap();
    for f in 0..n - 1 {
        s.apply(f, &mut m);
        m.run_frame();
    }
    s.apply(n - 1, &mut m);
    static HIST: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());
    static LAST: Mutex<u32> = Mutex::new(0);
    m.step_hook = Some(Box::new(move |c: &m68k::Cpu, _: &cps2::Cps2Bus, pc: u32| {
        let mut last = LAST.lock().unwrap();
        let mut h = HIST.lock().unwrap();
        if c.d[r] != *last {
            h.push((pc, c.d[r]));
            if h.len() > 12 { h.remove(0); }
            *last = c.d[r];
        }
        if pc == target {
            println!("en {pc:06x} : A0={:08x} D{r}={:08x} ; écritures avant (pc suivant, valeur) : {:x?}", c.a[0], c.d[r], *h);
        }
    }));
    m.run_frame();
}
