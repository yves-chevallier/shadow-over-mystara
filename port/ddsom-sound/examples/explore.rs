//! Exploration du pilote Z80 : couverture des instructions exécutées et journal d'événements.
//!
//! explore <roms> <eeprom> <scenario> <images> <sortie>
//! Produit <sortie>.cov (octets 0/1 par adresse Z80 0-0x7FFF, un octet = instruction exécutée)
//! et <sortie>.log (texte : événements du Z80 regroupés par interruption).

use cps2::sound::Event;
use std::cell::RefCell;
use std::fmt::Write as _;
use std::path::Path;
use std::rc::Rc;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, frames, out] = &a[..] else {
        eprintln!("usage : explore <roms> <eeprom> <scenario> <images> <sortie>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    ddsom_rc::install(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    let frames: usize = frames.parse().unwrap();

    let cov = Rc::new(RefCell::new(vec![0u8; 0x10000]));
    let c2 = cov.clone();
    m.sound.pc_hook = Some(Box::new(move |z| c2.borrow_mut()[z.pc as usize] = 1));
    m.sound.trace = Some(Vec::new());

    let mut log = String::new();
    let mut irq = 0u64;
    let mut latch = [0u8; 2];
    for f in 0..frames {
        script.apply(f, &mut m);
        m.run_frame();
        let ev = std::mem::take(m.sound.trace.as_mut().unwrap());
        let mut last_read: Option<(u16, u8)> = None;
        let mut nread = 0;
        for e in ev {
            if !matches!(e, Event::Read(..)) {
                if let Some((a, v)) = last_read.take() {
                    let _ = writeln!(log, "  R {a:03x}={v:02x} x{nread}");
                }
            }
            match e {
                Event::Irq => {
                    irq += 1;
                    let _ = writeln!(log, "IRQ {irq} image {f}");
                }
                Event::Read(a, v) => {
                    if last_read == Some((a, v)) {
                        nread += 1;
                    } else {
                        if let Some((a, v)) = last_read {
                            let _ = writeln!(log, "  R {a:03x}={v:02x} x{nread}");
                        }
                        last_read = Some((a, v));
                        nread = 1;
                    }
                }
                Event::Write(a, v) => {
                    let _ = writeln!(log, "  W {a:03x}={v:02x}");
                }
                Event::Port(2, r) => {
                    let _ = writeln!(log, "  Q {r:02x} <- {:02x}{:02x}", latch[0], latch[1]);
                }
                Event::Port(3, b) => {
                    let _ = writeln!(log, "  B {b:02x}");
                }
                Event::Port(o, v) => latch[o as usize] = v,
            }
        }
        if let Some((a, v)) = last_read {
            let _ = writeln!(log, "  R {a:03x}={v:02x} x{nread}");
        }
    }
    std::fs::write(format!("{out}.log"), log).unwrap();
    std::fs::write(format!("{out}.cov"), &cov.borrow()[..]).unwrap();
}
