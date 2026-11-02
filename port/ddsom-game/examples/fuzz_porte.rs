//! Fuzz différentiel du changement de zone de la porte de sortie (SET 7, 0xcb29c) : les chemins qui cèdent la
//! main au fondu 0x1460, que `fuzz families` n'atteint pas (il faut tous les joueurs prêts au même moment).
//!
//!   fuzz_porte <roms> <eeprom> <scénario> <chauffe> <essais>
//!
//! `DDSOM_PORTE=29` : la porte de passage (SET 29, 0xd8dce) à la place, que les scénarios n'atteignent pas : état 2
//! avec des joueurs actifs tirés près de la porte (genre de sortie `+0x48` tiré, 0 à 3), ou état 4.
//!
//! Chaque essai part de l'état du jeu après la chauffe, pose une porte de variante tirée, déjà initialisée,
//! dans l'état 2 avec tous les joueurs prêts (genre de sortie `+0x48` tiré, 0 à 3), ou dans l'état 4 (retour
//! de zone, champs mémorisés tirés). Le code d'origine s'exécute seul (interpréteur) jusqu'à son retour ou
//! jusqu'à la première cession du fondu 0x1460 (`jsr 0xa40`) ; la native de même (elle appelle le fondu en Rust,
//! qui cède : `Unwound`). Les deux mémoires sont comparées (hors table des tâches et piles), avec A0 et D7 du
//! contexte rangé par la cession, et la suite rangée dans les appels en cours (l'étape de la porte) avec l'adresse
//! de retour de l'appel d'origine. Puis chacun finit le fondu sans attendre : l'original en revenant aussitôt de
//! chaque `jsr 0xa40`, la native en exécutant les étapes de la tâche jusqu'au retour du gestionnaire. Les mémoires
//! finales sont comparées.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

use cps2::Cps2Bus;
use ddsom_game::mem::{rd32, wr16, wr32, wr8, A5};
use ddsom_game::tasks::table::{self, Resume, Next};
use m68k::Cpu;

/// Entrées des deux portes (types SET 7 et 29).
const EXIT: u32 = 0xcb29c;
const PASSAGE: u32 = 0xd8dce;
const FADING: u32 = 0x1460;
/// Enveloppe de cession (`jsr 0xa40`) appelée par chaque pas du fondu.
const CESSION: u32 = 0xa40;
const SENTINELLE: u32 = 0xfffe;
/// Table des tâches FF0064 et piles : propres à chaque machine.
const IGNORED: std::ops::Range<u32> = 0xff_0000..0xff_0e66;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn among<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.next() as usize % v.len()]
    }
}

/// Code d'origine seul jusqu'au retour à la sentinelle, ou jusqu'à `stop`. Vrai si `stop` est atteint. Avec
/// `without_cession`, chaque `jsr 0xa40` revient aussitôt (le fondu se fait sans attendre).
fn original(c: &mut Cpu, b: &mut Cps2Bus, stop: u32, without_cession: bool) -> Option<bool> {
    for _ in 0..2_000_000 {
        if c.pc == SENTINELLE {
            return Some(false);
        }
        if c.pc == stop {
            return Some(true);
        }
        if without_cession && c.pc == CESSION {
            c.pc = rd32(b, c.a[7]);
            c.a[7] = c.a[7].wrapping_add(4);
            continue;
        }
        c.step(b);
    }
    None
}

/// L'étape où reprendra la tâche courante, qui a cédé (SR du mode utilisateur, comme `kernel::launch`).
fn resume(c: &mut Cpu, b: &mut Cps2Bus) -> Option<ddsom_game::tasks::Step> {
    let t = table::table(b);
    let Resume::Step(step) = t.slots[t.current].resume else { return None };
    c.set_sr(0);
    Some(step)
}

fn calls(c: &mut Cpu, b: &mut Cps2Bus, address: u32) {
    c.a[7] = c.a[7].wrapping_sub(4);
    wr32(b, c.a[7], SENTINELLE);
    c.pc = address;
}

