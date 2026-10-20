//! Quels types d ennemis (pool ENEMY, A5+0x299e) apparaissent dans quelle scène ?
//!   enemy_scan <roms> <eeprom> <base.txt> <scène_début> <scène_fin> [images_par_scène]
//! Joue la base (pièce, Start, personnage), puis saute de scène en scène (FF8034, phase 8) sur la machine
//! d'origine, joueur invincible, et note les types de boss vus.
use std::path::Path;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let ee = std::fs::read(&a[2]).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(&a[1]), ee.as_deref()).expect("roms");
    m.set_overclock(16.0);
    ddsom_game::install_reference(&mut m);
    m.idle = Some(ddsom_game::idle);
    let base = ddsom_game::scenario::Scenario::load(Path::new(&a[3])).unwrap();
    let (s0, s1): (u32, u32) = (a[4].parse().unwrap(), a[5].parse().unwrap());
    let per: usize = a.get(6).map_or(500, |v| v.parse().unwrap());
    let rd8 = |m: &cps2::Machine, ad: u32| {
        let o = (ad - 0xff0000) as usize;
        let w = m.bus.ram[o / 2];
        if o % 2 == 0 { (w >> 8) as u8 } else { w as u8 }
    };
    let mut f = 0;
    while f < 2400 {
        base.apply(f, &mut m);
        m.run_frame();
        f += 1;
    }
    let mut scene = s0;
    ddsom_game::scenario::poke(&mut m, 0xff8034, scene, 1);
    let mut since = 0;
    let mut seen = std::collections::BTreeSet::new();
    loop {
        base.apply(f, &mut m);
        for k in 0..2 {
            ddsom_game::scenario::poke(&mut m, 0xff85de + 0x200 * k + 0x62, 0x70, 2);
        }
        m.run_frame();
        f += 1;
        let phase = rd8(&m, 0xff802c);
        if f > 2700 && phase != 8 && phase != 0x10 && phase != 4 {
            since += 1;
            for k in 0..22 {
                let o = 0xff8000 + 0x299e + 0xc0 * k;
                if rd8(&m, o) != 0 {
                    seen.insert((rd8(&m, o + 2), rd8(&m, o + 3)));
                }
            }
        }
        if since >= per {
            println!("scène {scene} : ennemis (type, variante) {:?}", seen);
            seen.clear();
            since = 0;
            scene += 1;
            if scene > s1 {
                break;
            }
            ddsom_game::scenario::poke(&mut m, 0xff8034, scene, 1);
            ddsom_game::scenario::poke(&mut m, 0xff802c, 8, 1);
            ddsom_game::scenario::poke(&mut m, 0xff802d, 0, 1);
            ddsom_game::scenario::poke(&mut m, 0xff802e, 0, 1);
        }
    }
}
