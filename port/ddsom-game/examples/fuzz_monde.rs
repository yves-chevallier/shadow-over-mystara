//! Fuzz des routines « monde » réécrites avec leurs registres exacts : listes triées (0x1ee1a,
//! 0x1ef6c), collision avec le décor (0x33fe4, 0x3403c), suivi vertical des joueurs (0x31616), aides
//! des effets (0xb9eba, 0xb9ee6, 0xba094). Les aides d'IA n'ont plus de native : `fuzz_ia` compare leurs cœurs.
//!
//! Chaque routine est appelée en mode contrôle (l'original s'exécute à côté sur une copie ; mémoire,
//! tous les registres et les drapeaux sont comparés) depuis des états tirés au sort : listes d'objets
//! fabriquées, positions, tables du décor, registres quelconques.
//!
//!   fuzz_monde <roms> <eeprom> <scénario> <images de chauffe> <tours> [préfixe de routine]
//!
//! Variables : `DDSOM_GRAINE`, `DDSOM_VERBOSE`.

use std::path::Path;

use ddsom_game::mem::{rd16, wr16, wr32, wr8, A5};

struct Alea(u64);

impl Alea {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn dans(&mut self, n: u32) -> u32 {
        self.next() % n
    }
    fn parmi<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.dans(v.len() as u32) as usize]
    }
}

type Cpu = m68k::Cpu;
type Bus = cps2::Cps2Bus;

/// Zone de RAM où l'on fabrique des objets (inutilisée par le jeu à ce moment : on travaille sur une copie).
const ZONE: u32 = 0xffe000;
const PAS: u32 = 0x80;

fn registres(c: &mut Cpu, alea: &mut Alea) {
    for r in 0..8 {
        c.d[r] = match alea.dans(4) {
            0 => alea.next() & 0xff,
            1 => alea.next() & 0xffff,
            _ => alea.next(),
        };
    }
    for r in 1..5 {
        c.a[r] = 0xff0000 | (alea.next() & 0xffff);
    }
    c.sr = (c.sr & !0x1f) | (alea.next() as u16 & 0x1f);
}

fn objet(i: u32) -> u32 {
    ZONE + PAS * i
}

fn pose_y(b: &mut Bus, o: u32, y: u32) {
    wr32(b, o + 0x10, y);
}

/// Profondeur : peu de valeurs pour provoquer des égalités.
fn profondeur(alea: &mut Alea) -> u32 {
    if alea.dans(3) == 0 {
        alea.next()
    } else {
        alea.dans(8) << 16 | alea.parmi(&[0, 0x8000])
    }
}

fn liste(c: &mut Cpu, b: &mut Bus, alea: &mut Alea, tete: u32, queue: u32, fin: u16) {
    let n = alea.dans(7);
    let mut ys: Vec<u32> = (0..n).map(|_| profondeur(alea)).collect();
    ys.sort_by(|a, b| (*b as i32).cmp(&(*a as i32)));
    for (i, &y) in ys.iter().enumerate() {
        let o = objet(i as u32);
        pose_y(b, o, y);
        let prev = if i == 0 { fin } else { objet(i as u32 - 1) as u16 };
        let next = if i as u32 + 1 == n { fin } else { objet(i as u32 + 1) as u16 };
        wr16(b, o + 0x24, prev);
        wr16(b, o + 0x26, next);
    }
    wr16(b, A5 + tete, if n == 0 { 0 } else { objet(0) as u16 });
    wr16(b, A5 + queue, if n == 0 { 0 } else { objet(n - 1) as u16 });
    // le sujet : un objet de la liste dont la profondeur change, ou un objet libre
    if n > 0 && alea.dans(3) != 0 {
        let k = alea.dans(n);
        let y = if alea.dans(2) == 0 { profondeur(alea) } else { ys[k as usize] };
        pose_y(b, objet(k), y);
        c.a[0] = objet(k);
    } else {
        let o = objet(8);
        wr16(b, o + 0x24, 0);
        wr16(b, o + 0x26, 0);
        pose_y(b, o, profondeur(alea));
        c.a[0] = o;
    }
}

fn alea_terrain(c: &mut Cpu, b: &mut Bus, alea: &mut Alea) {
    // attributs de la couche 2 : numéros de terrain valides (la table des types en compte 128)
    for i in (0xa000..0xc000).step_by(2) {
        b.gfxram[i] = alea.next() as u16;
        let numero = if alea.dans(3) == 0 { alea.dans(128) } else { alea.parmi(&[0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]) };
        b.gfxram[i + 1] = (numero << 7) as u16 | (alea.next() as u16 & 0x7f);
    }
    wr8(b, A5 + 0x34, alea.parmi(&[1, 2, 5, 0x14]));
    wr8(b, A5 + 0x36, alea.dans(2) as u8);
    wr16(b, A5 + 0x7fd0, if alea.dans(8) == 0 { 0 } else { 0x800 | alea.next() as u16 & 0x7ff });
    for off in [0x506u32, 0x50a, 0x556, 0x55a] {
        wr16(b, A5 + off, alea.next() as u16 & 0xfff);
    }
    let o = objet(0);
    wr16(b, o + 8, alea.next() as u16 & 0xfff);
    wr16(b, o + 0x10, alea.next() as u16 & 0x3ff);
    wr16(b, o + 0x14, alea.next() as u16);
    c.a[0] = o;
}

