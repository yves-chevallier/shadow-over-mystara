//! Trace des états des boss d'un scénario : à chaque changement de (type, +4..+7, PV) d'un objet BOSS,
//! une ligne `image emplacement type état PV`. Montre quelles phases (dont la mort) un scénario exerce.
//!   boss_trace <roms> <eeprom> <scénario> <images>
use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(&a[1]), ee.as_deref()).expect("roms");
    m.set_overclock(16.0);
    ddsom_rc::install(&mut m);
    m.idle = Some(ddsom_game::idle);
    let script = ddsom_game::scenario::Scenario::load(Path::new(&a[3])).unwrap();
    let frames: usize = a[4].parse().unwrap();
    let rd8 = |m: &cps2::Machine, ad: u32| {
        let o = (ad - 0xff0000) as usize;
        let w = m.bus.ram[o / 2];
        if o % 2 == 0 { (w >> 8) as u8 } else { w as u8 }
    };
    let mut last = [(0u8, [0u8; 4], 0u16); 3];
    for f in 0..frames {
        script.apply(f, &mut m);
        m.run_frame();
        for k in 0..3usize {
            let o = 0xffba1e + 0xc0 * k as u32;
            let now = if rd8(&m, o) != 0 {
                let hp = (rd8(&m, o + 0x62) as u16) << 8 | rd8(&m, o + 0x63) as u16;
                (rd8(&m, o + 2) + 1, [rd8(&m, o + 4), rd8(&m, o + 5), rd8(&m, o + 6), rd8(&m, o + 3)], hp)
            } else {
                (0, [0; 4], 0)
            };
            if now != last[k] {
                println!("image {f} : emplacement {k} type {} états {:02x?} (+3 variante) PV {}", now.0.wrapping_sub(1), now.1, now.2);
                last[k] = now;
            }
        }
    }
}
