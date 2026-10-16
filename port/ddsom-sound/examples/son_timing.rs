//! Mesure : durée pendant laquelle le Z80 masque les interruptions au démarrage (initialisation
//! de la QSound et remise à zéro de la RAM), et retard de la routine d'interruption sur la
//! période de 250 Hz, dans la machine complète.
//!
//! son_timing <roms> <eeprom> <scénario> <images>

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, frames] = &a[..] else {
        eprintln!("usage : son_timing <roms> <eeprom> <scénario> <images>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    ddsom_rc::install(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    let marks = Rc::new(RefCell::new(Vec::new()));
    let hook = marks.clone();
    m.sound.pc_hook = Some(Box::new(move |z| match z.pc {
        0x00aa | 0x0175 | 0x0038 => hook.borrow_mut().push((z.pc, z.cycles)),
        _ => {}
    }));
    for f in 0..frames.parse().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    let marks = marks.borrow();
    let mut di = 0;
    let mut late = std::collections::BTreeMap::new();
    for &(pc, cycles) in marks.iter() {
        match pc {
            0xaa => di = cycles,
            0x175 => println!(
                "interruptions masquées pendant {} cycles Z80 ({:.2} périodes de 250 Hz)",
                cycles - di,
                (cycles - di) as f64 / 32000.0
            ),
            _ => {
                // retard sur la dernière frontière de période
                *late.entry((cycles % 32000) / 100 * 100).or_insert(0usize) += 1;
            }
        }
    }
    println!("retard de la routine d'interruption sur la période (par tranche de 100 cycles) : {late:?}");
}
