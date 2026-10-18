//! Validation différentielle des actions des personnages sur des états aléatoires : après une chauffe
//! sur un scénario (un joueur en pleine partie), chaque action native d'un personnage est appelée en
//! mode contrôle sur des états tirés au sort (pas, sous-action, entrées, événement, armes, position,
//! vitesses...) ; l'original s'exécute à côté sur une copie et la mémoire est comparée. Exerce les
//! chemins que les scénarios n'atteignent pas (objets, magies, coups rares).
//!
//!   fuzz_player <personnage> <roms> <eeprom> <scénario> <images de chauffe> <tours>
//!
//! `personnage` : `fighter` ou `cleric` (préfixe du nom des natives). `DDSOM_ENTREE=<hexa>` limite le
//! tir à une seule entrée.

use std::path::Path;

use ddsom_game::mem::{rd8, wr16, wr32, wr8, A5};
use ddsom_game::player::Player;

struct Alea(u64);

impl Alea {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn below(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.below(v.len() as u32) as usize]
    }
}

/// Plus grand pas valide de l'action `entry` pour la sous-action `sub` (les autres valeurs feraient
/// sauter l'original dans des données).
fn max_step(name: &str, sub: u8, which: u8) -> u8 {
    match name {
        "fighter_jump" | "cleric_jump" => 8,
        "fighter_dash" | "cleric_dash" | "elf_dash" => 6,
        "elf_jump" => 0xc,
        "mage_jump" => 0xa,
        "mage_dash" => 8,
        "mage_attack" => match sub {
            0 | 2 | 4 | 6 | 0xc | 0xe | 0x14 | 0x16 | 0x18 | 0x1a => 4,
            _ => 2,
        },
        "mage_special" => match which {
            9 => 6,
            0xa | 0xd..=0x14 | 0x1f => 4,
            0x18 | 0x1b | 0x1d | 0x1e | 0x20..=0x24 => 8,
            0x19 => 0xa,
            0x1a => 0xc,
            0x1c => 0x10,
            _ => 2,
        },
        "elf_attack" => match sub {
            0 => 6,
            6 | 0xc | 0xe | 0x1a | 0x1c => 4,
            0x16 => 8,
            _ => 2,
        },
        "elf_special" => match which {
            9 => 6,
            0xa | 0xd..=0x14 | 0x17 | 0x1b => 4,
            0x15 | 0x16 | 0x1a => 8,
            0x18 | 0x19 => 6,
            0x1c => 0xc,
            _ => 2,
        },
        "fighter_attack" => match sub {
            0 | 6 | 0xc | 0xe => 4,
            0x16 => 0x12,
            _ => 2,
        },
        "cleric_attack" => match sub {
            0 | 6 | 0xc | 0xe => 4,
            0x16 => 0xa,
            0x18 => 6,
            _ => 2,
        },
        "fighter_special" => match which {
            9 => 6,
            10 => 4,
            12 => 2,
            13..=20 | 0..=8 => 2,
            _ => 0,
        },
        "cleric_special" => match which {
            9 | 0x24 | 0x26 | 0x29 | 0x2a | 0x2c | 0x2d => 6,
            10 | 0x28 => 4,
            0x27 => 0xa,
            0x2b => 8,
            _ => 2,
        },
        _ => 0,
    }
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let [_, who, roms, eeprom, scenario, warm, rounds] = &a[..] else {
        eprintln!("usage : fuzz_player <fighter|cleric> <roms> <eeprom> <scénario> <images de chauffe> <tours>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    ddsom_game::install(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    for f in 0..warm.parse().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    if std::env::var("DDSOM_PROBE").is_ok() {
        // sonde : soin d'origine (0x41656) pour chaque niveau et chaque sort
        let (c, b) = (&mut m.cpu, &mut m.bus);
        let p = Player::nth(0).0;
        for kind in [0x13u8, 0x2c, 0x2d] {
            let mut out = Vec::new();
            for level in 0..20u8 {
                wr8(b, p + 0x15e, kind);
                wr8(b, p + 0x10c, level);
                wr8(b, p + 0x1ee, 0);
                wr16(b, p + 0x60, 0x7fff);
                wr16(b, p + 0x62, 0);
                c.a[0] = p;
                c.a[5] = A5;
                ddsom_game::call(c, b, 0x41656);
                out.push(ddsom_game::mem::rd16(b, p + 0x62));
            }
            println!("{kind:#x} : {out:?}");
        }
        return;
    }
    ddsom_game::set_check(true);
    let only = std::env::var("DDSOM_ENTREE").ok().and_then(|v| u32::from_str_radix(&v, 16).ok());
    let entries: Vec<(u32, &'static str)> = ddsom_game::all_natives()
        .filter(|n| n.name.starts_with(who.as_str()) && only.map_or(true, |e| e == n.entry))
        .map(|n| (n.entry, n.name))
        .collect();
    let (c, b) = (&mut m.cpu, &mut m.bus);
    b.irq_horizon = u64::MAX;
    let seed: u64 = std::env::var("DDSOM_SEED").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut alea = Alea(0x9e37_79b9_7f4a_7c15 ^ seed.wrapping_mul(0x2545_f491_4f6c_dd1d));
    let mut shown = 0;
    let p = Player::nth(0).0;
    println!("joueur 0 : état {:08x}, personnage {}", ddsom_game::mem::rd32(b, p + 4), rd8(b, p + 3));
    for _ in 0..rounds.parse::<u32>().unwrap() {
        for &(entry, name) in &entries {
            for r in 0..8 {
                c.d[r] = alea.next();
            }
            c.d[0] = 0;
            c.a[0] = p;
            c.a[5] = A5;
            // état aléatoire du joueur
            let mut sub = alea.pick(&[0u8, 2, 4, 6, 8, 0xa, 0xc, 0xe, 0x10, 0x12, 0x14, 0x16, 0x18, 0x1a, 0x1c]);
            if who == "elf" && alea.below(15) == 0 {
                sub = 0x1e;
            }
            let which = if who == "fighter" {
                alea.pick(&[0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20])
            } else if who == "elf" {
                alea.below(0x1d) as u8
            } else if who == "mage" {
                alea.below(0x25) as u8
            } else {
                alea.pick(&[
                    0u8, 1, 6, 7, 8, 9, 10, 12, 13, 14, 15, 16, 17, 18, 19, 20, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29,
                    0x2a, 0x2b, 0x2c, 0x2d,
                ])
            };
            let max = max_step(name, sub, which);
            let step = 2 * alea.below(max as u32 / 2 + 1) as u8;
            wr8(b, p + 4, 2);
            wr8(b, p + 5, 0);
            wr8(b, p + 6, 0);
            wr8(b, p + 7, step);
            wr8(b, p + 0x6a, sub);
            wr8(b, p + 0x15e, which);
            wr8(b, A5 + 0x5b, which);
            wr8(b, p + 0x6c, alea.next() as u8);
            wr8(b, p + 0x6d, alea.next() as u8);
            wr8(b, p + 0x6e, alea.next() as u8);
            wr8(b, p + 0x2c, alea.pick(&[0u8, 0x20]));
            wr8(b, p + 0x33, alea.pick(&[0u8, 0, 1, 2]));
            wr8(b, p + 0x46, alea.pick(&[0u8, 0, 0, 1, 2, 3]));
            wr8(b, p + 0x67, alea.below(4) as u8);
            wr8(b, p + 0x68, alea.pick(&[0u8, 1, 1]));
            wr8(b, p + 0x69, alea.next() as u8);
            wr8(b, p + 0x6b, alea.below(2) as u8);
            wr8(b, p + 0x70, alea.pick(&[0u8, 1, 2, 4, 3, 7]));
            wr8(b, p + 0x72, alea.below(6) as u8);
            wr8(b, p + 0x73, alea.below(2) as u8);
            wr8(b, p + 0x74, alea.below(8) as u8);
            wr8(b, p + 0x75, alea.below(2) as u8);
            wr8(b, p + 0x58, alea.below(2) as u8);
            wr8(b, p + 0x66, alea.below(2) as u8);
            wr16(b, p + 0x08, 0x40 + alea.below(0x180) as u16);
            wr16(b, p + 0x0c, alea.pick(&[0u16, 0, 0x10, 0xfff0, 0x30]));
            wr16(b, p + 0x10, 0x40 + alea.below(0x60) as u16);
            for off in [0x14u32, 0x16, 0x18, 0x1a, 0x1e] {
                let e = alea.next() as u16;
                wr16(b, p + off, alea.pick(&[0u16, 1, 0x100, 0x200, 0x500, 0xfe00, 0xfff0, 0x180, 0x181, e]));
            }
            wr16(b, p + 0x80, alea.next() as u16 & 0x1ff);
            let e = alea.below(0x2f) as u8;
            wr8(b, p + 0x10d, alea.pick(&[0u8, 1, 8, 0x18, 0xe, 0x1e, 0x10, 0x11, 0x14, 0x20, e]));
            let e = alea.below(0x2f) as u8;
            wr8(b, p + 0x10e, alea.pick(&[0u8, 4, 5, 0x20, 0x28, 0x29, 0x2c, e]));
            wr8(b, p + 0x10c, alea.below(20) as u8);
            wr8(b, p + 0x15f, 0x2e + alea.below(10) as u8);
            let e = alea.next() as u8;
            wr8(b, p + 0x140, alea.pick(&[0x2d, 0x56, 0x10, e]));
            wr8(b, p + 0x148, alea.pick(&[1u8, 1, 2, 5]));
            for off in [0x152u32, 0x153, 0x154, 0x155, 0x157] {
                wr8(b, p + off, alea.pick(&[0u8, 1, 2, 3]));
            }
            wr8(b, p + 0x164, alea.below(4) as u8);
            wr8(b, p + 0x165, alea.pick(&[0u8, 0, 3]));
            wr8(b, p + 0x167, alea.pick(&[0u8, 2, 0xff]));
            let e = alea.next() as u16 & 0xff;
            wr16(b, p + 0x168, alea.pick(&[0u16, 1, 2, 0x10, 0x20, 0x60, 0x61, 0xc0, e]));
            wr8(b, p + 0x182, alea.pick(&[0u8, 0, 1, 2, 3, 4]));
            wr8(b, p + 0x189, alea.pick(&[0u8, 0, 0, 1]));
            wr8(b, p + 0x18f, alea.below(2) as u8);
            wr8(b, p + 0x1e8, alea.below(14) as u8);
            wr8(b, p + 0x1e9, alea.below(4) as u8);
            wr8(b, p + 0x1ec, alea.below(2) as u8);
            wr8(b, p + 0x1ed, alea.below(3) as u8);
            wr8(b, p + 0x1ee, alea.below(2) as u8);
            wr16(b, p + 0x62, 1 + alea.below(100) as u16);
            wr16(b, p + 0x60, 100);
            wr8(b, A5 + 0x6f, alea.next() as u8);
            wr8(b, A5 + 0x6d, alea.next() as u8);
            wr8(b, A5 + 0x3d, alea.pick(&[0u8, 0, 1]));
            wr8(b, A5 + 0x56, alea.pick(&[0u8, 0x80, 0x4f]));
            // personnage joué : classe et variante
            let base = match who.as_str() {
                "fighter" => 0,
                "cleric" => 2,
                "elf" => 4,
                _ => 8,
            };
            wr8(b, p + 3, base + alea.below(2) as u8);
            // champs des classes elfe et magicien : accessoires, cibles, états de saut et de sort
            wr8(b, p + 0x59, alea.below(4) as u8);
            wr8(b, p + 0x15d, alea.pick(&[0u8, 2, 2]));
            wr8(b, p + 0x166, alea.pick(&[0u8, 4]));
            wr8(b, p + 0x165, alea.pick(&[0u8, 3, 4, 5]));
            wr8(b, p + 0x1ea, alea.below(5) as u8);
            wr8(b, p + 0x1eb, alea.below(4) as u8);
            wr8(b, p + 0x1ef, alea.below(2) as u8);
            wr8(b, p + 0x151, alea.pick(&[0u8, 1, 2, 3]));
            wr8(b, p + 0x156, alea.pick(&[0u8, 1, 2, 3]));
            wr8(b, p + 0x158, alea.pick(&[0u8, 1, 2, 3]));
            wr8(b, p + 0x159, alea.pick(&[0u8, 1, 2, 3]));
            wr8(b, A5 + 0x48f, alea.pick(&[0u8, 1]));
            wr8(b, p + 0x18a, 0);
            // cible mémorisée (foudre, mur, bénédiction) : un monstre ou un joueur
            let target = if alea.below(2) == 0 && which != 0x15 {
                0x299e + 0xc0 * alea.below(5) as u16
            } else {
                0x5de + 0x200 * alea.below(4) as u16
            } | 0x8000;
            if who == "elf" {
                wr16(b, p + 0x1ec, target);
            } else if who == "mage" && (which == 9 || which == 10) {
                wr16(b, p + 0x1e8, target);
            }
            // autres joueurs : parfois en jeu, devant ou derrière
            for k in 1..4u32 {
                let q = p + 0x200 * k;
                wr8(b, q, alea.pick(&[0u8, 1, 1]));
                wr8(b, q + 1, alea.pick(&[0u8, 1, 2, 3]));
                wr8(b, q + 4, alea.pick(&[2u8, 2, 4]));
                wr8(b, q + 6, alea.pick(&[0u8, 0, 0x22, 4]));
                wr8(b, q + 0x10f, alea.pick(&[0u8, 0, 1]));
                wr8(b, q + 0x8a, alea.pick(&[0u8, 0, 1]));
                wr16(b, q + 0x62, alea.pick(&[0u16, 30, 100]));
                wr16(b, q + 0x60, 100);
                wr16(b, q + 0x0c, alea.pick(&[0u16, 0, 0, 8]));
                wr16(b, q + 0x08, 0x40 + alea.below(0x180) as u16);
                wr16(b, q + 0x10, 0x40 + alea.below(0x60) as u16);
                wr8(b, q + 0x78, alea.pick(&[0u8, 0, 1]));
                wr16(b, q + 0x7a, alea.pick(&[0u16, 0, 0x100]));
            }
            // monstres : certains en jeu, de types variés
            for pool in [(0x3a1eu32, 3u32), (0x299e, 22)] {
                for k in 0..pool.1 {
                    let o = A5 + pool.0 + 0xc0 * k;
                    if alea.below(2) == 0 {
                        continue;
                    }
                    wr8(b, o, alea.pick(&[0u8, 1, 1, 1]));
                    wr8(b, o + 1, alea.pick(&[0u8, 1, 1, 3]));
                    wr8(b, o + 2, alea.pick(&[0u8, 1, 5, 0x13, 2, 3]));
                    wr8(b, o + 0x28, alea.pick(&[2u8, 4]));
                    wr16(b, o + 0x62, alea.pick(&[0u16, 5, 40, 90]));
                    wr16(b, o + 0x60, 90);
                    wr16(b, o + 0x0c, alea.pick(&[0u16, 0, 0, 12]));
                    wr8(b, o + 0x40, alea.pick(&[0u8, 0, 0, 1]));
                    wr8(b, o + 0x49, alea.pick(&[0u8, 0, 0x9c]));
                    wr8(b, o + 0x93, alea.pick(&[0u8, 1]));
                    wr32(b, o + 0x94, alea.next() & alea.next());
                }
            }
            // la cible de la foudre et du mur : un objet quelconque
            if who == "fighter" || who == "cleric" {
                wr16(b, p + 0x1ea, 0x299e + 0xc0 * alea.below(5) as u16 | 0x8000);
            }
            let before = ddsom_game::report().values().map(|s| s.mismatches).sum::<u64>();
            let snapshot = (rd8(b, p + 7), rd8(b, p + 0x6a));
            let context: String = [0x10du32, 0x10e, 0x6c, 0x6d, 0x6e, 0x67, 0x6b, 0x73, 0x33, 0x46, 0x70, 0x2c, 0x189, 0x140, 0x148]
                .iter()
                .map(|o| format!("{o:x}={:02x} ", rd8(b, p + o)))
                .collect();
            if std::env::var("DDSOM_TRACE").is_ok() {
                eprintln!("{entry:06x} {name} pas {} sous-action {:#x} 15e={which:#x} {context}", snapshot.0, snapshot.1);
            }
            ddsom_game::call(c, b, entry);
            let after = ddsom_game::report().values().map(|s| s.mismatches).sum::<u64>();
            if after > before && shown < 12 {
                shown += 1;
                let last = ddsom_game::report().get(name).and_then(|s| s.last.clone());
                println!(
                    "écart en {entry:06x} {name} (pas {} sous-action {:#x}, 15e={which:#x}) : {}\n    {context}",
                    snapshot.0,
                    snapshot.1,
                    last.unwrap_or_default()
                );
            }
        }
    }
    let mut total = 0;
    for (name, st) in ddsom_game::report() {
        total += st.mismatches;
        println!("  {name:28} {:>7} appels {:>5} écarts{}", st.calls, st.mismatches, st.first.map(|f| format!(" — {f}")).unwrap_or_default());
    }
    println!("{}", if total == 0 { "VALIDE : aucun écart" } else { "ÉCHEC" });
}
