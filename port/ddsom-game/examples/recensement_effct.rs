//! Recensement du code d'origine encore atteignable depuis les gestionnaires EFFCT (table `0x39ce0`).
//!
//! Pour chaque type dont le gestionnaire est natif, on crée dans le pool EFFCT un objet du type (variante
//! et états tirés dans une grille, propriétaire = un joueur ou un ennemi actif), puis on appelle le
//! gestionnaire image après image sur la machine hybride, en relevant chaque instruction 68000 exécutée
//! pendant ces appels. Sortie : par type et variante, les sites Rust qui exécutent encore du 68000
//! (`fichier:ligne routine`), avec un exemple d'état ; puis les adresses de la zone EFFCT exécutées.
//!
//!   recensement_effct <roms> <eeprom> <scénario> <images de chauffe> [types séparés par des virgules]
//!
//! `RECENS_VARIANTES=n` (16 par défaut), `RECENS_PAS=n` (pas par trajectoire, 40), `RECENS_TOURS=n` (8).
//! Les combinaisons hors des tables de l'original (variante ou état que la table ne prévoit pas) font
//! sauter le code d'origine dans des données : elles apparaissent aussi, à trier à la main.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ddsom_game::mem::{rd16, rd8, wr16, wr8, A5};
use ddsom_game::objects::pools::{ENEMY, EFFCT};

/// Zone du code des gestionnaires EFFCT.
const ZONE: std::ops::Range<u32> = 0xa8594..0xba310;
/// Instructions 68000 au plus par appel (au-delà, l'original est sorti des rails).
const BUDGET: u64 = 100_000;

thread_local! {
    static PCS: RefCell<BTreeSet<u32>> = RefCell::new(BTreeSet::new());
    static SITES: RefCell<BTreeSet<(String, u32)>> = RefCell::new(BTreeSet::new());
    static COMPTE: RefCell<u64> = const { RefCell::new(0) };
}

fn trace(pc: u32) {
    let n = COMPTE.with(|c| {
        *c.borrow_mut() += 1;
        *c.borrow()
    });
    if n > BUDGET {
        std::panic::panic_any("budget");
    }
    if ZONE.contains(&pc) {
        PCS.with(|p| p.borrow_mut().insert(pc));
    }
    let site = ddsom_game::site_d_appel().map_or(("hors appel".to_string(), 0), |(l, r)| (format!("{}:{}", l.file().rsplit('/').next().unwrap(), l.line()), r));
    SITES.with(|s| s.borrow_mut().insert(site));
}

struct Alea(u64);
impl Alea {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
    fn pick<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.next() as usize % v.len()]
    }
}

fn main() {
    // les appels imbriqués (natives, code d'origine) consomment beaucoup de pile
    std::thread::Builder::new().stack_size(1 << 30).spawn(principal).unwrap().join().unwrap();
}

