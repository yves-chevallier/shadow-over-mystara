//! Fuzz différentiel des gestionnaires EFFCT (table `0x39ce0`) sur des trajectoires complètes.
//!
//! Pour chaque type demandé, un objet du type est créé dans le pool EFFCT (variante tirée, naissance à
//! l'état 0 ou état actif de la grille, propriétaire = joueur ou ennemi actif, champs de liaison sur le
//! propriétaire) puis le gestionnaire est appelé image après image, avec des perturbations du propriétaire,
//! des joueurs et de la scène. Chaque appel de routine native est comparé à l'original (mode contrôle de
//! `ddsom_game`, sous-routines natives comprises). La couverture du code d'origine du gestionnaire (exécuté à
//! part sur une copie) est relevée : en fin de course, les instructions du gestionnaire jamais exécutées
//! par l'original sont listées (chemins non validés).
//!
//!   fuzz_effct <roms> <eeprom> <scénario> <images de chauffe> <types séparés par des virgules>
//!
//! Variables : `FUZZ_CREATEURS=b4bcc,b96c0` (créateurs essayés depuis des registres tirés au hasard),
//! `FUZZ_VARIANTES=0,2,5` (variantes tirées ; par défaut 0 à 15), `FUZZ_TOURS=n` (trajectoires par
//! variante, 24), `FUZZ_PAS=n` (pas par trajectoire, 60), `FUZZ_GRAINE=n`, `FUZZ_PLAGE=début-fin` (plage du
//! code d'origine dont on relève la couverture ; par défaut de l'entrée à l'entrée suivante de la table).

use std::collections::BTreeSet;
use std::path::Path;

use cps2::Cps2Bus;
use m68k::decode::{Ea, Fetch, Op};
use m68k::Cpu;

use ddsom_game::mem::{rd16, rd8, wr16, wr32, wr8, A5};
use ddsom_game::objects::pools::{EFFCT, ENEMY};

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
    fn une_sur(&mut self, n: u32) -> bool {
        self.next() % n == 0
    }
}

const JOUEURS: u32 = 0xff85de;

/// Instructions atteignables depuis `entree` dans `plage` (branchements, `bsr` locaux, tables de sauts dont la
/// fin est le début du premier code qu'elles désignent).
fn atteignables(b: &mut Cps2Bus, entree: u32, plage: std::ops::Range<u32>) -> BTreeSet<u32> {
    let mut vus = BTreeSet::new();
    let mut todo = vec![entree];
    let dedans = |t: u32| plage.contains(&t) && t % 2 == 0;
    while let Some(depart) = todo.pop() {
        let mut pc = depart;
        while dedans(pc) && !vus.contains(&pc) {
            let ins = m68k::decode(b, pc);
            vus.insert(pc);
            let suite = pc + ins.len.max(2);
            let mut continue_ = true;
            match ins.op {
                Op::Bra(t) => {
                    todo.push(t);
                    continue_ = false;
                }
                Op::Bcc(_, t) | Op::Bsr(t) | Op::Dbcc(_, _, t) => todo.push(t),
                Op::Rts | Op::Rte | Op::Rtr | Op::Illegal => continue_ = false,
                Op::Jmp(ea) | Op::Jsr(ea) => {
                    match ea {
                        Ea::AbsL(t) | Ea::AbsW(t) | Ea::PcDisp(t) => todo.push(t),
                        Ea::PcIdx(base, _) => {
                            let mut fin = plage.end;
                            let mut at = base;
                            while at < fin && at < base + 0x200 {
                                let t = (base as i32 + b.fetch16(at) as i16 as i32) as u32;
                                if dedans(t) && t > at {
                                    fin = fin.min(t);
                                    todo.push(t);
                                }
                                at += 2;
                            }
                        }
                        _ => {}
                    }
                    if matches!(ins.op, Op::Jmp(_)) {
                        continue_ = false;
                    }
                }
                _ => {}
            }
            if !continue_ {
                break;
            }
            pc = suite;
        }
    }
    vus
}

