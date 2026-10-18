//! Qui écrit une adresse ? watch <roms> <eeprom> <scenario> <image> <adresse_hex> [natives=1]
//! Exécute jusqu'à l'image donnée, puis la rejoue pas à pas en signalant chaque changement.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(std::path::Path::new(&a[1]), ee.as_deref()).unwrap();
    let natives = a.get(6).map(|s| s == "1").unwrap_or(true);
    if natives { ddsom_game::install(&mut m) } else { ddsom_rc::install(&mut m) }
    m.set_overclock(16.0);
    m.idle = Some(ddsom_game::idle);
    let s = ddsom_game::scenario::Scenario::load(std::path::Path::new(&a[3])).unwrap();
    let n: usize = a[4].parse().unwrap();
    let addr = u32::from_str_radix(&a[5], 16).unwrap();
    let i = ((addr - 0xff0000) / 2) as usize;
    for f in 0..n - 1 {
        s.apply(f, &mut m);
        m.run_frame();
    }
    s.apply(n - 1, &mut m);
    // dernière image : les natives restent actives, mais le reste est interprété pas à pas
    let exec = m.executor;
    m.executor = Some(|pc, c, b, dl| {
        let before = b.ram[WATCH.with(|w| w.get())];
        let r = ddsom_game::run_native_only(pc, c, b, dl);
        if b.ram[WATCH.with(|w| w.get())] != before {
            println!("écrit par une routine native en {pc:06x} : {:04x}", b.ram[WATCH.with(|w| w.get())]);
        }
        r
    });
    WATCH.with(|w| w.set(i));
    let _ = exec;
    let mut last = m.bus.ram[i];
    m.step_hook = Some(Box::new(move |c: &m68k::Cpu, b: &cps2::Cps2Bus, pc: u32| {
        if b.ram[i] != last {
            println!("pc {pc:06x} -> {:04x}  d0={:08x} a0={:08x} a1={:08x} a4={:08x} a6={:08x}", b.ram[i], c.d[0], c.a[0], c.a[1], c.a[4], c.a[6]);
            last = b.ram[i];
        }
    }));
    m.run_frame();
}

thread_local! {
    static WATCH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
