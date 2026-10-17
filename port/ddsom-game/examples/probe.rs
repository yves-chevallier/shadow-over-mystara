//! Suivi de l'état de jeu : probe <roms> <eeprom> <scenario> <images> [adresse_hex:taille ...]
//! Affiche à chaque changement : mode (FF0E70-72), phase de partie (FF802C-E), partie active/masque
//! joueurs (FF8030-31), scène (FF8034), + les adresses demandées (taille 1, 2 ou 4).
use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(&a[1]), ee.as_deref()).unwrap();
    ddsom_rc::install(&mut m);
    let s = ddsom_game::scenario::Scenario::load(Path::new(&a[3])).unwrap();
    let frames: usize = a[4].parse().unwrap();
    let extra: Vec<(u32, u32)> = a[5..]
        .iter()
        .map(|t| {
            let (x, n) = t.split_once(':').unwrap_or((t, "1"));
            (u32::from_str_radix(x, 16).unwrap(), n.parse().unwrap())
        })
        .collect();
    let rd = |m: &cps2::Machine, ad: u32, n: u32| -> u32 {
        (0..n).fold(0, |v, k| {
            let o = (ad + k - 0xff0000) as usize;
            let w = m.bus.ram[o / 2];
            v << 8 | if o % 2 == 0 { w >> 8 } else { w & 0xff } as u32
        })
    };
    let mut last = String::new();
    for f in 0..frames {
        s.apply(f, &mut m);
        m.run_frame();
        let mut st = format!(
            "mode {:02x}{:02x}{:02x} phase {:02x}/{:02x}/{:02x} actif {:02x} joueurs {:02x} scene {:02x} f35 {:02x}",
            rd(&m, 0xff0e70, 1), rd(&m, 0xff0e71, 1), rd(&m, 0xff0e72, 1),
            rd(&m, 0xff802c, 1), rd(&m, 0xff802d, 1), rd(&m, 0xff802e, 1),
            rd(&m, 0xff8030, 1), rd(&m, 0xff8031, 1), rd(&m, 0xff8034, 1), rd(&m, 0xff8035, 1));
        for &(ad, n) in &extra {
            st += &format!(" [{ad:x}]={:0w$x}", rd(&m, ad, n), w = 2 * n as usize);
        }
        if st != last {
            println!("{:6} {st}", f + 1);
            last = st;
        }
    }
}
