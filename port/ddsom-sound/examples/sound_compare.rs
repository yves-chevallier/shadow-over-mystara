//! Differential validation on the game: the complete machine (68000 + original Z80) runs an
//! input scenario; for each 250 Hz interrupt of the Z80 the writes to the QSound and to the
//! shared RAM are collected and compared with those of the Rust driver fed with the same
//! commands (that is, with what the Z80 read from the shared RAM).
//!
//! sound_compare <roms> <eeprom> <scenario> <frames>
//!
//! `SON_REFERENCE=1`: the machine is the reference machine of `validate` (overclocked 68000) instead of the arcade one.

#[path = "support/mod.rs"]
mod support;

use cps2::sound::Event;
use ddsom_sound::{Driver, Rom, Write};
use std::path::Path;
use support::Report;

/// `SON_SHOW=from-to`: writes of both sides for these interrupts.
fn show() -> Option<(usize, usize)> {
    let v = std::env::var("SON_SHOW").ok()?;
    let (a, b) = v.split_once('-')?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, frames] = &a[..] else {
        eprintln!("usage: sound_compare <roms> <eeprom> <scenario> <frames>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    if std::env::var_os("SON_REFERENCE").is_some() {
        ddsom_game::install_reference(&mut m);
        m.set_overclock(16.0);
        m.idle = Some(ddsom_game::idle);
        m.rendering = false;
    } else {
        ddsom_rc::install(&mut m);
    }
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scenario");
    let frames: usize = frames.parse().unwrap();
    m.sound.trace = Some(Vec::new());

    let mut driver = Driver::new(Rom::load(Path::new(roms)).expect("sound rom"));
    // What the Z80 saw of the shared RAM: reconstructed from its reads.
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

    println!("{frames} frames, {irq} interrupts at 250 Hz");
    println!(
        "Z80 writes: {} (QSound {}, shared RAM {}); Rust driver: {} (QSound {}, shared RAM {})",
        report.real.total,
        report.real.qsound,
        report.real.total - report.real.qsound,
        report.drv.total,
        report.drv.qsound,
        report.drv.total - report.drv.qsound
    );
    println!(
        "identical and in the same order: {} QSound/pan writes + {} acknowledgements",
        report.matched_main, report.matched_acks
    );
    println!(
        "interrupts: {} exact, {} with work spilling over into the next one, {} without writes",
        report.exact_intervals, report.spill_intervals, report.idle_intervals
    );
    println!(
        "QSound registers identical at the end of the interrupt: {} out of {}",
        report.same_state_intervals,
        irq + 1
    );
    match &report.first_divergence {
        None if report.identical() => println!("RESULT: exact identity of the write sequences"),
        None => println!("RESULT: identical prefix, different lengths"),
        Some(d) => println!("RESULT: first divergence: {d}"),
    }
}

/// Ends an interval (from the acknowledgement of one interrupt to the next): updates the shared
/// RAM from what the Z80 read there, runs the driver and files the writes of both sides.
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
        // Z80 restarted by the 68000: the driver restarts too.
        driver.reset(shared, &mut drv);
    } else if irq > 0 {
        driver.interrupt(shared, &mut drv);
    }
    if let Some((from, to)) = show() {
        if (from..=to).contains(&irq) {
            let f = |w: &[Write]| w.iter().map(|w| match w {
                Write::Qsound { reg, data } => format!("{reg:02x}={data:x}"),
                Write::Shared { offset, value } => format!("[{offset:03x}]={value:02x}"),
            }).collect::<Vec<_>>().join(" ");
            println!("irq {irq} cmd {:02x?}\n  Z80  {}\n  Rust {}", &shared[..16], f(&real), f(&drv));
        }
    }
    report.add_interval(irq, real, drv);
}
