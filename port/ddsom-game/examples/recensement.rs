//! Recensement des gestionnaires de types des familles d'objets : pour chaque entrée des tables
//! d'aiguillage (type → gestionnaire), dit si une routine native la couvre, puis l'exerce sur des
//! objets synthétiques et compte le code 68000 que la native exécute encore (sous-états délégués,
//! sous-routines d'origine), les appels hors contrat (l'original s'exécute à la place) et les cessions.
//!
//!   recensement <roms> <eeprom> <scénario> <chauffe> <trajectoires> [CLASSE,...]
//!
//! Classes : PSHL PEFF ESHL SET SOBJ ITEM BOX (toutes par défaut). `DDSOM_ENTREE=address` : un seul
//! gestionnaire ; `DDSOM_ETATS=1` : la moitié des trajectoires part d'états `+4..+7` tirés. Une trajectoire crée un objet neuf (état 0, variante 0, ou tirée parmi les `DDSOM_VARIANTES` premières)
//! puis appelle le gestionnaire jusqu'à 120 fois en perturbant les entrées qu'il lit ; une sur deux
//! tire en plus des variables de scène (partie, transition, mode de jeu ; marquée « tirée »).
//! Sortie : une ligne par entrée, puis les plages de 68000 exécutées par entrée.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use ddsom_game::mem::{rd8, wr16, wr32, wr8, A5};
use ddsom_game::objects::pools::{self, Pool};
use m68k::decode::Fetch;

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

/// (nom, pool, table, nombre d'entrées) : la table s'arrête à la boucle de dispatch suivante.
const CLASSES: &[(&str, &Pool, u32, u32)] = &[
    ("PSHL", &pools::PSHL, 0x39908, (0x3996c - 0x39908) / 4),
    ("PEFF", &pools::PEFF, 0x39992, (0x39a52 - 0x39992) / 4),
    ("ESHL", &pools::ESHL, 0x39b96, (0x39cba - 0x39b96) / 4),
    ("BOX", &pools::BOX, 0x39e76, 4),
    ("SET", &pools::SET, 0x39eac, 34),
    ("ITEM", &pools::ITEM, 0x39f5a, 10),
    ("SOBJ", &pools::SOBJ, 0x39fa8, 16),
];

#[derive(Default)]
struct Summary {
    calls: u64,
    calls_68k: u64,
    instructions: u64,
    outside_contract: u64,
    cessions: u64,
    panics: BTreeSet<String>,
    /// adresses 68000 exécutées : (trajectoire légitime, états tirés)
    pcs: BTreeMap<u32, (u64, u64)>,
}

static PROGRESS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

thread_local! {
    static ACTIVE: RefCell<bool> = const { RefCell::new(false) };
    static SEEN: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    static DEPTH: RefCell<u32> = const { RefCell::new(0) };
    static CALLED_NATIVES: RefCell<u32> = const { RefCell::new(0) };
}

/// Profondeur des natives imbriquées : un objet synthétique incohérent peut faire sauter le code à
/// l'adresse 0 (redémarrage), qui se rappelle sans fin.
fn hook(_pc: u32, entry: bool) {
    let total = CALLED_NATIVES.with(|n| {
        *n.borrow_mut() += 1;
        *n.borrow()
    });
    if total > 2_000_000 {
        CALLED_NATIVES.with(|n| *n.borrow_mut() = 0);
        panic!("boucle de natives sans fin");
    }
    let p = DEPTH.with(|p| {
        let mut p = p.borrow_mut();
        if entry { *p += 1 } else { *p = p.saturating_sub(1) }
        *p
    });
    if p > 400 && entry {
        DEPTH.with(|p| *p.borrow_mut() = 0);
        CALLED_NATIVES.with(|n| *n.borrow_mut() = 0);
        panic!("natives imbriquées sans fin (saut hors du code)");
    }
}

fn trace(pc: u32) {
    if ACTIVE.with(|a| *a.borrow()) {
        let n = SEEN.with(|v| {
            let mut v = v.borrow_mut();
            v.push(pc);
            v.len()
        });
        if n > 300_000 {
            ACTIVE.with(|a| *a.borrow_mut() = false);
            panic!("boucle 68000 sans fin");
        }
    }
}

/// Plages contiguës (à moins de 0x10 octets) d'un ensemble d'adresses.
fn ranges(pcs: impl Iterator<Item = u32>) -> Vec<(u32, u32)> {
    let mut out: Vec<(u32, u32)> = Vec::new();
    for pc in pcs {
        match out.last_mut() {
            Some((_, fin)) if pc <= *fin + 0x10 => *fin = pc,
            _ => out.push((pc, pc)),
        }
    }
    out
}

fn main() {
    // les natives imbriquées (appels de code d'origine qui rappellent des natives) demandent une grande pile
    // chien de garde : une trajectoire bloquée plus d'une minute arrête tout (diagnostic sous gdb)
    std::thread::spawn(|| loop {
        let before = PROGRESS.load(std::sync::atomic::Ordering::Relaxed);
        std::thread::sleep(std::time::Duration::from_secs(60));
        if PROGRESS.load(std::sync::atomic::Ordering::Relaxed) == before {
            eprintln!("trajectoire bloquée");
            std::process::abort();
        }
    });
    std::thread::Builder::new().stack_size(1 << 30).spawn(principal).unwrap().join().unwrap();
}