fn principal() {
    let a: Vec<String> = std::env::args().collect();
    let (Some(roms), Some(eeprom), Some(scenario), Some(chauffe)) = (a.get(1), a.get(2), a.get(3), a.get(4)) else {
        eprintln!("usage : recensement_effct <roms> <eeprom> <scénario> <images de chauffe> [types]");
        std::process::exit(2);
    };
    let env = |k: &str, d: u32| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    let nb_variantes = env("RECENS_VARIANTES", 16);
    let pas = env("RECENS_PAS", 40);
    let tours = env("RECENS_TOURS", 8);
    let voulus: Option<Vec<u32>> = a.get(5).map(|s| s.split(',').map(|x| x.parse().unwrap()).collect());
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    m.set_overclock(16.0);
    ddsom_game::install(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    for f in 0..chauffe.parse::<usize>().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    ddsom_game::set_trace(trace);
    std::panic::set_hook(Box::new(|_| {}));
    let (c0, b0) = (m.cpu.clone(), m.bus.clone());
    let mut alea = Alea(0x2545_f491_4f6c_dd1d);
    let natives: BTreeSet<u32> = ddsom_game::all_natives().map(|n| n.entry).collect();
    for t in 0..92u32 {
        if voulus.as_ref().is_some_and(|v| !v.contains(&t)) {
            continue;
        }
        let entree = {
            let mut b = b0.clone();
            (m68k::Bus::read_prog16(&mut b, 0x39ce0 + 4 * t) as u32) << 16 | m68k::Bus::read_prog16(&mut b, 0x39ce2 + 4 * t) as u32
        };
        if !natives.contains(&entree) {
            println!("TYPE {t} {entree:06x} : pas de native");
            continue;
        }
        // (variante, site, routine) -> (nombre de trajectoires, exemple)
        let mut releve: BTreeMap<(u32, String, u32), (u32, String)> = BTreeMap::new();
        let mut zone: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
        let mut paniques = 0;
        for v in 0..nb_variantes {
            for tour in 0..tours {
                for s5 in [0u8, 2, 4, 6, 8, 0xa, 0xc, 0xe] {
                    let (mut c, mut b) = (c0.clone(), b0.clone());
                    b.irq_horizon = u64::MAX;
                    let mut proprietaires: Vec<u32> = ENEMY.iter().filter(|o| rd8(&mut b, o.0) != 0).map(|o| o.0).collect();
                    proprietaires.extend((0..4).map(|j| 0xff85de + 0x200 * j));
                    let obj = EFFCT.slot(alea.next() % EFFCT.slots).0;
                    for off in (0..EFFCT.stride).step_by(2) {
                        wr16(&mut b, obj + off, 0);
                    }
                    let owner = alea.pick(&proprietaires);
                    wr8(&mut b, obj, 1);
                    wr8(&mut b, obj + 2, t as u8);
                    wr8(&mut b, obj + 3, v as u8);
                    // premier tour : naissance (état 0) ; suivants : état actif, sous-état de la grille
                    // une trajectoire sur deux part de la naissance (état 0), les autres de l'état actif
                    let naissance = tour % 2 == 0;
                    let s4 = if naissance { 0 } else { 2 };
                    wr8(&mut b, obj + 4, s4);
                    wr8(&mut b, obj + 5, if naissance { 0 } else { s5 });
                    wr8(&mut b, obj + 6, if !naissance && tour >= 2 { alea.pick(&[0, 2, 4, 6]) } else { 0 });
                    wr8(&mut b, obj + 0x28, EFFCT.class);
                    for off in [0x46u32, 0x50, 0x54, 0x5a, 0x5c, 0x1e] {
                        wr16(&mut b, obj + off, owner as u16);
                    }
                    for off in [0x8u32, 0xc, 0x10] {
                        let w = rd16(&mut b, owner + off);
                        wr16(&mut b, obj + off, w);
                    }
                    if naissance && s5 != 0 {
                        // naissance avec des champs de création variés
                        for off in [0x2fu32, 0x30, 0x31, 0x42, 0x43, 0x58, 0x59] {
                            wr8(&mut b, obj + off, alea.pick(&[0, 1, 2, 3, 4, 8]));
                        }
                    }
                    if alea.next() % 2 == 0 {
                        // action et compteurs du propriétaire, lus par les bulles et les effets liés
                        wr8(&mut b, owner + 0x3d, 2 * (alea.next() % 0x1a) as u8);
                        for off in [0x10du32, 0x10e, 0x77, 0x86, 0x43] {
                            wr8(&mut b, owner + off, alea.pick(&[0, 1, 2, 3, 4, 5]));
                        }
                    }
                    wr16(&mut b, A5 + EFFCT.free, (EFFCT.slots / 2) as u16);
                    let depart = format!("v{v:x} {s4:x}/{:x}/{:x}", rd8(&mut b, obj + 5), rd8(&mut b, obj + 6));
                    for k in 0..pas {
                        if rd8(&mut b, obj) == 0 || rd8(&mut b, obj + 2) != t as u8 {
                            break;
                        }
                        if k > 0 && alea.next() % 4 == 0 {
                            // le propriétaire et le monde bougent
                            wr8(&mut b, owner + 4, alea.pick(&[0, 2, 4, 6]));
                            wr8(&mut b, owner + 0x3d, (alea.next() % 0x30) as u8);
                            wr8(&mut b, obj + 0x33, alea.pick(&[0, 1]));
                            wr8(&mut b, obj + 0x57, alea.pick(&[0, 8, 0x20]));
                        }
                        c.a[0] = obj;
                        c.a[5] = A5;
                        c.d[7] = k;
                        c.d[0] = 4 * t;
                        c.a[1] = entree;
                        PCS.with(|p| p.borrow_mut().clear());
                        SITES.with(|s| s.borrow_mut().clear());
                        COMPTE.with(|x| *x.borrow_mut() = 0);
                        let etat = format!("{:x}/{:x}/{:x}", rd8(&mut b, obj + 4), rd8(&mut b, obj + 5), rd8(&mut b, obj + 6));
                        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ddsom_game::call(&mut c, &mut b, entree)));
                        let sites = SITES.with(|s| std::mem::take(&mut *s.borrow_mut()));
                        let pcs = PCS.with(|p| std::mem::take(&mut *p.borrow_mut()));
                        // appel suspect : l'original est sorti des rails (exception, adresse impaire, budget épuisé)
                        let suspect = r.is_err() || sites.iter().any(|(f, a)| f.starts_with("demarrage") || a & 1 != 0);
                        for (site, routine) in sites {
                            let site = if suspect { format!("? {site}") } else { site };
                            let e = releve.entry((v, site, routine)).or_insert((0, format!("départ {depart} pas {k} état {etat}")));
                            e.0 += 1;
                        }
                        zone.entry(v).or_default().extend(pcs);
                        if r.is_err() {
                            paniques += 1;
                            break;
                        }
                    }
                }
            }
        }
        println!("TYPE {t} {entree:06x} : {paniques} trajectoires interrompues");
        for ((v, site, routine), (n, ex)) in &releve {
            println!("  v{v:x} {site} {routine:06x} ×{n} ({ex})");
        }
        for (v, pcs) in &zone {
            if pcs.is_empty() {
                continue;
            }
            // plages contiguës
            let mut plages: Vec<(u32, u32)> = Vec::new();
            for &p in pcs {
                match plages.last_mut() {
                    Some(r) if p - r.1 <= 0x10 => r.1 = p,
                    _ => plages.push((p, p)),
                }
            }
            let s: Vec<String> = plages.iter().map(|(a, b)| format!("{a:06x}-{b:06x}")).collect();
            println!("  ZONE v{v:x} : {}", s.join(" "));
        }
    }
}
