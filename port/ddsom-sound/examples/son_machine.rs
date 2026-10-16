//! Machine complète avec le pilote Rust à la place du Z80 : le 68000 ne doit voir aucune
//! différence. Deux machines tournent en parallèle sur le même scénario (Z80 d'origine, et pilote
//! Rust) ; on compare la RAM du 68000 et la RAM partagée à chaque image, et les suites d'écritures
//! dans la QSound.
//!
//! son_machine <roms> <eeprom> <scenario> <images>

use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, frames] = &a[..] else {
        eprintln!("usage : son_machine <roms> <eeprom> <scenario> <images>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let load = || {
        let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
        ddsom_rc::install(&mut m);
        m.sound.qsound.log = Some(Vec::new());
        m
    };
    let (mut z80, mut rust) = (load(), load());
    rust.sound.use_rust_driver();
    if std::env::var("SON_TRACE").is_ok() {
        z80.sound.trace = Some(Vec::new());
        rust.sound.trace = Some(Vec::new());
    }
    let mut irq_flags: (Vec<(usize, u8)>, Vec<(usize, u8)>) = Default::default();
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    let frames: usize = frames.parse().unwrap();

    let mut first_ram = None;
    let mut first_shared = None;
    let mut shared_diffs = 0usize;
    let mut log_ends = Vec::new();
    for f in 0..frames {
        script.apply(f, &mut z80);
        script.apply(f, &mut rust);
        z80.run_frame();
        rust.run_frame();
        if z80.sound.trace.is_some() {
            use cps2::sound::Event;
            let keep = |t: Vec<Event>, out: &mut Vec<(usize, u8)>| {
                let mut after_irq = false;
                for e in t {
                    match e {
                        Event::Irq => after_irq = true,
                        Event::Read(0xf, v) if after_irq => {
                            out.push((f, v));
                            after_irq = false;
                        }
                        _ => {}
                    }
                }
            };
            keep(std::mem::take(z80.sound.trace.as_mut().unwrap()), &mut irq_flags.0);
            keep(std::mem::take(rust.sound.trace.as_mut().unwrap()), &mut irq_flags.1);
        }
        log_ends.push((z80.sound.qsound.log.as_ref().map_or(0, Vec::len), rust.sound.qsound.log.as_ref().map_or(0, Vec::len)));
        if first_ram.is_none() && z80.bus.ram != rust.bus.ram {
            first_ram = Some(f + 1);
        }
        if z80.bus.qram != rust.bus.qram {
            shared_diffs += 1;
            if first_shared.is_none() {
                let diffs: Vec<String> = (0..z80.bus.qram.len())
                    .filter(|&i| z80.bus.qram[i] != rust.bus.qram[i])
                    .map(|i| format!("{i:03x}: {:02x}/{:02x}", z80.bus.qram[i], rust.bus.qram[i]))
                    .collect();
                println!("première différence de RAM partagée à l'image {} : {}", f + 1, diffs.join(" "));
            }
            first_shared.get_or_insert(f + 1);
        }
    }
    let (lz, lr) = (z80.sound.qsound.log.take().unwrap(), rust.sound.qsound.log.take().unwrap());
    let common = lz.iter().zip(&lr).take_while(|(a, b)| a == b).count();
    println!("{frames} images");
    if !irq_flags.0.is_empty() {
        let n = irq_flags.0.iter().zip(&irq_flags.1).take_while(|(a, b)| a.1 == b.1).count();
        println!("interruptions : Z80 {} / pilote Rust {} ; drapeau de boîte aux lettres identique sur les {n} premières", irq_flags.0.len(), irq_flags.1.len());
        for i in n.saturating_sub(3)..(n + 4).min(irq_flags.0.len().min(irq_flags.1.len())) {
            println!("  interruption {i} : Z80 (image {}, drapeau {:02x}) / Rust (image {}, drapeau {:02x})", irq_flags.0[i].0, irq_flags.0[i].1, irq_flags.1[i].0, irq_flags.1[i].1);
        }
    }
    if common < lz.len().min(lr.len()) {
        let frame = log_ends.iter().position(|&(a, b)| a > common || b > common).unwrap_or(0);
        println!("première écriture QSound différente : n°{common}, image {frame}");
        for i in common.saturating_sub(3)..(common + 6).min(lz.len().min(lr.len())) {
            println!("  {i} : Z80 {:02x?} / Rust {:02x?}", lz[i], lr[i]);
        }
    }
    println!("écritures QSound : Z80 {} / pilote Rust {} ; identiques dans l'ordre : {common}", lz.len(), lr.len());
    match first_ram {
        None => println!("RAM du 68000 identique à chaque image"),
        Some(f) => println!("RAM du 68000 différente dès l'image {f}"),
    }
    match first_shared {
        None => println!("RAM partagée identique à chaque image"),
        Some(f) => println!("RAM partagée différente à {shared_diffs} images, la première à l'image {f}"),
    }
}
