//! Nombre de passages par image au PC donné (par exemple 1c86e, la mise à jour du monde : une par image en temps normal,
//! une centaine dans la « tempête de pas », voir docs/couverture.md). Natives désactivées par `DDSOM_SANS=` (vide : toutes),
//! sans quoi le PC n'est vu que s'il est exécuté hors d'une native.
//!
//! passages <roms> <eeprom> <scénario> <image début> <image fin> <pc hexa>
use std::cell::Cell;
use std::rc::Rc;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(std::path::Path::new(&a[1]), ee.as_deref()).unwrap();
    ddsom_game::install(&mut m);
    m.set_overclock(16.0);
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
