//! Validation of the native routines over a scenario: two machines in parallel.
//!   A: native routines in check mode (every call compared to the original, original timing)
//!   B: original code only (recompiled)
//! Any RAM difference (outside the task stacks) between A and B is a rewrite error.
//!
//! Variant of `validate` that also prints the coverage of the internal routines of `combat`.
//! combat_coverage <roms> <eeprom> <scenario> <frames>

use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, frames] = &a[..] else {
        eprintln!("usage: validate <roms> <eeprom> <scenario> <frames>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let load = || cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    let (mut ma, mut mb) = (load(), load());
    ddsom_game::install(&mut ma);
    ddsom_game::set_check(true);
    ddsom_game::combat::enable_coverage();
    ddsom_game::install_reference(&mut mb);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scenario");
    let frames: usize = frames.parse().unwrap();
    // Register save areas: task stacks (FF0A00-FF0FFF) and the VBLANK interrupt's software
    // stack (below FF2A1A). A register left free by a native routine is only a copy
    // there; if it were really used, the difference would show up elsewhere.
    // + the kernel RAM, which the game keeps in its task table in Rust.
    let saved = |a: usize| ddsom_game::tasks::table::kernel_ram(a as u32) || (0x2900..0x2a1a).contains(&a);
    let t0 = std::time::Instant::now();
    let mut first = None;
    for f in 0..frames {
        script.apply(f, &mut ma);
        script.apply(f, &mut mb);
        ma.run_frame();
        mb.run_frame();
        let diff: Vec<usize> = (0..0x8000)
            .filter(|&i| ma.bus.ram[i] != mb.bus.ram[i] && !saved(2 * i))
            .collect();
        if !diff.is_empty() && first.is_none() {
            first = Some(f + 1);
            println!("frame {}: {} different words", f + 1, diff.len());
            for &i in diff.iter().take(10) {
                println!("  {:06x}: natif {:04x}  origine {:04x}", 0xff0000 + 2 * i, ma.bus.ram[i], mb.bus.ram[i]);
            }
        }
    }
    println!("{frames} frames in {:.1?}", t0.elapsed());
    for (name, st) in ddsom_game::report() {
        println!("  {name:28} {:>7} calls {:>5} mismatches{}", st.calls, st.mismatches,
            st.first.map(|f| format!(" — {f}")).unwrap_or_default());
    }
    println!("--- coverage of the internal routines of combat ---");
    for (name, n) in ddsom_game::combat::coverage() {
        println!("  {name:34} {n:>9}");
    }
    let errors: u64 = ddsom_game::report().values().map(|s| s.mismatches).sum();
    match (first, errors) {
        (None, 0) => println!("VALID: identical RAM over {frames} frames, no mismatch"),
        (None, n) => println!("FAILED: {n} native calls with mismatches (state restored from the original each time)"),
        (Some(f), _) => println!("FAILED: first RAM difference at frame {f}"),
    }
}
