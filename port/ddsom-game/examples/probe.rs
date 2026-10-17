//! Suivi de l'état de jeu : probe <roms> <eeprom> <scenario> <images> [adresse_hex:taille ...]
//! Affiche à chaque changement : mode (FF0E70-72), phase de partie (FF802C-E), partie active/masque
//! joueurs (FF8030-31), scène (FF8034), + les adresses demandées (taille 1, 2 ou 4).
use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(&a[1]), ee.as_deref()).unwrap();
    ddsom_rc::install(&mut m);
    // comme validate : 68000 surcadencé (DDSOM_OC, défaut 16 ; 1 = vitesse arcade)
    m.set_overclock(std::env::var("DDSOM_OC").ok().and_then(|v| v.parse().ok()).unwrap_or(16.0));
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
        // DUMP=adresse_hex:longueur:image,image,... : hexdump de la RAM à ces images
        if let Ok(d) = std::env::var("DUMP") {
            let p: Vec<&str> = d.split(':').collect();
            let ad = u32::from_str_radix(p[0], 16).unwrap();
            let len: u32 = p[1].parse().unwrap();
            if p[2].split(',').any(|x| x.parse::<usize>().ok() == Some(f + 1)) {
                for o in (0..len).step_by(16) {
                    let row: Vec<String> = (0..16.min(len - o)).map(|k| format!("{:02x}", rd(&m, ad + o + k, 1))).collect();
                    println!("{f:6} {:06x}: {}", ad + o, row.join(" "));
                }
            }
        }
        if st != last {
            println!("{:6} {st}", f + 1);
            last = st;
        }
    }
}
