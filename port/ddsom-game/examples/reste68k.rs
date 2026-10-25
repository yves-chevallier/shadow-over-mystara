//! Inventaire du code 68000 qui s'exécute encore hors des routines natives.
//!
//!   reste68k run <roms> <eeprom> <scénario> <images> <sortie.cnt>
//!       rejoue un scénario sur la machine hybride (natives Rust + 68000 interprété, une instruction
//!       à la fois) et écrit « adresse nombre » (hexadécimal) pour chaque instruction 68000 exécutée ;
//!   reste68k report <index.tsv> <atlas.md> <sortie.md> <fichier.cnt>...
//!       fusionne les comptes, les regroupe par fonction (decomp/index.tsv) et par sous-système (atlas)
//!       et écrit le tableau de `docs/reste68k.md` ; `DDSOM_JAMAIS=fichier` y écrit en plus la liste des
//!       fonctions jamais atteintes (« adresse taille sous-système »).
//! Orchestration : `tools/reste68k.sh`.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;
use std::sync::Mutex;

/// Compteur d'exécutions par mot de programme (0x000000-0x0FFFFF).
static COUNTS: Mutex<Vec<u32>> = Mutex::new(Vec::new());

fn trace(pc: u32) {
    COUNTS.lock().unwrap()[(pc as usize & 0xfffff) >> 1] += 1;
}

fn run(a: &[String]) {
    let [roms, eeprom, scenario, frames, out] = a else {
        eprintln!("usage : reste68k run <roms> <eeprom> <scénario> <images> <sortie.cnt>");
        std::process::exit(2);
    };
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    m.set_overclock(16.0);
    ddsom_game::install(&mut m);
    // exécuteur d'inventaire : natives, sinon une instruction 68000 comptée
    m.executor = Some(ddsom_game::run_traced);
    ddsom_game::set_trace(trace);
    *COUNTS.lock().unwrap() = vec![0; 0x80000];
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    let frames: usize = frames.parse().unwrap();
    for f in 0..frames {
        script.apply(f, &mut m);
        m.run_frame();
    }
    let mut s = format!("# {scenario} {frames}\n");
    for (w, &n) in COUNTS.lock().unwrap().iter().enumerate().filter(|(_, &n)| n > 0) {
        writeln!(s, "{:06x} {n}", 2 * w).unwrap();
    }
    std::fs::write(out, s).unwrap();
}

/// Attente active : ordonnanceur et traps (0x928-0xb00) et tâche d'avance rapide (0xee080), qui cède
/// sans cesse. Surcadencé, le processeur y passe tout le temps que le jeu n'utilise pas : le nombre
/// d'instructions n'est pas du travail. Comptées à part.
const SPIN: [std::ops::Range<u32>; 2] = [0x928..0xb00, 0xee080..0xee16e];

struct Func {
    addr: u32,
    name: String,
    size: u32,
    instr: u64,
    scenarios: u32,
    native: bool,
}

#[derive(Default, Clone)]
struct Sub {
    instr: u64,
    funcs: u32,
    touched: u32,
    native: u32,
    never: u32,
    never_bytes: u32,
    /// Instructions exécutées dans la plage du sous-système, hors de toute fonction de l'index.
    orphan: u64,
}

/// Sous-systèmes de l'atlas : (id, nom, plages d'adresses), lus dans le tableau de la section 4.1.
fn atlas(path: &str) -> Vec<(String, String, Vec<(u32, u32)>)> {
    let text = std::fs::read_to_string(path).unwrap();
    let mut v = Vec::new();
    for l in text.lines() {
        let c: Vec<&str> = l.split('|').map(str::trim).collect();
        if c.len() < 9 || c[1].is_empty() || c[1].len() > 3 || c[1] == "ID" {
            continue;
        }
        let ranges: Vec<(u32, u32)> = c[3]
            .split(',')
            .filter_map(|r| {
                let (a, b) = r.trim().split_once('-')?;
                Some((u32::from_str_radix(a, 16).ok()?, u32::from_str_radix(b, 16).ok()?))
            })
            .collect();
        if !ranges.is_empty() {
            v.push((c[1].to_string(), c[2].to_string(), ranges));
        }
    }
    v
}