/// Écarts entre les deux mémoires (hors zone ignorée), et entre A0 / D7.
fn gaps(c1: &Cpu, b1: &Cps2Bus, c2: &Cpu, b2: &Cps2Bus) -> Vec<String> {
    let mut d = Vec::new();
    ddsom_game::mem::diff_bus(b1, b2, IGNORED, &mut d);
    if c1.a[0] != c2.a[0] {
        d.push(format!("a0={:08x} au lieu de {:08x}", c1.a[0], c2.a[0]));
    }
    if c1.d[7] as u16 != c2.d[7] as u16 {
        d.push(format!("d7={:08x} au lieu de {:08x}", c1.d[7], c2.d[7]));
    }
    d
}

fn main() {
    std::thread::Builder::new().stack_size(1 << 30).spawn(principal).unwrap().join().unwrap();
}

fn principal() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, warmup, tests, ..] = &a[..] else {
        eprintln!("usage : fuzz_porte <roms> <eeprom> <scénario> <chauffe> <essais>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    m.set_overclock(16.0);
    ddsom_game::install(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    for f in 0..warmup.parse::<usize>().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    ddsom_game::set_trace(|_| {});
    if std::env::var_os("DDSOM_PANIQUES").is_none() {
        std::panic::set_hook(Box::new(|_| {}));
    }
    let passage = std::env::var("DDSOM_PORTE").is_ok_and(|v| v == "29");
    let (entry, kind, variants) = if passage { (PASSAGE, 29u8, 3) } else { (EXIT, 7, 0x38) };
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let (mut summary, mut seen_gaps, mut fadings, mut abandons) = (0u32, 0u32, 0u32, 0u32);
    for test in 0..tests.parse::<u32>().unwrap() {
        let (mut c, mut b) = (m.cpu.clone(), m.bus.clone());
        b.irq_horizon = u64::MAX;
        let door = ddsom_game::objects::pools::SET.slot(rng.next() % 32).0;
        for off in (0..0x50).step_by(2) {
            wr16(&mut b, door + off, 0);
        }
        wr8(&mut b, door, 1);
        wr8(&mut b, door + 2, kind);
        wr8(&mut b, door + 3, (rng.next() % variants) as u8);
        wr8(&mut b, door + 0x28, 0x10);
        // initialisation (état 0) par la native, sur la copie : champs de la table des portes
        c.a[0] = door;
        c.a[5] = A5;
        for (off, v) in [(0x3d, 0), (0x50, 0), (0x42, 0)] {
            wr8(&mut b, A5 + off, v);
        }
        wr16(&mut b, A5 + 0x52, 0);
        ddsom_game::call(&mut c, &mut b, entry);
        let state4 = rng.next() % 3 == 0;
        if state4 {
            wr8(&mut b, door + 4, 4);
            for off in [0x30u32, 0x34, 0x38, 0x3c, 0x4a] {
                wr32(&mut b, door + off, rng.next() << 4);
            }
            for off in [0x18u32, 0x22, 0x14, 0x1c] {
                wr16(&mut b, door + off, rng.next() as u16 & 0x3ff);
            }
            for off in [0x49u32, 0x25, 0x26] {
                wr8(&mut b, door + off, rng.among(&[0, 1, 2]));
            }
            for j in 0..4u32 {
                let p = 0xff_85de + 0x200 * j;
                wr8(&mut b, p + 4, rng.among(&[0, 2, 2, 4]));
            }
        } else if passage {
            wr8(&mut b, door + 4, 2);
            wr8(&mut b, door + 0x40, 0);
            wr8(&mut b, door + 0x48, rng.among(&[0, 0, 1, 1, 2, 3]));
            // joueurs actifs ou non, près de la porte (emprise 0x60 x 0x40) ou un peu au-delà
            for j in 0..4u32 {
                let p = 0xff_85de + 0x200 * j;
                wr8(&mut b, p, rng.among(&[0, 1, 1, 2]));
                for (field, door_field, portee) in [(8u32, 8u32, 0x30u32), (0x10, 0x10, 0x20)] {
                    let gap = (rng.next() % (4 * portee)) as u16;
                    let v = ddsom_game::mem::rd16(&mut b, door + door_field).wrapping_add(gap).wrapping_sub(2 * portee as u16);
                    wr16(&mut b, p + field, v);
                }
            }
        } else {
            wr8(&mut b, door + 4, 2);
            wr8(&mut b, door + 0x48, rng.among(&[0, 0, 1, 1, 2, 3]));
            wr8(&mut b, door + 0x17, 0xf);
            wr8(&mut b, door + 0x1a, 0xf);
            wr8(&mut b, door + 0x16, rng.among(&[4, 4, 1, 0]));
            wr8(&mut b, door + 0x24, rng.next() as u8 & 3);
            wr8(&mut b, A5 + 0x35, rng.among(&[1, 2, 3, 4]));
            wr16(&mut b, door + 0x20, 5);
        }
        wr16(&mut b, A5 + 0x52, if rng.next() % 2 == 0 { 0 } else { door as u16 });
        // quelques objets des autres pools suspendus ou non (réveil, libération)
        for pool in ddsom_game::objects::pools::POOLS {
            for i in 0..pool.slots {
                let o = pool.slot(i).0;
                if ddsom_game::mem::rd8(&mut b, o) != 0 && rng.next() % 3 == 0 {
                    wr8(&mut b, o + 0x40, rng.among(&[0, 1, 2]));
                }
            }
        }
        c.a[0] = door;
        c.a[5] = A5;
        c.d[7] = rng.next() & 0x1f;
        c.d[0] = kind as u32 * 4;
        c.a[1] = entry;
        let (mut c2, mut b2) = (c.clone(), b.clone());
        // l'original, jusqu'à l'entrée du fondu (adresse de retour au sommet de la pile), puis jusqu'à sa cession
        calls(&mut c2, &mut b2, entry);
        let Some(fading) = original(&mut c2, &mut b2, FADING, false) else {
            abandons += 1;
            continue;
        };
        let ret = rd32(&mut b2, c2.a[7]);
        if fading && original(&mut c2, &mut b2, CESSION, false) != Some(true) {
            abandons += 1;
            continue;
        }
        // la native : le fondu cède la main (`call` s'arrête alors sur une panique), ou elle revient
        let r = catch_unwind(AssertUnwindSafe(|| ddsom_game::call(&mut c, &mut b, entry)));
        let native_fading = match r {
            Ok(()) => false,
            Err(e) if e.downcast_ref::<String>().is_some_and(|m| m.contains("a cédé la main")) => true,
            Err(_) => {
                println!("essai {test} : panique de la native");
                seen_gaps += 1;
                continue;
            }
        };
        summary += 1;
        if fading != native_fading {
            println!("essai {test} : fondu {fading} (original) / {native_fading} (native)");
            seen_gaps += 1;
            continue;
        }
        let step = if fading { resume(&mut c, &mut b) } else { None };
        let d = gaps(&c, &b, &c2, &b2);
        if !d.is_empty() {
            println!("essai {test} état4={state4} variante {:02x} : avant le fondu {}", ddsom_game::mem::rd8(&mut b2, door + 3), d.join(", "));
            seen_gaps += 1;
            continue;
        }
        if !fading {
            continue;
        }
        fadings += 1;
        // la suite du fondu : l'étape de la porte, à l'adresse de retour de l'appel d'origine
        let next = table::table(&mut b).frames().top().map(|k| k.next);
        if !matches!(next, Some(Next::Step(e)) if e.address() == ret) {
            println!("essai {test} : suite {next:x?} au lieu de {ret:06x}");
            seen_gaps += 1;
            continue;
        }
        // fin du fondu sans attendre : chaque pas de la tâche, jusqu'au retour du gestionnaire
        let mut step = step;
        let r = catch_unwind(AssertUnwindSafe(|| {
            for _ in 0..64 {
                let Some(e) = step else { break };
                ddsom_game::tasks::resume_step(e, &mut c, &mut b);
                if c.pc == SENTINELLE {
                    return true;
                }
                step = resume(&mut c, &mut b);
            }
            false
        }));
        if r.ok() != Some(true) || original(&mut c2, &mut b2, SENTINELLE, true) != Some(false) {
            println!("essai {test} : reprise interrompue");
            seen_gaps += 1;
            continue;
        }
        let d = gaps(&c, &b, &c2, &b2);
        if !d.is_empty() {
            println!("essai {test} état4={state4} variante {:02x} : après le fondu {}", ddsom_game::mem::rd8(&mut b2, door + 3), d.join(", "));
            seen_gaps += 1;
        }
    }
    println!("{summary} essais comparés, {fadings} avec fondu, {abandons} abandons, {seen_gaps} écarts");
    println!("{}", if seen_gaps == 0 { "FUZZ OK" } else { "FUZZ ÉCARTS" });
}