/// Exécute le code d'origine seul sur une copie et rend les adresses exécutées dans `plage`.
fn couverture(c: &Cpu, b: &Cps2Bus, entree: u32, plage: &std::ops::Range<u32>, vus: &mut BTreeSet<u32>) {
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (mut c, mut b) = (c.clone(), b.clone());
        c.pc = entree;
        c.a[7] = c.a[7].wrapping_sub(4);
        wr32(&mut b, c.a[7], 0xfffe);
        let mut local = Vec::new();
        for _ in 0..200_000 {
            if c.pc == 0xfffe {
                break;
            }
            if plage.contains(&c.pc) {
                local.push(c.pc);
            }
            c.step(&mut b);
        }
        local
    }));
    if let Ok(l) = r {
        vus.extend(l);
    }
}

/// Sous-états (`+5`) de l'état actif que la table de l'original prévoit (au-delà, il saute dans des données).
fn sous_etats(t: u32) -> &'static [u8] {
    match t {
        63 | 43 => &[0, 2, 4],
        _ => &[0],
    }
}

/// Réglages propres à un type, après la création générique de l'objet.
fn prepare(t: u32, b: &mut Cps2Bus, obj: u32, owner: u32, alea: &mut Alea) {
    let joueur = JOUEURS + 0x200 * (alea.next() % 4);
    match t {
        // effets liés à un joueur : le propriétaire est un joueur
        81 | 80 => {
            wr16(b, obj + 0x50, joueur as u16);
            for off in [0x8u32, 0xc, 0x10] {
                let w = rd16(b, joueur + off);
                wr16(b, obj + off, w);
            }
            wr16(b, joueur + 0x42, (alea.next() % 6) as u16);
        }
        // créateur en +0x42
        63 => wr16(b, obj + 0x42, owner as u16),
        // objet suivi en +0x5c (parfois aucun), réglage 0 ou 1
        43 => {
            wr16(b, obj + 0x5c, if alea.une_sur(10) { 0 } else { owner as u16 });
            wr8(b, obj + 0x2f, alea.pick(&[0, 0, 1]));
            wr16(b, obj + 0x42, (alea.next() % 0x28) as u16);
        }
        _ => {}
    }
}

/// Perturbation à chaque pas (propriétaire, joueurs, scène).
fn perturbe(b: &mut Cps2Bus, obj: u32, owner: u32, alea: &mut Alea) {
    if alea.une_sur(4) {
        wr8(b, owner + 4, alea.pick(&[0, 2, 4, 6]));
        wr8(b, owner + 0x3d, 2 * (alea.next() % 0x1a) as u8);
    }
    if alea.une_sur(6) {
        wr8(b, obj + 0x33, alea.pick(&[0, 1]));
    }
    if alea.une_sur(8) {
        let j = JOUEURS + 0x200 * (alea.next() % 4);
        wr8(b, j, alea.pick(&[0, 1, 1, 2]));
        wr16(b, j + 0x62, alea.pick(&[0, 0x10, 0x40]));
        let (x, y) = (rd16(b, obj + 8), rd16(b, obj + 0x10));
        wr16(b, j + 8, x.wrapping_add((alea.next() % 0x40) as u16).wrapping_sub(0x20));
        wr16(b, j + 0x10, y.wrapping_add((alea.next() % 0x20) as u16).wrapping_sub(0x10));
    }
    // compteur d'images
    wr16(b, A5 + 0x6e, alea.next() as u16);
    // jeu en pause (rare)
    wr8(b, A5 + 0x3d, alea.une_sur(20) as u8);
}

fn main() {
    std::thread::Builder::new().stack_size(1 << 30).spawn(principal).unwrap().join().unwrap();
}

