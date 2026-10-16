//! Compare la RAM de travail (FF0000-FFFFFF) image par image avec un dump MAME
//! (mame/scenario.lua, RAMDUMP="1 N fichier" : 64 Ko par image à partir de l'image 1).
//! cargo run --release -p cps2 --example compare -- <roms> <eeprom> <ram_mame.bin> [images]

use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(&a[1]), ee.as_deref()).expect("roms");
    let dump = std::fs::read(&a[3]).expect("dump MAME");
    let n = (dump.len() / 0x10000).min(a.get(4).map(|s| s.parse().unwrap()).unwrap_or(usize::MAX));
    let mut first_bad = None;
    // divergences durables : octet différent pendant >= 30 images consécutives
    let mut run = vec![0u32; 0x10000];
    let mut reported = vec![false; 0x10000];
    let mut persistent = 0;
    for f in 0..n {
        m.run_frame();
        let mame = &dump[f * 0x10000..(f + 1) * 0x10000];
        let ours: Vec<u8> = m.bus.ram.iter().flat_map(|w| w.to_be_bytes()).collect();
        let diffs: Vec<usize> = (0..0x10000).filter(|&i| mame[i] != ours[i]).collect();
        for i in 0..0x10000 {
            if mame[i] != ours[i] {
                run[i] += 1;
                if run[i] == 30 && !reported[i] && persistent < 12 {
                    reported[i] = true;
                    persistent += 1;
                    println!("durable : {:06x} diffère depuis l'image {} (MAME {:02x} Rust {:02x})", 0xff0000 + i, f + 2 - 30, mame[i], ours[i]);
                }
            } else {
                run[i] = 0;
            }
        }
        if std::env::var("DETAIL").ok().and_then(|v| v.parse::<usize>().ok()) == Some(f + 1) {
            println!("détail image {} :", f + 1);
            for &i in &diffs {
                println!("  {:06x}: MAME {:02x}  Rust {:02x}", 0xff0000 + i, mame[i], ours[i]);
            }
        }
        if !diffs.is_empty() {
            if first_bad.is_none() {
                first_bad = Some(f + 1);
                println!("image {} : {} octets différents (pc={:06x})", f + 1, diffs.len(), m.cpu.pc);
                for &i in diffs.iter().take(24) {
                    println!("  {:06x}: MAME {:02x}  Rust {:02x}", 0xff0000 + i, mame[i], ours[i]);
                }
            } else if (f + 1) % 60 == 0 {
                println!("image {} : {} octets différents", f + 1, diffs.len());
            }
        }
    }
    match first_bad {
        None => println!("{n} images identiques à MAME"),
        Some(f) => println!("première divergence à l'image {f}"),
    }
}