fn alea_camera_y(c: &mut Cpu, b: &mut Bus, alea: &mut Alea) {
    let cam = A5 + 0x4fe;
    let y = alea.dans(0x300) as u16;
    wr16(b, cam + 0xc, y);
    for rec in [0x5deu32, 0x7de, 0x9de, 0xbde] {
        wr8(b, A5 + rec, alea.dans(4).min(1) as u8);
        wr16(b, A5 + rec + 0x62, if alea.dans(4) == 0 { 0 } else { alea.next() as u16 & 0x7f });
        wr16(b, A5 + rec + 0x10, y.wrapping_add(alea.dans(0x200) as u16).wrapping_sub(0x40));
    }
    c.a[0] = if alea.dans(4) == 0 { objet(0) } else { cam };
    wr16(b, c.a[0] + 0xc, y);
}

fn alea_effet(c: &mut Cpu, b: &mut Bus, alea: &mut Alea) {
    let (e, autre) = (objet(0), objet(1));
    let (x, y) = (alea.dans(0x400) as u16, alea.dans(0x200) as u16);
    wr16(b, e + 8, x);
    wr16(b, e + 0x10, y);
    for o in [autre] {
        wr16(b, o + 8, x.wrapping_add(alea.dans(0x100) as u16).wrapping_sub(0x80));
        wr16(b, o + 0x10, y.wrapping_add(alea.dans(0x80) as u16).wrapping_sub(0x40));
    }
    for rec in [0x5deu32, 0x7de, 0x9de, 0xbde] {
        wr8(b, A5 + rec, alea.dans(3).min(1) as u8);
        wr16(b, A5 + rec + 0xc, alea.next() as u16 % 0x20 - 8);
        wr16(b, A5 + rec + 8, x.wrapping_add(alea.dans(0x100) as u16).wrapping_sub(0x80));
        wr16(b, A5 + rec + 0x10, y.wrapping_add(alea.dans(0x80) as u16).wrapping_sub(0x40));
    }
    // fils (0xba094)
    let n = alea.dans(6);
    wr16(b, e + 0x58, n as u16);
    for k in 0..6u32 {
        let fils = objet(2 + k);
        wr16(b, e + 0x50 + 2 * k, fils as u16);
        wr8(b, fils + 0x30, alea.parmi(&[0, 1, 1, 3, 0xff]));
    }
    c.a[0] = e;
    c.a[1] = if alea.dans(2) == 0 { autre } else { A5 + 0x5de };
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a.len() < 6 {
        eprintln!("usage : fuzz_monde <roms> <eeprom> <scénario> <images de chauffe> <tours> [préfixe]");
        std::process::exit(2);
    }
    let (roms, eeprom, scenario) = (&a[1], &a[2], &a[3]);
    let chauffe: u32 = a[4].parse().unwrap();
    let tours: u32 = a[5].parse().unwrap();
    let filtre = a.get(6).cloned().unwrap_or_default();
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    ddsom_game::install(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    for f in 0..chauffe as usize {
        script.apply(f, &mut m);
        m.run_frame();
    }
    ddsom_game::set_check(true);
    ddsom_game::set_check_all(true);
    std::panic::set_hook(Box::new(|_| {}));
    let graine: u64 = std::env::var("DDSOM_GRAINE").ok().and_then(|v| v.parse().ok()).unwrap_or(0);
    let mut alea = Alea(0x1234_5678_9abc_def1 ^ graine.wrapping_mul(0x9e37_79b9_7f4a_7c15));
    let (c0, b0) = (m.cpu.clone(), m.bus.clone());

    type Prep = fn(&mut Cpu, &mut Bus, &mut Alea);
    let routines: Vec<(u32, &str, Prep)> = vec![
        (0x1ee1a, "list_reinsert_actors", |c, b, a| liste(c, b, a, 0x5be, 0x5c0, 1)),
        (0x1ef6c, "list_reinsert_foreground", |c, b, a| liste(c, b, a, 0x5c2, 0x5c4, 2)),
        (0x33fe4, "terrain_push", alea_terrain),
        (0x3403c, "terrain_push_2", alea_terrain),
        (0x31616, "camera_suit_joueurs_y", alea_camera_y),
        (0xb9ee6, "effct_dans_la_boite", alea_effet),
        (0xb9eba, "effct_joueur_dans_la_boite", alea_effet),
        (0xba094, "effct_fils_finis", alea_effet),
    ];
    for _ in 0..tours {
        for &(entree, nom, prep) in &routines {
            if !nom.starts_with(&filtre) {
                continue;
            }
            let (mut c, mut b) = (c0.clone(), b0.clone());
            b.irq_horizon = u64::MAX;
            registres(&mut c, &mut alea);
            prep(&mut c, &mut b, &mut alea);
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ddsom_game::call(&mut c, &mut b, entree)));
        }
    }
    let _ = rd16;
    let mut mauvais = 0;
    for (nom, s) in ddsom_game::report() {
        if routines.iter().any(|r| r.1 == nom) {
            println!("{nom:30} {:6} appels {:6} écarts {}", s.calls, s.mismatches, s.first.as_deref().unwrap_or(""));
            mauvais += s.mismatches;
        }
    }
    println!("{}", if mauvais == 0 { "FUZZ OK" } else { "FUZZ ÉCARTS" });
}
