//! Measurement: how long the Z80 masks interrupts at startup (QSound initialisation and
//! RAM clearing), and the lateness of the interrupt routine relative to the 250 Hz period,
//! in the complete machine.
//!
//! sound_timing <roms> <eeprom> <scenario> <frames>

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, frames] = &a[..] else {
        eprintln!("usage: sound_timing <roms> <eeprom> <scenario> <frames>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    ddsom_rc::install(&mut m);
    let script = ddsom_game::core::scenario::Scenario::load(Path::new(scenario)).expect("scenario");
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
                "interrupts masked for {} Z80 cycles ({:.2} periods of 250 Hz)",
                cycles - di,
                (cycles - di) as f64 / 32000.0
            ),
            _ => {
                // lateness relative to the last period boundary
                *late.entry((cycles % 32000) / 100 * 100).or_insert(0usize) += 1;
            }
        }
    }
    println!("lateness of the interrupt routine relative to the period (per 100-cycle bucket): {late:?}");
}
