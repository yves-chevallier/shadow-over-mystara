//! Fuzz différentiel de la RAM supplémentaire de la carte de développement (`eeprom/carte_dev.rs`).
//!
//! Sur des états aléatoires de la RAM 0x660000 (zones valides, exemplaires corrompus, compteurs près de
//! leurs bornes, RAM vierge ou aléatoire), compare le code d'origine (68000 recompilé, sans aucune native) à
//! la version Rust : mémoire complète et registres rendus.
//!
//!   fuzz_carte_dev <roms> <eeprom> [essais [graine]]

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

use cps2::Cps2Bus;
use m68k::Cpu;

use ddsom_game::mem::{rd32, rd8, wr16, wr32, wr8, A5};

struct Alea(u64);

impl Alea {
    fn suivant(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn dans(&mut self, n: u64) -> u64 {
        self.suivant() % n
    }
    fn parmi<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.dans(v.len() as u64) as usize]
    }
}

const RAM: u32 = 0x66_0000;
/// (décalage, mots) des zones de la RAM supplémentaire.
const ZONES: &[(u32, u32)] =
    &[(0x1140, 32), (0x1020, 4), (0x1028, 4), (0x1030, 4), (0x1038, 4), (0x1040, 4), (0x1048, 4), (0x1200, 48), (0x1260, 8), (0x1270, 2)];

fn ecrire_long(b: &mut Cps2Bus, a: u32, v: u32) {
    for i in 0..4 {
        wr8(b, a + 1 + 2 * i, (v >> (24 - 8 * i)) as u8);
    }
}

/// Un état plausible de la RAM supplémentaire pour la région `r`.
fn etat(b: &mut Cps2Bus, a: &mut Alea, r: u32) {
    let valeurs: [u32; 12] = [0, 1, 5, 999_998, 999_999, 1_000_000, 0x127a_3980, 0x127a_3981, 0x4f_5880, 0x4f_5881, 0xffff_ffff, 0x1234_5678];
    for k in 0..3 {
        for &(debut, mots) in ZONES {
            let base = RAM + debut + 0x1000 * k;
            for i in 0..mots {
                wr16(b, base + 2 * i, 0);
            }
        }
    }
    let mode = a.dans(10);
    for &(debut, mots) in ZONES {
        // une valeur commune aux trois exemplaires
        let mut octets = vec![0u8; mots as usize];
        for (i, o) in octets.iter_mut().enumerate() {
            *o = match debut {
                0x1140 => ddsom_game::mem::rd8(b, 0x2ebfc + ((r << 4) as u16 as u32) + i as u32),
                _ => {
                    if i % 4 == 0 && mots == 4 {
                        0
                    } else {
                        { let r = a.suivant() as u8; a.parmi(&[0, 0, 1, 2, 0xff, r]) }
                    }
                }
            };
        }
        if mots == 4 {
            let v = if a.dans(3) == 0 { a.suivant() as u32 } else { a.parmi(&valeurs) };
            for (i, o) in octets.iter_mut().enumerate() {
                *o = (v >> (24 - 8 * i as u32)) as u8;
            }
        }
        for k in 0..3 {
            let base = RAM + debut + 0x1000 * k;
            for (i, &o) in octets.iter().enumerate() {
                match mode {
                    0 => wr16(b, base + 2 * i as u32, a.suivant() as u16),
                    1 => {}
                    _ => wr16(b, base + 2 * i as u32, if a.dans(4) == 0 { (a.suivant() as u16) & 0xff00 | o as u16 } else { o as u16 }),
                }
            }
        }
        // corruption : un exemplaire, parfois deux, quelques octets
        if mode >= 2 && a.dans(2) == 0 {
            for _ in 0..1 + a.dans(3) {
                let k = a.dans(3) as u32;
                let i = a.dans(mots as u64 * 2) as u32;
                let v = a.suivant() as u8;
                wr8(b, RAM + debut + 0x1000 * k + i, v);
            }
        }
    }
    // variables de travail
    for off in [0x6164u32, 0x6160, 0x615c, 0x6158, 0x6154, 0x6150, 0x611c, 0x6118] {
        let v = a.parmi(&valeurs);
        wr32(b, A5 - off, v);
    }
    for i in 0..48 {
        wr8(b, A5 - 0x614c + i, a.suivant() as u8);
    }
}

fn execute_origine(c: &mut Cpu, b: &mut Cps2Bus) -> bool {
    let ret = ddsom_game::mem::rd32(b, c.a[7]);
    let sp_apres = c.a[7].wrapping_add(4);
    let mut garde = 0;
    while !(c.pc == ret && c.a[7] == sp_apres) {
        if ddsom_rc::run_block(c.pc, c, b, u64::MAX).is_none() {
            c.step(b);
        }
        garde += 1;
        if garde > 200_000 {
            return false;
        }
    }
    true
}

