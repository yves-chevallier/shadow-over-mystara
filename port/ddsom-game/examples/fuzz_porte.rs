//! Fuzz différentiel du changement de zone de la porte de sortie (SET 7, 0xcb29c) : les chemins qui cèdent la
//! main au fondu 0x1460, que `fuzz familles` n'atteint pas (il faut tous les joueurs prêts au même moment).
//!
//!   fuzz_porte <roms> <eeprom> <scénario> <chauffe> <essais>
//!
//! Chaque essai part de l'état du jeu après la chauffe, pose une porte de variante tirée, déjà initialisée,
//! dans l'état 2 avec tous les joueurs prêts (genre de sortie `+0x48` tiré, 0 à 3), ou dans l'état 4 (retour
//! de zone, champs mémorisés tirés). Le code d'origine s'exécute seul (interpréteur) jusqu'à son retour ou
//! jusqu'à l'appel du fondu 0x1460 ; la native de même, arrêtée à l'entrée du fondu. Les deux mémoires sont
//! comparées (hors table des tâches et piles), puis chacun reprend après le fondu (le fondu, déjà natif,
//! n'est pas rejoué) : l'original à son adresse de retour, la native par sa native de reprise. Les mémoires
//! finales sont comparées.

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;

use cps2::Cps2Bus;
use ddsom_game::mem::{rd32, wr16, wr32, wr8, A5};
use m68k::Cpu;

const ENTREE: u32 = 0xcb29c;
const FONDU: u32 = 0x1460;
const SENTINELLE: u32 = 0xfffe;
/// Table des tâches FF0064 et piles : propres à chaque machine.
const IGNOREE: std::ops::Range<u32> = 0xff_0000..0xff_0e66;

struct Alea(u64);

impl Alea {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn parmi<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.next() as usize % v.len()]
    }
}

/// Arrêt de la native à l'entrée du fondu.
struct Fondu;

fn crochet(pc: u32, entree: bool) {
    if entree && pc == FONDU {
        std::panic::resume_unwind(Box::new(Fondu));
    }
}

/// Code d'origine seul jusqu'au retour à la sentinelle, ou jusqu'à l'entrée du fondu. Vrai si le fondu est atteint.
fn origine(c: &mut Cpu, b: &mut Cps2Bus) -> Option<bool> {
    for _ in 0..2_000_000 {
        if c.pc == SENTINELLE {
            return Some(false);
        }
        if c.pc == FONDU {
            return Some(true);
        }
        c.step(b);
    }
    None
}

fn appelle(c: &mut Cpu, b: &mut Cps2Bus, adresse: u32) {
    c.a[7] = c.a[7].wrapping_sub(4);
    wr32(b, c.a[7], SENTINELLE);
    c.pc = adresse;
}

