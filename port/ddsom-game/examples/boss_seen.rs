//! Quels boss (objets BOSS, FFBA1E) un scénario rencontre-t-il, et dans quelle scène ?
//!   boss_seen <roms> <eeprom> <scénario> <images>
//! Rejoue le scénario sur la machine d'origine et note la première image de chaque (scène, type, variante).
use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(&a[1]), ee.as_deref()).expect("roms");
    m.set_overclock(16.0);
    ddsom_game::install_reference(&mut m);
    m.idle = Some(ddsom_game::idle);
    let script = ddsom_game::scenario::Scenario::load(Path::new(&a[3])).unwrap();
    let frames: usize = a[4].parse().unwrap();
    let rd8 = |m: &cps2::Machine, ad: u32| {
        let o = (ad - 0xff0000) as usize;
        let w = m.bus.ram[o / 2];
        if o % 2 == 0 { (w >> 8) as u8 } else { w as u8 }
    };
    let mut seen = std::collections::BTreeSet::new();
    for f in 0..frames {
        script.apply(f, &mut m);
        m.run_frame();
        for k in 0..3 {
            let o = 0xffba1e + 0xc0 * k;
            if rd8(&m, o) != 0 {
                let key = (rd8(&m, 0xff8034), rd8(&m, o + 2), rd8(&m, o + 3));
                if seen.insert(key) {
                    println!("image {f} : scène {} boss type {} variante {}", key.0, key.1, key.2);
                }
            }
        }
    }
}
