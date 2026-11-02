//! Comparison with MAME by running the native routines of ddsom-game (+ recompiled code).
//! CHECK=1: check mode, each native call is compared to the original routine.
//! Compares the work RAM (FF0000-FFFFFF) frame by frame with a MAME dump
//! (mame/scenario.lua, RAMDUMP="1 N file": 64 KB per frame starting at frame 1).
//! cargo run --release -p cps2 --example compare -- <roms> <eeprom> <ram_mame.bin> [frames]

use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(&a[1]), ee.as_deref()).expect("roms");
    // NATIVES=0: recompiled code only (reference to isolate the effect of the native routines)
    if std::env::var("NATIVES").as_deref() == Ok("0") {
        ddsom_game::install_reference(&mut m);
    } else {
        ddsom_game::install(&mut m);
    }
    ddsom_game::set_check(std::env::var("CHECK").as_deref() == Ok("1"));
    let t0 = std::time::Instant::now();
    let dump = std::fs::read(&a[3]).expect("dump MAME");
    // IGNORE="start-end,..." (hex addresses): excluded zones (e.g. task stacks)
    let ignore: Vec<(usize, usize)> = std::env::var("IGNORE").unwrap_or_default().split(',')
        .filter(|s| !s.is_empty())
        .map(|r| {
            let (a, b) = r.split_once('-').unwrap();
            (usize::from_str_radix(a, 16).unwrap() - 0xff0000, usize::from_str_radix(b, 16).unwrap() - 0xff0000)
        })
        .collect();
    // STRIDE=k: the MAME dump contains only one frame out of k (RAMDUMP "1 N file k")
    let stride: usize = std::env::var("STRIDE").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    let n = (dump.len() / 0x10000 * stride + std::env::var("DUMP_FIRST").ok().and_then(|v| v.parse::<usize>().ok()).unwrap_or(1) - 1).min(a.get(4).map(|s| s.parse().unwrap()).unwrap_or(usize::MAX));
    // SCENARIO=file (mame/scenario.lua format): same inputs, applied after the given frame
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
    // SNAP="f1,f2" SNAPDIR=folder: writes the rendering of these frames as PNG
    let snaps: Vec<usize> = std::env::var("SNAP").unwrap_or_default().split(',').filter_map(|v| v.parse().ok()).collect();
    let snapdir = std::env::var("SNAPDIR").unwrap_or_else(|_| ".".into());
    let mut first_bad = None;
    // lasting divergences: byte different for >= 30 consecutive frames
    let mut run = vec![0u32; 0x10000];
    let mut reported = vec![false; 0x10000];
    let mut persistent = 0;
    for f in 0..n {
        for (af, name, v) in &actions {
            // MAME only sees a value set by set_value at the next frame's port update
            if *af + 1 == f {
                let (port, bit) = input_bit(name);
                let reg = match port { 0 => &mut m.bus.inputs.in0, 1 => &mut m.bus.inputs.in1, _ => &mut m.bus.inputs.in2 };
                if *v != 0 { *reg &= !bit } else { *reg |= bit }
            }
        }
        let img = m.run_frame();
        // PCLOG="a-b": PC and SR at the end of the frame over this interval
        if let Some((a, b)) = std::env::var("PCLOG").ok().and_then(|r| r.split_once('-').map(|(a, b)| (a.parse::<usize>().unwrap(), b.parse::<usize>().unwrap()))) {
            if (a..=b).contains(&(f + 1)) {
                println!("P {} pc={:06x} sr={:04x}", f + 1, m.cpu.pc, m.cpu.sr);
            }
        }
        if snaps.contains(&(f + 1)) {
            let file = std::fs::File::create(format!("{}/{}.png", snapdir, f + 1)).unwrap();
            let mut enc = png::Encoder::new(file, 384, 224);
            enc.set_color(png::ColorType::Rgb);
            enc.set_depth(png::BitDepth::Eight);
            enc.write_header().unwrap().write_image_data(&img.concat()).unwrap();
        }
        // DUMP_FIRST=n: first frame contained in the dump (default 1)
        let first: usize = std::env::var("DUMP_FIRST").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
        if f + 1 < first || (f + 1 - first) % stride != 0 {
            continue;
        }
        let k = (f + 1 - first) / stride;
        if (k + 1) * 0x10000 > dump.len() {
            break;
        }
        let mame = &dump[k * 0x10000..(k + 1) * 0x10000];
        let ours: Vec<u8> = m.bus.ram.iter().flat_map(|w| w.to_be_bytes()).collect();
        let ignored = |i: usize| ignore.iter().any(|&(a, b)| i >= a && i <= b);
        let diffs: Vec<usize> = (0..0x10000).filter(|&i| mame[i] != ours[i] && !ignored(i)).collect();
        for i in 0..0x10000 {
            if mame[i] != ours[i] && !ignored(i) {
                run[i] += 1;
                if run[i] == (30 / stride).max(3) as u32 && !reported[i] && persistent < 12 {
                    reported[i] = true;
                    persistent += 1;
                    println!("lasting: {:06x} differs since frame {} (MAME {:02x} Rust {:02x})", 0xff0000 + i, f + 2 - 30, mame[i], ours[i]);
                }
            } else {
                run[i] = 0;
            }
        }
        if std::env::var("DETAIL").ok().and_then(|v| v.parse::<usize>().ok()) == Some(f + 1) {
            println!("detail frame {}:", f + 1);
            for &i in &diffs {
                println!("  {:06x}: MAME {:02x}  Rust {:02x}", 0xff0000 + i, mame[i], ours[i]);
            }
        }
        if std::env::var_os("VERBOSE").is_some() && !diffs.is_empty() {
            let list: Vec<String> = diffs.iter().take(8).map(|&i| format!("{:06x}:{:02x}/{:02x}", 0xff0000 + i, mame[i], ours[i])).collect();
            println!("  [{}] pc={:06x} {} mismatches: {}", f + 1, m.cpu.pc, diffs.len(), list.join(" "));
        }
        if !diffs.is_empty() {
            if first_bad.is_none() {
                first_bad = Some(f + 1);
                println!("frame {}: {} different bytes (pc={:06x})", f + 1, diffs.len(), m.cpu.pc);
                for &i in diffs.iter().take(24) {
                    println!("  {:06x}: MAME {:02x}  Rust {:02x}", 0xff0000 + i, mame[i], ours[i]);
                }
            } else if (f + 1) % 60 == 0 {
                println!("frame {}: {} different bytes", f + 1, diffs.len());
            }
        }
    }
    eprintln!("{n} frames in {:.2?}; {} 68000 instructions interpreted", t0.elapsed(), m.interp_steps);
    let mut misses: Vec<(u32, u64)> = m.misses.iter().map(|(&a, &n)| (a, n)).collect();
    misses.sort_by_key(|x| std::cmp::Reverse(x.1));
    eprintln!("{} addresses outside blocks; the most frequent:", misses.len());
    for (a, n) in misses.iter().take(10) {
        eprintln!("  {a:06x} x{n}");
    }
    if let Ok(path) = std::env::var("MISSES_OUT") {
        let txt: String = m.misses.keys().map(|a| format!("{a:06x}\n")).collect();
        std::fs::write(path, txt).unwrap();
    }
    for (name, st) in ddsom_game::report() {
        eprintln!("check {name:24} {} calls, {} mismatches{}", st.calls, st.mismatches,
            st.first.map(|f| format!(" — ex. {f}")).unwrap_or_default());
    }
    match first_bad {
        None => println!("{n} frames identical to MAME"),
        Some(f) => println!("first divergence at frame {f}"),
    }
}

/// MAME input name -> (port, bit) (cps2_4p4b, active at 0).
fn input_bit(name: &str) -> (u8, u16) {
    let player = |p: &str| -> u16 { if p == "1" { 0 } else { 8 } };
    if let Some(rest) = name.strip_prefix('P') {
        let (p, what) = rest.split_once(' ').unwrap();
        let shift = player(p);
        let b = match what {
            "Right" => 0x01, "Left" => 0x02, "Down" => 0x04, "Up" => 0x08,
            "Button 1" => 0x10, "Button 2" => 0x20, "Button 3" => 0x40, "Button 4" => 0x80,
            _ => panic!("unknown input {name}"),
        };
        return (0, b << shift);
    }
    match name {
        "Coin 1" => (2, 0x1000),
        "Coin 2" => (2, 0x2000),
        "1 Player Start" => (2, 0x0100),
        "2 Players Start" => (2, 0x0200),
        "Service 1" => (2, 0x0004),
        _ => panic!("unknown input {name}"),
    }
}