/// Écarts entre les deux mémoires (hors zone ignorée), et entre A0 / D7.
fn ecarts(c1: &Cpu, b1: &Cps2Bus, c2: &Cpu, b2: &Cps2Bus) -> Vec<String> {
    let mut d = Vec::new();
    ddsom_game::mem::diff_bus(b1, b2, IGNOREE, &mut d);
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
    let [_, roms, eeprom, scenario, chauffe, essais, ..] = &a[..] else {
        eprintln!("usage : fuzz_porte <roms> <eeprom> <scénario> <chauffe> <essais>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    m.set_overclock(16.0);
    ddsom_game::install(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    for f in 0..chauffe.parse::<usize>().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    ddsom_game::set_trace(|_| {});
    ddsom_game::set_native_hook(crochet);
    if std::env::var_os("DDSOM_PANIQUES").is_none() {
        std::panic::set_hook(Box::new(|_| {}));
    }
    let mut alea = Alea(0x2545_f491_4f6c_dd1d);
    let (mut bilan, mut ecarts_vus, mut fondus, mut abandons) = (0u32, 0u32, 0u32, 0u32);
    for essai in 0..essais.parse::<u32>().unwrap() {
        let (mut c, mut b) = (m.cpu.clone(), m.bus.clone());
        b.irq_horizon = u64::MAX;
        let porte = ddsom_game::objects::pools::SET.slot(alea.next() % 32).0;
        for off in (0..0x50).step_by(2) {
            wr16(&mut b, porte + off, 0);
        }
        wr8(&mut b, porte, 1);
        wr8(&mut b, porte + 2, 7);
        wr8(&mut b, porte + 3, (alea.next() % 0x38) as u8);
        wr8(&mut b, porte + 0x28, 0x10);
        // initialisation (état 0) par la native, sur la copie : champs de la table des portes
        c.a[0] = porte;
        c.a[5] = A5;
        for (off, v) in [(0x3d, 0), (0x50, 0), (0x42, 0)] {
            wr8(&mut b, A5 + off, v);
        }
        wr16(&mut b, A5 + 0x52, 0);
        ddsom_game::call(&mut c, &mut b, ENTREE);
        let etat4 = alea.next() % 3 == 0;
        if etat4 {
            wr8(&mut b, porte + 4, 4);
            for off in [0x30u32, 0x34, 0x38, 0x3c, 0x4a] {
                wr32(&mut b, porte + off, alea.next() << 4);
            }
            for off in [0x18u32, 0x22, 0x14, 0x1c] {
                wr16(&mut b, porte + off, alea.next() as u16 & 0x3ff);
            }
            for off in [0x49u32, 0x25, 0x26] {
                wr8(&mut b, porte + off, alea.parmi(&[0, 1, 2]));
            }
            for j in 0..4u32 {
                let p = 0xff_85de + 0x200 * j;
                wr8(&mut b, p + 4, alea.parmi(&[0, 2, 2, 4]));
            }
        } else {
            wr8(&mut b, porte + 4, 2);
            wr8(&mut b, porte + 0x48, alea.parmi(&[0, 0, 1, 1, 2, 3]));
            wr8(&mut b, porte + 0x17, 0xf);
            wr8(&mut b, porte + 0x1a, 0xf);
            wr8(&mut b, porte + 0x16, alea.parmi(&[4, 4, 1, 0]));
            wr8(&mut b, porte + 0x24, alea.next() as u8 & 3);
            wr8(&mut b, A5 + 0x35, alea.parmi(&[1, 2, 3, 4]));
            wr16(&mut b, porte + 0x20, 5);
        }
        wr16(&mut b, A5 + 0x52, if alea.next() % 2 == 0 { 0 } else { porte as u16 });
        // quelques objets des autres pools suspendus ou non (réveil, libération)
        for pool in ddsom_game::objects::pools::POOLS {
            for i in 0..pool.slots {
                let o = pool.slot(i).0;
                if ddsom_game::mem::rd8(&mut b, o) != 0 && alea.next() % 3 == 0 {
                    wr8(&mut b, o + 0x40, alea.parmi(&[0, 1, 2]));
                }
            }
        }
        c.a[0] = porte;
        c.a[5] = A5;
        c.d[7] = alea.next() & 0x1f;
        c.d[0] = 7 * 4;
        c.a[1] = ENTREE;
        let (mut c2, mut b2) = (c.clone(), b.clone());
        // l'original
        appelle(&mut c2, &mut b2, ENTREE);
        let Some(fondu) = origine(&mut c2, &mut b2) else {
            abandons += 1;
            continue;
        };
        // la native
        let r = catch_unwind(AssertUnwindSafe(|| ddsom_game::call(&mut c, &mut b, ENTREE)));
        let fondu_natif = match r {
            Ok(()) => false,
            Err(e) if e.is::<Fondu>() => true,
            Err(_) => {
                println!("essai {essai} : panique de la native");
                ecarts_vus += 1;
                continue;
            }
        };
        bilan += 1;
        if fondu != fondu_natif {
            println!("essai {essai} : fondu {fondu} (original) / {fondu_natif} (native)");
            ecarts_vus += 1;
            continue;
        }
        let d = ecarts(&c, &b, &c2, &b2);
        if !d.is_empty() {
            println!("essai {essai} état4={etat4} variante {:02x} : avant le fondu {}", ddsom_game::mem::rd8(&mut b2, porte + 3), d.join(", "));
            ecarts_vus += 1;
            continue;
        }
        if !fondu {
            continue;
        }
        fondus += 1;
        // reprise après le fondu : adresse de retour de l'appel, au sommet de la pile de chacun
        let retour = rd32(&mut b2, c2.a[7]);
        c2.a[7] = c2.a[7].wrapping_add(4);
        c2.pc = retour;
        let retour_natif = rd32(&mut b, c.a[7]);
        if retour != retour_natif {
            println!("essai {essai} : reprise en {retour_natif:06x} au lieu de {retour:06x}");
            ecarts_vus += 1;
            continue;
        }
        let a0 = c.a[0];
        let r = catch_unwind(AssertUnwindSafe(|| {
            c.a[0] = a0;
            ddsom_game::call(&mut c, &mut b, retour)
        }));
        if r.is_err() || origine(&mut c2, &mut b2) != Some(false) {
            println!("essai {essai} : reprise interrompue");
            ecarts_vus += 1;
            continue;
        }
        let d = ecarts(&c, &b, &c2, &b2);
        if !d.is_empty() {
            println!("essai {essai} état4={etat4} variante {:02x} : après le fondu {}", ddsom_game::mem::rd8(&mut b2, porte + 3), d.join(", "));
            ecarts_vus += 1;
        }
    }
    println!("{bilan} essais comparés, {fondus} avec fondu, {abandons} abandons, {ecarts_vus} écarts");
    println!("{}", if ecarts_vus == 0 { "FUZZ OK" } else { "FUZZ ÉCARTS" });
}