fn principal() {
    let a: Vec<String> = std::env::args().collect();
    let (Some(roms), Some(eeprom), Some(scenario), Some(chauffe), Some(types)) = (a.get(1), a.get(2), a.get(3), a.get(4), a.get(5)) else {
        eprintln!("usage : fuzz_effct <roms> <eeprom> <scénario> <images de chauffe> <types>");
        std::process::exit(2);
    };
    let env = |k: &str, d: u32| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    let variantes: Vec<u32> = std::env::var("FUZZ_VARIANTES")
        .map(|v| v.split(',').map(|x| x.parse().unwrap()).collect())
        .unwrap_or_else(|_| (0..16).collect());
    let tours = env("FUZZ_TOURS", 24);
    let pas = env("FUZZ_PAS", 60);
    let types: Vec<u32> = types.split(',').filter(|x| !x.is_empty()).map(|x| x.parse().unwrap()).collect();
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    m.set_overclock(16.0);
    ddsom_game::install(&mut m);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    for f in 0..chauffe.parse::<usize>().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    ddsom_game::set_check(true);
    std::panic::set_hook(Box::new(|_| {}));
    let (c0, b0) = (m.cpu.clone(), m.bus.clone());
    let mut alea = Alea(0x9e37_79b9_7f4a_7c15 ^ (env("FUZZ_GRAINE", 1) as u64).wrapping_mul(0x1000_0000_01b3));
    let mut table = Vec::new();
    {
        let mut b = b0.clone();
        for k in 0..92 {
            table.push((b.fetch16(0x39ce0 + 4 * k) as u32) << 16 | b.fetch16(0x39ce2 + 4 * k) as u32);
        }
    }
    let mut appels = 0u64;
    let mut abandons = 0u64;
    // créateurs (FUZZ_CREATEURS=adresse,...) : appelés depuis des registres tirés au hasard, A0 sur un
    // joueur, un ennemi ou un effet, A1 dans la RAM de travail ; le pool EFFCT plus ou moins rempli
    let createurs: Vec<u32> = std::env::var("FUZZ_CREATEURS")
        .map(|v| v.split(',').map(|x| u32::from_str_radix(x, 16).unwrap()).collect())
        .unwrap_or_default();
    for &entree in &createurs {
        for _ in 0..tours * 50 {
            let (mut c, mut b) = (c0.clone(), b0.clone());
            b.irq_horizon = u64::MAX;
            for r in 0..8 {
                c.d[r] = if alea.une_sur(2) { alea.next() & 0xff } else { alea.next() << 8 | alea.next() & 0xff };
            }
            c.d[0] = alea.next() & 0xf;
            c.d[1] = alea.next() & 0xf;
            let (e, n) = (EFFCT.slot(alea.next() % EFFCT.slots).0, ENEMY.slot(alea.next() % ENEMY.slots).0);
            c.a[0] = alea.pick(&[JOUEURS, JOUEURS + 0x200, e, n]);
            c.a[1] = 0xff_9000 + 2 * (alea.next() % 0x800);
            c.a[2] = alea.next();
            c.a[5] = A5;
            wr16(&mut b, A5 + EFFCT.free, alea.pick(&[0, 1, 2, 5, 19]));
            appels += 1;
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ddsom_game::call(&mut c, &mut b, entree))).is_err() {
                abandons += 1;
            }
        }
    }
    for &t in &types {
        let entree = table[t as usize];
        let plage = match std::env::var("FUZZ_PLAGE") {
            Ok(p) => {
                let (d, f) = p.split_once('-').unwrap();
                u32::from_str_radix(d, 16).unwrap()..u32::from_str_radix(f, 16).unwrap()
            }
            Err(_) => entree..table.iter().copied().filter(|&e| e > entree).min().unwrap_or(entree + 0x400),
        };
        let mut vus = BTreeSet::new();
        for &v in &variantes {
            for tour in 0..tours {
                let (mut c, mut b) = (c0.clone(), b0.clone());
                b.irq_horizon = u64::MAX;
                let mut proprietaires: Vec<u32> = ENEMY.iter().filter(|o| rd8(&mut b, o.0) != 0).map(|o| o.0).collect();
                proprietaires.extend((0..4).map(|j| JOUEURS + 0x200 * j));
                let obj = EFFCT.slot(alea.next() % EFFCT.slots).0;
                for off in (0..EFFCT.stride).step_by(2) {
                    wr16(&mut b, obj + off, 0);
                }
                let owner = alea.pick(&proprietaires);
                let naissance = tour % 2 == 0;
                wr8(&mut b, obj, 1);
                wr8(&mut b, obj + 2, t as u8);
                wr8(&mut b, obj + 3, v as u8);
                wr8(&mut b, obj + 4, if naissance { 0 } else { 2 });
                wr8(&mut b, obj + 5, if naissance { 0 } else { alea.pick(sous_etats(t)) });
                wr8(&mut b, obj + 0x28, EFFCT.class);
                for off in [0x46u32, 0x50, 0x54, 0x5a, 0x5c, 0x1e] {
                    wr16(&mut b, obj + off, owner as u16);
                }
                for off in [0x8u32, 0xc, 0x10] {
                    let w = rd16(&mut b, owner + off);
                    wr16(&mut b, obj + off, w);
                }
                for off in [0x2fu32, 0x30, 0x31, 0x42, 0x43, 0x58, 0x59] {
                    if alea.une_sur(2) {
                        wr8(&mut b, obj + off, alea.pick(&[0, 1, 2, 3, 4, 8]));
                    }
                }
                wr8(&mut b, obj + 0x2c, alea.pick(&[0, 0x20]));
                wr16(&mut b, A5 + EFFCT.free, (EFFCT.slots / 2) as u16);
                prepare(t, &mut b, obj, owner, &mut alea);
                for k in 0..pas {
                    if rd8(&mut b, obj) == 0 || rd8(&mut b, obj + 2) != t as u8 {
                        break;
                    }
                    if k > 0 {
                        perturbe(&mut b, obj, owner, &mut alea);
                    }
                    c.a[0] = obj;
                    c.a[5] = A5;
                    c.d[7] = k;
                    c.d[0] = 4 * t;
                    c.a[1] = entree;
                    couverture(&c, &b, entree, &plage, &mut vus);
                    appels += 1;
                    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ddsom_game::call(&mut c, &mut b, entree)));
                    if r.is_err() {
                        abandons += 1;
                        break;
                    }
                }
            }
        }
        let mut b = b0.clone();
        let tout = atteignables(&mut b, entree, plage.clone());
        let manque: Vec<u32> = tout.difference(&vus).copied().collect();
        let mut plages: Vec<(u32, u32)> = Vec::new();
        for &p in &manque {
            match plages.last_mut() {
                Some(r) if p - r.1 <= 0x10 => r.1 = p,
                _ => plages.push((p, p)),
            }
        }
        println!(
            "TYPE {t} {entree:06x} : {} instructions atteignables, {} exécutées par l'original, non vues : {}",
            tout.len(),
            tout.intersection(&vus).count(),
            plages.iter().map(|(a, b)| format!("{a:06x}-{b:06x}")).collect::<Vec<_>>().join(" ")
        );
    }
    let mut ecarts = 0;
    for (nom, st) in ddsom_game::report() {
        if st.calls == 0 {
            continue;
        }
        ecarts += st.mismatches;
        if st.mismatches > 0 || nom.starts_with("effct") || nom.starts_with("cree") || nom.starts_with("bloc") {
            println!("  {nom:28} {:>7} appels {:>5} écarts{}", st.calls, st.mismatches, st.first.map(|f| format!(" — {f}")).unwrap_or_default());
        }
    }
    println!("{appels} appels, {abandons} trajectoires abandonnées");
    println!("{}", if ecarts == 0 { "FUZZ VALIDE : aucun écart" } else { "FUZZ ÉCHEC" });
}
