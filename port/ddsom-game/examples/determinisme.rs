//! Le jeu surcadencé est-il déterminé image par image ? Deux machines d'origine, à deux fréquences
//! différentes, rejouent le même scénario ; on compare la RAM de travail à chaque image.
//!
//! determinisme <roms> <eeprom> <scenario> <images> <k1> <k2>

use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, frames, k1, k2] = &a[..] else {
        eprintln!("usage : determinisme <roms> <eeprom> <scenario> <images> <k1> <k2>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let load = |k: &str| {
        let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
        ddsom_game::install_reference(&mut m);
        m.set_overclock(k.parse().unwrap());
        if std::env::var("IDLE").is_err() {
            m.idle = None;
        }
        m
    };
    // même démarrage (sa durée dépend de la fréquence), puis fréquences différentes
    let (mut ma, mut mb) = (load(k1), load(k1));
    let k2: f64 = k2.parse().unwrap();
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    let frames: usize = frames.parse().unwrap();
    let mut shown = 0;
    let mut freq = std::collections::BTreeMap::<usize, usize>::new();
    for f in 0..frames {
        if f == 2000 {
            mb.set_overclock(k2);
        }
        script.apply(f, &mut ma);
        script.apply(f, &mut mb);
        ma.run_frame();
        mb.run_frame();
        let diff: Vec<usize> = (0..0x8000).filter(|&i| ma.bus.ram[i] != mb.bus.ram[i] && !(0x500..0x800).contains(&i)).collect();
        for &i in &diff {
            *freq.entry(i).or_default() += 1;
        }
        if (f + 1) % 2000 == 0 {
            let w = |m: &cps2::Machine, a: u32| m.bus.ram[((a - 0xff0000) / 2) as usize];
            println!("image {} : {} mots différents ; 806c {:04x}/{:04x} 806e {:04x}/{:04x} 85bc {:04x}/{:04x}", f + 1, diff.len(),
                w(&ma, 0xff806c), w(&mb, 0xff806c), w(&ma, 0xff806e), w(&mb, 0xff806e), w(&ma, 0xff85bc), w(&mb, 0xff85bc));
        }
        if diff.contains(&(0x144 / 2)) && shown < 8 {
            let t = |m: &cps2::Machine| (0..8).map(|k| format!("{:04x}", m.bus.ram[0x144 / 2 + k])).collect::<Vec<_>>().join(" ");
            println!("image {} tâche 14 : {} | {}", f + 1, t(&ma), t(&mb));
        }
        if !diff.is_empty() && shown < 5 {
            shown += 1;
            print!("image {} : {} mots :", f + 1, diff.len());
            for &i in diff.iter().take(12) {
                print!(" {:06x}", 0xff0000 + 2 * i);
            }
            println!();
        }
    }
    let mut v: Vec<_> = freq.into_iter().collect();
    v.sort_by_key(|&(_, n)| std::cmp::Reverse(n));
    for (i, n) in v.iter().take(25) {
        println!("  {:06x} : {n} images", 0xff0000 + 2 * i);
    }
    println!("fin ({shown} images en écart affichées)");
}
