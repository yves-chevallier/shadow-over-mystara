//! Trace of the PCs executed during a frame (native routines active, the rest interpreted), to
//! compare two runs (for example with and without a native routine, via DDSOM_SANS).
//!
//! pctrace <roms> <eeprom> <scenario> <frame> [excluded_start excluded_end (hex)]
//! `PCTRACE_SEUL=start-end` limits the trace to a PC range, `PCTRACE_IMAGES=k` traces k consecutive frames.

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(std::path::Path::new(&a[1]), ee.as_deref()).unwrap();
    ddsom_game::install(&mut m);
    m.set_overclock(16.0);
    let s = ddsom_game::scenario::Scenario::load(std::path::Path::new(&a[3])).unwrap();
    let n: usize = a[4].parse().unwrap();
    let hex = |i: usize| a.get(i).map(|v| u32::from_str_radix(v, 16).unwrap());
    let skip = hex(5).unwrap_or(0)..hex(6).unwrap_or(0);
    for f in 0..n - 1 {
        s.apply(f, &mut m);
        m.run_frame();
    }
    s.apply(n - 1, &mut m);
    m.executor = Some(ddsom_game::run_native_only);
    // PCTRACE_SEUL=start-end (hex): shows only the PCs of this range; PCTRACE_IMAGES=k: traces k consecutive frames
    let only = std::env::var("PCTRACE_SEUL").ok().and_then(|v| {
        let (a, b) = v.split_once('-')?;
        Some(u32::from_str_radix(a, 16).ok()?..u32::from_str_radix(b, 16).ok()?)
    });
    m.step_hook = Some(Box::new(move |c: &m68k::Cpu, _: &cps2::Cps2Bus, pc: u32| {
        if !skip.contains(&pc) && only.as_ref().is_none_or(|r| r.contains(&pc)) {
            let d: Vec<String> = c.d.iter().chain(&c.a[..7]).map(|r| format!("{r:08x}")).collect();
            println!("{pc:06x} {} sr={:02x}", d.join(" "), c.sr & 0x1f);
        }
    }));
    let k: usize = std::env::var("PCTRACE_IMAGES").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    for i in 0..k {
        if i > 0 {
            println!("--- frame {} (counter FF806E: {})", n + i, m.bus.ram[(0xff806e - 0xff0000) / 2]);
            s.apply(n - 1 + i, &mut m);
        }
        m.run_frame();
    }
}