type Rust = Box<dyn Fn(&mut Cpu, &mut Cps2Bus)>;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (Some(roms), Some(eeprom)) = (a.get(1), a.get(2)) else {
        eprintln!("usage : fuzz_carte_dev <roms> <eeprom> [essais [graine]]");
        std::process::exit(2);
    };
    let essais: u32 = a.get(3).map_or(2000, |s| s.parse().unwrap());
    let mut alea = Alea(0x9e37_79b9_7f4a_7c15 ^ a.get(4).map_or(1, |s| s.parse::<u64>().unwrap()).wrapping_mul(0x1000_0000_01b3));
    std::panic::set_hook(Box::new(|_| {}));
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    ddsom_game::install(&mut m);
    // quelques images : la RAM de travail est celle d'un jeu qui a démarré
    for _ in 0..700 {
        m.run_frame();
    }
    // (nom, entrée d'origine, routine Rust, registre D0 aléatoire ?)
    let natives = |entree: u32| -> Rust {
        let n = ddsom_game::all_natives().find(|n| n.entry == entree).expect("native");
        Box::new(move |c, b| n.executer(c, b))
    };
    let cas: Vec<(&str, u32, Rust)> = vec![
        ("démarrage 0x2dc9a", 0x2dc9a, natives(0x2dc9a)),
        ("réinitialisation 0x2e8e4", 0x2e8e4, Box::new(|c, b| {
            ddsom_game::eeprom::reinitialiser_carte(c, b);
            ddsom_game::mem::rts(c, b);
        })),
        ("temps de jeu 0x2e540", 0x2e540, Box::new(|c, b| {
            ddsom_game::eeprom::carte_dev::temps_de_jeu(b);
            ddsom_game::mem::rts(c, b);
        })),
        ("pièce 0x2e448", 0x2e448, natives(0x2e448)),
        ("départ 0x2e49a", 0x2e49a, natives(0x2e49a)),
        ("parties 0x2e4ec", 0x2e4ec, natives(0x2e4ec)),
        ("personnage 0x2e630", 0x2e630, natives(0x2e630)),
        ("durée 0x2e696", 0x2e696, natives(0x2e696)),
    ];
    let mut total = 0;
    for (nom, entree, rust) in &cas {
        let (mut ecarts, mut paniques, mut abandons) = (0, 0, 0);
        let mut exemples = Vec::new();
        for _ in 0..essais {
            let mut c = m.cpu.clone();
            let mut b = m.bus.clone();
            let region = alea.dans(7) as u32;
            wr16(&mut b, A5 + 0x70, region as u16);
            wr8(&mut b, A5 + 0x82, 1);
            wr8(&mut b, A5 + 0x25, alea.dans(2) as u8);
            etat(&mut b, &mut alea, region);
            c.d[0] = match *entree {
                0x2e630 => alea.parmi(&[0, 1, 2, 3, 4, 5, 6, 11, 0xffff, 0x1234_0005]),
                0x2e696 => {
                    let (p, q) = (alea.suivant() as u32 & 0xffff, alea.suivant() as u32);
                    alea.parmi(&[0, 59, 60, 61, 3600, 0x7fff_ffff, 0x8000_0000, 0xffff_ffff, p, q])
                }
                _ => c.d[0],
            };
            // la durée doit se comparer à la plus longue / plus courte du moment
            c.a[7] = 0xff_f000;
            c.a[7] -= 4;
            wr32(&mut b, c.a[7], 0xfffe);
            c.pc = *entree;
            let (mut c2, mut b2) = (c.clone(), b.clone());
            let avant = b.clone();
            if !execute_origine(&mut c2, &mut b2) {
                abandons += 1;
                continue;
            }
            let r = catch_unwind(AssertUnwindSafe(|| rust(&mut c, &mut b)));
            if r.is_err() {
                paniques += 1;
                continue;
            }
            let mut diffs = Vec::new();
            let sp = c2.a[7] & 0xff_ffff;
            ddsom_game::mem::diff_bus(&b, &b2, sp.saturating_sub(0x400)..sp, &mut diffs);
            if std::env::var_os("FUZZ_REGISTRES").is_some() {
                for i in 0..8 {
                    if c.d[i] != c2.d[i] {
                        diffs.push(format!("d{i}={:08x} au lieu de {:08x}", c.d[i], c2.d[i]));
                    }
                    if i < 7 && c.a[i] != c2.a[i] {
                        diffs.push(format!("a{i}={:08x} au lieu de {:08x}", c.a[i], c2.a[i]));
                    }
                }
            }
            if c.pc != c2.pc {
                diffs.push(format!("pc={:06x} au lieu de {:06x}", c.pc, c2.pc));
            }
            if !diffs.is_empty() {
                ecarts += 1;
                if std::env::var_os("FUZZ_DUMP").is_some() && exemples.is_empty() {
                    for (nom, bus) in [("avant", &avant), ("rust", &b), ("origine", &b2)] {
                        for k in 0..3usize {
                            let i = (0x1020 + 0x1000 * k) / 2;
                            let sig = (0x1140 + 0x1000 * k) / 2;
                            println!("{nom} {k}: {:04x?} sig {:04x?}", &bus.extra_ram[i..i + 4], &bus.extra_ram[sig..sig + 4]);
                        }
                    }
                }
                if exemples.len() < 3 {
                    exemples.push(format!("région {region} d0={:08x} : {}", c2.d[0], diffs.join(", ")));
                }
            }
        }
        println!("{nom:28} {essais} essais, {ecarts} écarts, {paniques} paniques, {abandons} abandons");
        for e in exemples {
            println!("    {e}");
        }
        total += ecarts + paniques;
    }
    let _ = rd32;
    let _ = rd8;
    println!("{}", if total == 0 { "FUZZ OK" } else { "FUZZ ÉCARTS" });
}
