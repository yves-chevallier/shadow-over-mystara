//! Qui écrit une adresse ? watch <roms> <eeprom> <scenario> <image> <adresse_hex> [natives=1]
//! Exécute jusqu'à l'image donnée, puis la rejoue pas à pas en signalant chaque changement.
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(std::path::Path::new(&a[1]), ee.as_deref()).unwrap();
    let natives = a.get(6).map(|s| s == "1").unwrap_or(true);
    if natives { ddsom_game::install(&mut m) } else { ddsom_game::install_reference(&mut m) }
    m.set_overclock(16.0);
    m.idle = Some(ddsom_game::idle);
    let s = ddsom_game::scenario::Scenario::load(std::path::Path::new(&a[3])).unwrap();
    let n: usize = a[4].parse().unwrap();
    let addr = u32::from_str_radix(&a[5], 16).unwrap();
    let gfx = (0x900000..0xb00000).contains(&addr);
    let i = if gfx { ((addr - 0x900000) / 2) as usize } else { ((addr - 0xff0000) / 2) as usize };
    for f in 0..n - 1 {
        s.apply(f, &mut m);
        m.run_frame();
    }
    s.apply(n - 1, &mut m);
    // dernière image : les natives restent actives, mais le reste est interprété pas à pas
    let exec = m.executor;
    m.executor = Some(|pc, c, b, dl| {
        let k = WATCH.with(|w| w.get());
        let get = |b: &cps2::Cps2Bus| if GFX.with(|g| g.get()) { b.gfxram[k] } else { b.ram[k] };
        let before = get(b);
        let r = ddsom_game::run_native_only(pc, c, b, dl);
        if get(b) != before {
            println!("écrit par une routine native en {pc:06x} : {:04x}", get(b));
        }
        r
    });
    WATCH.with(|w| w.set(i));
    GFX.with(|g| g.set(gfx));
    let _ = exec;
    let mut last = if gfx { m.bus.gfxram[i] } else { m.bus.ram[i] };
    m.step_hook = Some(Box::new(move |c: &m68k::Cpu, b: &cps2::Cps2Bus, pc: u32| {
        let now = if gfx { b.gfxram[i] } else { b.ram[i] };
        if now != last {
            println!("pc {pc:06x} -> {now:04x}  d0={:08x} a0={:08x} a1={:08x} a4={:08x} a6={:08x} a7={:08x}", c.d[0], c.a[0], c.a[1], c.a[4], c.a[6], c.a[7]);
            last = now;
        }
    }));
    m.run_frame();
}

thread_local! {
    static WATCH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static GFX: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
