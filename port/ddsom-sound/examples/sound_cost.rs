//! Measurement: Z80 time of the main loop work items (command, ADPCM tick, sequencer tick) on the reference machine
//! of `validate`, against the number of QSound writes they make.
//!
//! sound_cost <roms> <eeprom> <scenario> <frames>

use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::rc::Rc;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, frames] = &a[..] else {
        eprintln!("usage: sound_cost <roms> <eeprom> <scenario> <frames>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    ddsom_game::install_reference(&mut m);
    m.set_overclock(16.0);
    m.idle = Some(ddsom_game::idle);
    m.rendering = false;
    // instructions `ld (D002),a`: the QSound register writes
    let fixed = &m.sound.rom[..0x8000];
    let writes: HashSet<u16> = (0..fixed.len() - 3).filter(|&i| fixed[i..i + 3] == [0x32, 0x02, 0xd0]).map(|i| i as u16).collect();
    let marks = Rc::new(RefCell::new(Vec::new()));
    let hook = marks.clone();
    m.sound.pc_hook = Some(Box::new(move |z| {
        if matches!(z.pc, 0x185 | 0x188 | 0x190 | 0x193 | 0x19a | 0x17d) || writes.contains(&z.pc) {
            hook.borrow_mut().push((z.pc, z.cycles));
        }
    }));
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scenario");
    for f in 0..frames.parse().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    let marks = marks.borrow();
    // gaps between consecutive QSound writes inside an item, in cycles (bucketed by 128)
    let mut gaps: BTreeMap<u64, usize> = BTreeMap::new();
    // item: (kind, writes, cycles)
    let mut items: BTreeMap<(&str, usize), (usize, u64, u64, u64)> = BTreeMap::new();
    let mut cur: Option<(&str, u64, usize, Option<u64>)> = None;
    for &(pc, cy) in marks.iter() {
        match pc {
            0x185 => cur = Some(("command", cy, 0, None)),
            0x190 => cur = Some(("adpcm", cy, 0, None)),
            0x19a => cur = Some(("tick", cy, 0, None)),
            0x188 | 0x193 | 0x17d => {
                if let Some((k, t0, n, _)) = cur.take() {
                    let d = cy - t0;
                    let e = items.entry((k, n)).or_insert((0, u64::MAX, 0, 0));
                    e.0 += 1;
                    e.1 = e.1.min(d);
                    e.2 = e.2.max(d);
                    e.3 += d;
                }
            }
            _ => {
                if let Some((_, _, n, last)) = cur.as_mut() {
                    if let Some(l) = *last {
                        *gaps.entry((cy - l) / 128 * 128).or_insert(0) += 1;
                    }
                    *last = Some(cy);
                    *n += 1;
                }
            }
        }
    }
    println!("gaps between writes (cycles, by 128): {gaps:?}");
    for ((k, n), (count, min, max, sum)) in &items {
        println!("{k:8} {n:4} writes: {count:7} items, cycles min {min} max {max} mean {}", sum / *count as u64);
    }
}
