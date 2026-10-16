//! Validation différentielle sur le jeu : la machine complète (68000 + Z80 d'origine) tourne sur
//! un scénario d'entrées ; pour chaque interruption à 250 Hz du Z80 on relève les écritures dans la
//! QSound et dans la RAM partagée, et on les compare à celles du pilote Rust nourri des mêmes
//! commandes (c'est-à-dire de ce que le Z80 a lu dans la RAM partagée).
//!
//! son_compare <roms> <eeprom> <scenario> <images>

#[path = "support/mod.rs"]
mod support;

use cps2::sound::Event;
use ddsom_sound::{Driver, Rom, Write};
use std::path::Path;
use support::Report;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, frames] = &a[..] else {
        eprintln!("usage : son_compare <roms> <eeprom> <scenario> <images>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    ddsom_rc::install(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    let frames: usize = frames.parse().unwrap();
    m.sound.trace = Some(Vec::new());

    let mut driver = Driver::new(Rom::load(Path::new(roms)).expect("rom son"));
    // Ce que le Z80 a vu de la RAM partagée : reconstitué à partir de ses lectures.
    let mut shared = vec![0u8; ddsom_sound::SHARED_SIZE];
    let mut report = Report::default();
    let mut latch = [0u8; 2];
    let mut interval: Vec<Event> = Vec::new();
    let mut irq = 0usize;

    for f in 0..frames {
        script.apply(f, &mut m);
        m.run_frame();
        for e in std::mem::take(m.sound.trace.as_mut().unwrap()) {
            if e == Event::Irq {
                run_interval(&mut interval, &mut latch, &mut shared, &mut driver, irq, &mut report);
                irq += 1;
            }
            interval.push(e);
        }
    }
    run_interval(&mut interval, &mut latch, &mut shared, &mut driver, irq, &mut report);

    println!("{frames} images, {irq} interruptions à 250 Hz");
    println!(
        "écritures du Z80 : {} (QSound {}, RAM partagée {}) ; du pilote Rust : {} (QSound {}, RAM partagée {})",
        report.real.total,
        report.real.qsound,
        report.real.total - report.real.qsound,
        report.drv.total,
        report.drv.qsound,
        report.drv.total - report.drv.qsound
    );
    println!(
        "identiques et dans le même ordre : {} écritures QSound/pan + {} acquittements",
        report.matched_main, report.matched_acks
    );
    println!(
        "interruptions : {} exactes, {} avec débordement du travail sur la suivante, {} sans écriture",
        report.exact_intervals, report.spill_intervals, report.idle_intervals
    );
    println!(
        "registres QSound identiques en fin d'interruption : {} sur {}",
        report.same_state_intervals,
        irq + 1
    );
    match &report.first_divergence {
        None if report.identical() => println!("RÉSULTAT : identité exacte des suites d'écritures"),
        None => println!("RÉSULTAT : préfixe identique, longueurs différentes"),
        Some(d) => println!("RÉSULTAT : première divergence : {d}"),
    }
}

/// Termine un intervalle (de l'acquittement d'une interruption à la suivante) : met à jour la RAM
/// partagée d'après ce que le Z80 y a lu, déroule le pilote et range les écritures des deux côtés.
fn run_interval(
    interval: &mut Vec<Event>,
    latch: &mut [u8; 2],
    shared: &mut [u8],
    driver: &mut Driver,
    irq: usize,
    report: &mut Report,
) {
    let mut real: Vec<Write> = Vec::new();
    let mut reset = false;
    for e in interval.drain(..) {
        match e {
            Event::Read(a, v) => shared[usize::from(a)] = v,
            Event::Write(a, v) => {
                reset |= a == 0xfff && v == 0x77;
                real.push(Write::Shared { offset: a, value: v });
            }
            Event::Port(2, reg) => {
                real.push(Write::Qsound { reg, data: u16::from(latch[0]) << 8 | u16::from(latch[1]) });
            }
            Event::Port(o @ 0..=1, v) => latch[usize::from(o)] = v,
            _ => {}
        }
    }
    let mut drv: Vec<Write> = Vec::new();
    if reset {
        // Remise en route du Z80 par le 68000 : le pilote redémarre aussi.
        driver.reset(shared, &mut drv);
    } else if irq > 0 {
        driver.interrupt(shared, &mut drv);
    }
    report.add_interval(irq, real, drv);
}