fn principal() {
    let a: Vec<String> = std::env::args().collect();
    let [_, roms, eeprom, scenario, warmup, tours, ..] = &a[..] else {
        eprintln!("usage : recensement <roms> <eeprom> <scénario> <chauffe> <trajectoires> [CLASSE,...]");
        std::process::exit(2);
    };
    let wanted: Option<Vec<&str>> = a.get(6).map(|s| s.split(',').collect());
    let filter: Option<u32> = std::env::var("DDSOM_ENTREE").ok().map(|s| u32::from_str_radix(s.trim_start_matches("0x"), 16).unwrap());
    // DDSOM_SAUTER=adresse,... : gestionnaires non exercés (objets synthétiques qui les font boucler)
    let jump: Vec<u32> = std::env::var("DDSOM_SAUTER")
        .map(|s| s.split(',').filter_map(|x| u32::from_str_radix(x.trim_start_matches("0x"), 16).ok()).collect())
        .unwrap_or_default();
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    m.set_overclock(16.0);
    ddsom_game::install(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    for f in 0..warmup.parse::<usize>().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    ddsom_game::set_trace(trace);
    ddsom_game::set_native_hook(hook);
    if std::env::var_os("DDSOM_PANIQUES").is_none() {
        std::panic::set_hook(Box::new(|_| {}));
    }
    let tours: u32 = tours.parse().unwrap();
    let mut rng = Rng(0x1234_5678_9abc_def1);
    let (c0, b0) = (m.cpu.clone(), m.bus.clone());
    let mut b = b0.clone();
    let mut summaries: BTreeMap<u32, Summary> = BTreeMap::new();
    let mut lines = Vec::new();
    for &(name, pool, table, n) in CLASSES {
        if wanted.as_ref().is_some_and(|v| !v.contains(&name)) {
            continue;
        }
        let mut views = BTreeSet::new();
        for kind in 0..n {
            let entry = (b.fetch16(table + 4 * kind) as u32) << 16 | b.fetch16(table + 4 * kind + 2) as u32;
            let native = ddsom_game::all_natives().find(|nat| nat.entry == entry);
            let duplicate = !views.insert(entry);
            lines.push((name, kind, entry, native.map(|n| n.name), duplicate));
            if native.is_none() || duplicate || filter.is_some_and(|f| f != entry) || jump.contains(&entry) {
                continue;
            }
            if std::env::var_os("DDSOM_VERBOSE").is_some() {
                eprintln!("{name} {kind} {entry:06x}");
            }
            let summary = summaries.entry(entry).or_default();
            for t in 0..tours {
                let tires = t % 2 == 1;
                trajectory(&c0, &b0, pool, kind as u8, entry, tires, &mut rng, summary);
            }
        }
    }
    println!("# entrées des tables d'aiguillage");
    println!("classe type adresse native appels appels_68k instr_68k hors_contrat cessions");
    for &(name, kind, entry, native, duplicate) in &lines {
        let b = summaries.get(&entry);
        let detail = match (native, b) {
            (None, _) => "68000".to_string(),
            (Some(_), _) if duplicate => "(même gestionnaire qu'un type précédent)".to_string(),
            (Some(n), Some(b)) => format!(
                "{n} {} {} {} {} {}{}",
                b.calls,
                b.calls_68k,
                b.instructions,
                b.outside_contract,
                b.cessions,
                if b.panics.is_empty() { String::new() } else { format!(" paniques: {:?}", b.panics) }
            ),
            (Some(n), None) => format!("{n} (non exercé)"),
        };
        println!("{name} {kind} {entry:06x} {detail}");
    }
    println!("# 68000 exécuté par les natives (plage : légitime/états tirés)");
    for (entry, b) in &summaries {
        if b.pcs.is_empty() {
            continue;
        }
        let list: Vec<String> = ranges(b.pcs.keys().copied())
            .into_iter()
            .map(|(d, f)| {
                let (l, t) = b.pcs.range(d..=f).fold((0, 0), |(l, t), (_, &(x, y))| (l + x, t + y));
                format!("{d:06x}-{f:06x}:{l}/{t}")
            })
            .collect();
        println!("{entry:06x} {}", list.join(" "));
    }
}

#[allow(clippy::too_many_arguments)]
fn trajectory(
    c0: &m68k::Cpu,
    b0: &cps2::Cps2Bus,
    pool: &Pool,
    kind: u8,
    entry: u32,
    tires: bool,
    rng: &mut Rng,
    summary: &mut Summary,
) {
    let (mut c, mut b) = (c0.clone(), b0.clone());
    b.irq_horizon = u64::MAX;
    let mut owners: Vec<u32> = pools::ENEMY.iter().filter(|o| rd8(&mut b, o.0) != 0).map(|o| o.0).collect();
    owners.extend((0..4).map(|j| 0xff85de + 0x200 * j));
    let player = 0xff85de + 0x200 * (rng.next() % 4);
    let owner = if pool.class == 6 || pool.class == 0x12 { player } else { rng.among(&owners) };
    let obj = pool.slot(rng.next() % pool.slots);
    for off in (0..pool.stride).step_by(2) {
        wr16(&mut b, obj.0 + off, 0);
    }
    wr8(&mut b, obj.0, 1);
    wr8(&mut b, obj.0 + 2, kind);
    // variante : 0 par défaut (au-delà de sa table, un type lit n'importe quoi et peut boucler sans fin)
    let variants: u32 = std::env::var("DDSOM_VARIANTES").ok().and_then(|v| v.parse().ok()).unwrap_or(1);
    wr8(&mut b, obj.0 + 3, (rng.next() % variants) as u8);
    wr8(&mut b, obj.0 + 0x28, pool.class);
    wr8(&mut b, obj.0 + 0x2c, (rng.next() & 1) as u8);
    for off in [0x50u32, 0x46, 0x5a, 0x5c, 0x42, 0x54, 0x1e] {
        if rng.next() % 4 != 0 {
            wr16(&mut b, obj.0 + off, owner as u16);
        }
    }
    for off in [0x8u32, 0xc, 0x10] {
        let v = (ddsom_game::mem::rd16(&mut b, owner + off) as u32) << 16 | (rng.next() & 0xffff);
        wr32(&mut b, obj.0 + off, v);
    }
    wr16(&mut b, obj.0 + 0xc, (rng.next() % 0x60) as u16);
    wr16(&mut b, A5 + pool.free, (pool.slots / 2) as u16);
    if std::env::var_os("DDSOM_ETATS").is_some() && rng.next() % 2 == 0 {
        // états de départ tirés (paires) : atteint les sous-états délégués ; peut sortir des tables de l'original
        for off in [4u32, 5, 6, 7] {
            wr8(&mut b, obj.0 + off, rng.among(&[0, 0, 2, 4, 6, 8, 0xa]));
        }
    }
    if tires {
        // variables de scène que lisent les décors et objets système : partie, transition, mode, joueurs
        for (off, vals) in [(0x35u32, &[0u8, 1, 2, 4][..]), (0x36, &[0, 1]), (0x42, &[0, 1]), (0x44, &[0, 1]), (0x50, &[0, 1]), (0x4a, &[0, 1]), (0x2c, &[0, 4, 8, 12])] {
            wr8(&mut b, A5 + off, rng.among(vals));
        }
    }
    for step in 0..120u32 {
        if rd8(&mut b, obj.0) == 0 {
            break;
        }
        if step > 0 {
            if rng.next() % 6 == 0 {
                wr8(&mut b, obj.0 + 0x57, rng.among(&[0x08, 0x20, 0x28, 0x10, 0x02, 0x40]));
            }
            if rng.next() % 8 == 0 {
                wr8(&mut b, obj.0 + 0x33, rng.next() as u8 & 1);
            }
            if rng.next() % 10 == 0 {
                wr8(&mut b, owner + 4, (rng.next() % 8) as u8);
            }
            if rng.next() % 5 == 0 {
                for off in [0x53u32, 0xa0, 0xa1, 0xa8, 0xa9, 0x3e, 0x2f, 0x42, 0x5d] {
                    if rng.next() % 3 == 0 {
                        wr8(&mut b, owner + off, rng.among(&[0, 1, 2, 3, 0xff]));
                    }
                }
            }
        }
        wr8(&mut b, A5 + 0x3d, (rng.next() % 10 == 0) as u8);
        wr8(&mut b, obj.0 + 0x40, (rng.next() % 40 == 0) as u8);
        c.a[0] = obj.0;
        c.a[5] = A5;
        c.d[7] = step;
        c.d[0] = 4 * kind as u32;
        c.a[1] = entry;
        summary.calls += 1;
        PROGRESS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        SEEN.with(|v| v.borrow_mut().clear());
        ACTIVE.with(|a| *a.borrow_mut() = true);
        DEPTH.with(|p| *p.borrow_mut() = 0);
        CALLED_NATIVES.with(|n| *n.borrow_mut() = 0);
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ddsom_game::call(&mut c, &mut b, entry)));
        ACTIVE.with(|a| *a.borrow_mut() = false);
        let seen = SEEN.with(|v| std::mem::take(&mut *v.borrow_mut()));
        if !seen.is_empty() {
            summary.calls_68k += 1;
            summary.instructions += seen.len() as u64;
            for pc in seen {
                let e = summary.pcs.entry(pc).or_default();
                if tires {
                    e.1 += 1;
                } else {
                    e.0 += 1;
                }
            }
        }
        match r {
            Ok(()) => {}
            // une cession dans un appel par adresse arrête `call` (panique)
            Err(e) if e.downcast_ref::<String>().is_some_and(|m| m.contains("a cédé la main")) => {
                summary.cessions += 1;
                break;
            }
            Err(e) => {
                let msg = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()));
                summary.panics.insert(msg.unwrap_or_default().chars().take(80).collect());
                break;
            }
        }
    }
}
