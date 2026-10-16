//! Comparaison avec MAME en exécutant les routines natives de ddsom-game (+ code recompilé).
//! CHECK=1 : mode contrôle, chaque appel natif est comparé à la routine d'origine.
//! Compare la RAM de travail (FF0000-FFFFFF) image par image avec un dump MAME
//! (mame/scenario.lua, RAMDUMP="1 N fichier" : 64 Ko par image à partir de l'image 1).
//! cargo run --release -p cps2 --example compare -- <roms> <eeprom> <ram_mame.bin> [images]

use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(&a[1]), ee.as_deref()).expect("roms");
    ddsom_game::install(&mut m);
    ddsom_game::set_check(std::env::var("CHECK").as_deref() == Ok("1"));
    let t0 = std::time::Instant::now();
    let dump = std::fs::read(&a[3]).expect("dump MAME");
    // IGNORE="debut-fin,..." (adresses hex) : zones exclues (ex. piles des tâches)
    let ignore: Vec<(usize, usize)> = std::env::var("IGNORE").unwrap_or_default().split(',')
        .filter(|s| !s.is_empty())
        .map(|r| {
            let (a, b) = r.split_once('-').unwrap();
            (usize::from_str_radix(a, 16).unwrap() - 0xff0000, usize::from_str_radix(b, 16).unwrap() - 0xff0000)
        })
        .collect();
    let n = (dump.len() / 0x10000).min(a.get(4).map(|s| s.parse().unwrap()).unwrap_or(usize::MAX));
    // SCENARIO=fichier (format mame/scenario.lua) : mêmes entrées, appliquées après l'image indiquée
    let mut actions: Vec<(usize, String, u32)> = Vec::new();
    if let Ok(path) = std::env::var("SCENARIO") {
        for line in std::fs::read_to_string(path).unwrap().lines() {
            let mut it = line.trim().splitn(2, ' ');
            let (Some(f), Some(rest)) = (it.next(), it.next()) else { continue };
            let Ok(f) = f.parse::<usize>() else { continue };
            if let Some((name, v)) = rest.rsplit_once(' ') {
                if let Ok(v) = v.parse() {
                    actions.push((f, name.to_string(), v));
                }
            }
        }
    }
    // SNAP="f1,f2" SNAPDIR=dossier : écrit le rendu de ces images en PNG
    let snaps: Vec<usize> = std::env::var("SNAP").unwrap_or_default().split(',').filter_map(|v| v.parse().ok()).collect();
    let snapdir = std::env::var("SNAPDIR").unwrap_or_else(|_| ".".into());
    let mut first_bad = None;
    // divergences durables : octet différent pendant >= 30 images consécutives
    let mut run = vec![0u32; 0x10000];
    let mut reported = vec![false; 0x10000];
    let mut persistent = 0;
    for f in 0..n {
        for (af, name, v) in &actions {
            // MAME ne voit une valeur posée par set_value qu'à la mise à jour des ports de l'image suivante
            if *af + 1 == f {
                let (port, bit) = input_bit(name);
                let reg = match port { 0 => &mut m.bus.inputs.in0, 1 => &mut m.bus.inputs.in1, _ => &mut m.bus.inputs.in2 };
                if *v != 0 { *reg &= !bit } else { *reg |= bit }
            }
        }
        let img = m.run_frame();
        if snaps.contains(&(f + 1)) {
            let file = std::fs::File::create(format!("{}/{}.png", snapdir, f + 1)).unwrap();
            let mut enc = png::Encoder::new(file, 384, 224);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header().unwrap().write_image_data(&img.concat()).unwrap();
        }
        let mame = &dump[f * 0x10000..(f + 1) * 0x10000];
        let ours: Vec<u8> = m.bus.ram.iter().flat_map(|w| w.to_be_bytes()).collect();
        let ignored = |i: usize| ignore.iter().any(|&(a, b)| i >= a && i <= b);
        let diffs: Vec<usize> = (0..0x10000).filter(|&i| mame[i] != ours[i] && !ignored(i)).collect();
        for i in 0..0x10000 {
            if mame[i] != ours[i] && !ignored(i) {
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
    eprintln!("{n} images en {:.2?} ; {} instructions 68000 interprétées", t0.elapsed(), m.interp_steps);
    let mut misses: Vec<(u32, u64)> = m.misses.iter().map(|(&a, &n)| (a, n)).collect();
    misses.sort_by_key(|x| std::cmp::Reverse(x.1));
    eprintln!("{} adresses hors blocs ; les plus fréquentes :", misses.len());
    for (a, n) in misses.iter().take(10) {
        eprintln!("  {a:06x} x{n}");
    }
    if let Ok(path) = std::env::var("MISSES_OUT") {
        let txt: String = m.misses.keys().map(|a| format!("{a:06x}\n")).collect();
        std::fs::write(path, txt).unwrap();
    }
    for (name, st) in ddsom_game::report() {
        eprintln!("contrôle {name:24} {} appels, {} écarts{}", st.calls, st.mismatches,
            st.first.map(|f| format!(" — ex. {f}")).unwrap_or_default());
    }
    match first_bad {
        None => println!("{n} images identiques à MAME"),
        Some(f) => println!("première divergence à l'image {f}"),
    }
}

/// Nom d'entrée MAME -> (port, bit) (cps2_4p4b, actif à 0).
fn input_bit(name: &str) -> (u8, u16) {
    let player = |p: &str| -> u16 { if p == "1" { 0 } else { 8 } };
    if let Some(rest) = name.strip_prefix('P') {
        let (p, what) = rest.split_once(' ').unwrap();
        let shift = player(p);
        let b = match what {
            "Right" => 0x01, "Left" => 0x02, "Down" => 0x04, "Up" => 0x08,
            "Button 1" => 0x10, "Button 2" => 0x20, "Button 3" => 0x40, "Button 4" => 0x80,
            _ => panic!("entrée inconnue {name}"),
        };
        return (0, b << shift);
    }
    match name {
        "Coin 1" => (2, 0x1000),
        "Coin 2" => (2, 0x2000),
        "1 Player Start" => (2, 0x0100),
        "2 Players Start" => (2, 0x0200),
        "Service 1" => (2, 0x0004),
        _ => panic!("entrée inconnue {name}"),
    }
}
