//! Affiche des octets de RAM à la fin de chaque image d'une plage, avec ou sans natives.
//!   dumpram <roms> <eeprom> <scénario> <image début> <image fin> <adresse hex> <longueur> <natives 0|1>

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(std::path::Path::new(&a[1]), ee.as_deref()).unwrap();
    m.set_overclock(16.0);
    m.idle = Some(ddsom_game::idle);
    if a[8] == "1" {
        ddsom_game::install(&mut m)
    } else {
        ddsom_game::install_reference(&mut m)
    }
    let s = ddsom_game::scenario::Scenario::load(std::path::Path::new(&a[3])).unwrap();
    let (from, to): (usize, usize) = (a[4].parse().unwrap(), a[5].parse().unwrap());
    let addr = u32::from_str_radix(&a[6], 16).unwrap();
    let len: usize = a[7].parse().unwrap();
    for f in 0..to {
        s.apply(f, &mut m);
        m.run_frame();
        if f + 1 >= from {
            let base = ((addr - 0xff0000) / 2) as usize;
            let words: Vec<String> = (0..len / 2).map(|i| format!("{:04x}", m.bus.ram[base + i])).collect();
            println!("{:5} {}", f + 1, words.join(" "));
        }
    }
}