fn report(a: &[String]) {
    let [index, atlas_md, out, cnts @ ..] = a else {
        eprintln!("usage : reste68k report <index.tsv> <atlas.md> <sortie.md> <fichier.cnt>...");
        std::process::exit(2);
    };
    let natives: Vec<u32> = ddsom_game::all_natives().map(|n| n.entry).collect();
    let mut funcs: Vec<Func> = std::fs::read_to_string(index)
        .unwrap()
        .lines()
        .skip(1)
        .filter_map(|l| {
            let v: Vec<&str> = l.split('\t').collect();
            Some(Func {
                addr: u32::from_str_radix(v[0], 16).ok()?,
                name: v[1].to_string(),
                size: v[2].parse().ok()?,
                instr: 0,
                scenarios: 0,
                native: false,
            })
        })
        .collect();
    funcs.sort_by_key(|f| f.addr);
    for f in &mut funcs {
        f.native = natives.iter().any(|&e| e >= f.addr && e < f.addr + f.size.max(2));
    }
    let mut frames = 0usize;
    let (mut orphan, mut spin) = (0u64, 0u64);
    let mut orphans: BTreeMap<u32, u64> = BTreeMap::new();
    let mut per_scenario: Vec<(String, usize, u64)> = Vec::new();
    for c in cnts {
        let text = std::fs::read_to_string(c).unwrap();
        let mut lines = text.lines();
        let head: Vec<&str> = lines.next().unwrap().trim_start_matches("# ").rsplitn(2, ' ').collect();
        let n: usize = head[0].parse().unwrap();
        frames += n;
        let mut seen = vec![false; funcs.len()];
        let mut total = 0;
        for l in lines {
            let (pc, k) = l.split_once(' ').unwrap();
            let (pc, k) = (u32::from_str_radix(pc, 16).unwrap(), k.parse::<u64>().unwrap());
            total += k;
            if SPIN.iter().any(|r| r.contains(&pc)) {
                spin += k;
                continue;
            }
            let i = funcs.partition_point(|f| f.addr <= pc);
            if i > 0 && pc < funcs[i - 1].addr + funcs[i - 1].size {
                funcs[i - 1].instr += k;
                seen[i - 1] = true;
            } else {
                orphan += k;
                *orphans.entry(pc).or_default() += k;
            }
        }
        for (f, s) in funcs.iter_mut().zip(&seen) {
            f.scenarios += *s as u32;
        }
        per_scenario.push((head[1].to_string(), n, total));
    }
    let atlas = atlas(atlas_md);
    let sub = |pc: u32| atlas.iter().position(|(_, _, r)| r.iter().any(|&(a, b)| pc >= a && pc < b)).unwrap_or(atlas.len());
    let label = |i: usize| atlas.get(i).map(|a| (a.0.as_str(), a.1.as_str())).unwrap_or(("-", "hors atlas"));
    let total: u64 = funcs.iter().map(|f| f.instr).sum::<u64>() + orphan;
    let mut subs = vec![Sub::default(); atlas.len() + 1];
    for f in &funcs {
        let s = &mut subs[sub(f.addr)];
        s.funcs += 1;
        s.instr += f.instr;
        if f.instr > 0 {
            s.touched += 1;
        } else if f.native {
            s.native += 1;
        } else {
            s.never += 1;
            s.never_bytes += f.size;
        }
    }
    for (&pc, &k) in &orphans {
        subs[sub(pc)].orphan += k;
    }
    // Plages contiguës (trous < 0x40) de code hors index : (début, fin, instructions).
    let mut plages: Vec<(u32, u32, u64)> = Vec::new();
    for (&pc, &k) in &orphans {
        match plages.last_mut() {
            Some(p) if pc - p.1 < 0x40 => {
                p.1 = pc;
                p.2 += k;
            }
            _ => plages.push((pc, pc, k)),
        }
    }
    plages.sort_by_key(|p| std::cmp::Reverse(p.2));
    let touched: Vec<&Func> = {
        let mut v: Vec<&Func> = funcs.iter().filter(|f| f.instr > 0).collect();
        v.sort_by_key(|f| std::cmp::Reverse(f.instr));
        v
    };
    let only_native = funcs.iter().filter(|f| f.instr == 0 && f.native).count();
    let never: Vec<&Func> = funcs.iter().filter(|f| f.instr == 0 && !f.native).collect();
    // DDSOM_JAMAIS=fichier : liste « adresse taille sous-système » des fonctions jamais atteintes (classement, docs/couverture.md)
    if let Ok(path) = std::env::var("DDSOM_JAMAIS") {
        let text: String = never.iter().map(|f| format!("{:06x} {} {}\n", f.addr, f.size, label(sub(f.addr)).0)).collect();
        std::fs::write(path, text).unwrap();
    }
    let pct = |n: u64| 100.0 * n as f64 / total.max(1) as f64;

    let mut md = String::new();
    writeln!(md, "# Code 68000 restant\n").unwrap();
    writeln!(md, "Généré par `tools/reste68k.sh` (`port/ddsom-game/examples/reste68k.rs`) ; ne pas éditer à la main.\n").unwrap();
    writeln!(md, "Mesure : la machine hybride du jeu (routines natives Rust, 68000 pour le reste, ×16) rejoue {} scénarios, soit {frames} images. Chaque instruction 68000 exécutée **hors routine native** est comptée, y compris le code appelé par une native via `ddsom_game::call`. Les natives et le Z80 (son) ne comptent pas.\n", cnts.len()).unwrap();
    writeln!(md, "* **{total} instructions 68000 exécutées** (hors attente active), soit {:.1} par image ;", total as f64 / frames.max(1) as f64).unwrap();
    writeln!(md, "* attente active, non comptée : {spin} instructions (ordonnanceur et traps 0x928-0xb00, tâche du gel 0xee080 : voir `SPIN`) ;").unwrap();
    writeln!(md, "* {} fonctions dans `decomp/index.tsv` : {} exécutent encore du 68000, {only_native} sont entièrement natives, **{} jamais atteintes** ({} octets : code mort, ou à exercer par de nouveaux scénarios) ;", funcs.len(), touched.len(), never.len(), never.iter().map(|f| f.size).sum::<u32>()).unwrap();
    writeln!(md, "* instructions hors de toute fonction de l'index : {orphan} (comptées par sous-système ci-dessous, colonne « Hors index », et par plage plus bas).\n").unwrap();

    writeln!(md, "## Par sous-système (atlas)\n").unwrap();
    writeln!(md, "| ID | Sous-système | Instr. 68000 | % | Hors index | Fonctions | Touchées | Natives seules | Jamais atteintes | Principales fonctions restantes |\n|---|---|---:|---:|---:|---:|---:|---:|---:|---|").unwrap();
    let mut order: Vec<usize> = (0..subs.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(subs[i].instr + subs[i].orphan));
    for i in order {
        let (s, (id, name)) = (&subs[i], label(i));
        let top: Vec<String> = touched.iter().filter(|f| sub(f.addr) == i).take(3).map(|f| format!("{:x} ({:.1} %)", f.addr, 100.0 * f.instr as f64 / s.instr.max(1) as f64)).collect();
        writeln!(md, "| {id} | {name} | {} | {:.2} | {} | {} | {} | {} | {} | {} |", s.instr + s.orphan, pct(s.instr + s.orphan), s.orphan, s.funcs, s.touched, s.native, s.never, top.join(", ")).unwrap();
    }

    writeln!(md, "\n## Hors de toute fonction de l'index (les 40 plages les plus gourmandes)\n").unwrap();
    writeln!(md, "Code que Ghidra n'a pas découpé en fonctions (gestionnaires atteints par table, par exemple) : à désassembler depuis le listing. Plages contiguës à moins de 0x40 octets.\n").unwrap();
    writeln!(md, "| Plage | Sous-système | Instr. 68000 | % |\n|---|---|---:|---:|").unwrap();
    for &(a, b, k) in plages.iter().take(40) {
        writeln!(md, "| {a:06x}-{b:06x} | {} | {k} | {:.2} |", label(sub(a)).0, pct(k)).unwrap();
    }

    writeln!(md, "\n## Par fonction (les 80 plus gourmandes)\n").unwrap();
    writeln!(md, "| Adresse | Nom | Sous-système | Taille | Instr. 68000 | % | Scénarios |\n|---|---|---|---:|---:|---:|---:|").unwrap();
    for f in touched.iter().take(80) {
        writeln!(md, "| {:06x} | {} | {} | {} | {} | {:.2} | {} |", f.addr, f.name, label(sub(f.addr)).0, f.size, f.instr, pct(f.instr), f.scenarios).unwrap();
    }
    let (mut cum, mut marks) = (0u64, [(90.0, 0), (99.0, 0), (100.0, 0)]);
    for (n, f) in touched.iter().enumerate() {
        cum += f.instr;
        for m in marks.iter_mut().filter(|m| m.1 == 0 && pct(cum) >= m.0 - 1e-9) {
            m.1 = n + 1;
        }
    }
    writeln!(md, "\nConcentration : {} fonctions pour 90 % des instructions restantes, {} pour 99 %, {} pour toutes les fonctions touchées.", marks[0].1, marks[1].1, touched.len()).unwrap();

    writeln!(md, "\n## Fonctions jamais atteintes, par sous-système\n").unwrap();
    writeln!(md, "| ID | Sous-système | Fonctions | Octets |\n|---|---|---:|---:|").unwrap();
    let mut o2: Vec<usize> = (0..subs.len()).filter(|&i| subs[i].never > 0).collect();
    o2.sort_by_key(|&i| std::cmp::Reverse(subs[i].never_bytes));
    for i in o2 {
        let (id, name) = label(i);
        writeln!(md, "| {id} | {name} | {} | {} |", subs[i].never, subs[i].never_bytes).unwrap();
    }

    writeln!(md, "\n## Scénarios\n\n| Scénario | Images | Instr. 68000 | par image |\n|---|---:|---:|---:|").unwrap();
    let by_scenario: BTreeMap<&str, _> = per_scenario.iter().map(|(s, n, t)| (s.as_str(), (n, t))).collect();
    for (s, (n, t)) in by_scenario {
        writeln!(md, "| {s} | {n} | {t} | {:.1} |", *t as f64 / (*n).max(1) as f64).unwrap();
    }
    std::fs::write(out, md).unwrap();
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    match a.get(1).map(String::as_str) {
        Some("run") => run(&a[2..]),
        Some("report") => report(&a[2..]),
        _ => eprintln!("usage : reste68k run|report ..."),
    }
}
