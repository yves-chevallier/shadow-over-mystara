//! Contrôle détaillé des gestionnaires des familles ITEM / BOX / EFFCT sur un scénario : chaque appel
//! est exécuté par l'original et par la routine native sur des copies, puis comparé (RAM, registres
//! de sortie, cycles). Un écart affiche l'objet (type, variante, états, 0x60 octets) avant l'appel.
//!   familles <roms> <eeprom> <scénario> <images> [nombre d'écarts à afficher]
//! Affiche aussi la couverture des décisions des gestionnaires (lignes `COUV fichier ligne résultat`).

use std::cell::RefCell;
use std::path::Path;

use cps2::Cps2Bus;
use m68k::Cpu;

thread_local! {
    static RESTE: RefCell<u32> = const { RefCell::new(5) };
    static APPELS: RefCell<std::collections::BTreeMap<&'static str, (u64, u64)>> =
        RefCell::new(std::collections::BTreeMap::new());
}

fn run(pc: u32, c: &mut Cpu, b: &mut Cps2Bus, dl: u64) -> Option<u32> {
    let Some(n) = ddsom_game::families::NATIVES.iter().find(|n| n.entry == pc) else {
        return ddsom_game::run(pc, c, b, dl);
    };
    let cout = (n.cost)(c, b);
    let obj = c.a[0];
    let avant: Vec<u8> = (0..0x60).map(|i| ddsom_game::mem::rd8(b, obj + i)).collect();
    let mut c1 = c.clone();
    let mut b1 = b.clone();
    let (c0, mut b0) = (c.clone(), b.clone());
    let debut = c1.cycles;
    (n.run)(&mut c1, &mut b1);
    c1.cycles += cout;
    // original
    let ret = ddsom_game::mem::rd32(b, c.a[7]);
    let sp_apres = c.a[7].wrapping_add(4);
    while !(c.pc == ret && c.a[7] == sp_apres) {
        if ddsom_rc::run_block(c.pc, c, b, u64::MAX).is_none() {
            c.step(b);
        }
    }
    let mut diffs = Vec::new();
    if c1.cycles - debut != c.cycles - debut {
        diffs.push(format!("cycles {} au lieu de {}", c1.cycles - debut, c.cycles - debut));
    }
    if c1.d[7] != c.d[7] {
        diffs.push(format!("d7={:08x} au lieu de {:08x}", c1.d[7], c.d[7]));
    }
    if c1.a[0] != c.a[0] {
        diffs.push(format!("a0={:08x} au lieu de {:08x}", c1.a[0], c.a[0]));
    }
    let sp = c.a[7] & 0xff_ffff;
    ddsom_game::mem::diff_bus(&b1, b, sp.saturating_sub(0x400)..sp, &mut diffs);
    APPELS.with(|m| {
        let mut m = m.borrow_mut();
        let e = m.entry(n.name).or_insert((0, 0));
        e.0 += 1;
        e.1 += !diffs.is_empty() as u64;
    });
    let afficher = !diffs.is_empty()
        && RESTE.with(|r| {
            let v = *r.borrow();
            *r.borrow_mut() = v.saturating_sub(1);
            v > 0
        });
    if afficher {
        ddsom_game::families::activer_diagnostic(true);
        (n.cost)(&c0, &mut b0);
        ddsom_game::families::activer_diagnostic(false);
        if std::env::var_os("FAMILLES_TRACE").is_some() {
            // trace de l'original, instruction par instruction (pc et cycles cumulés)
            let (mut ct, mut bt) = (c0.clone(), b0.clone());
            let ret = ddsom_game::mem::rd32(&mut bt, ct.a[7]);
            let sp_apres = ct.a[7].wrapping_add(4);
            let t0 = ct.cycles;
            while !(ct.pc == ret && ct.a[7] == sp_apres) {
                let pc = ct.pc;
                let avant = ct.cycles;
                ct.step(&mut bt);
                println!("      {pc:06x} +{} ({})", ct.cycles - avant, ct.cycles - t0);
            }
        }
        println!("== {} : {}", n.name, diffs.join(", "));
        println!("   objet {obj:06x} : type {:02x} variante {:02x} états {:02x} {:02x} {:02x} {:02x}  d7={:04x}",
            avant[2], avant[3], avant[4], avant[5], avant[6], avant[7], c1.d[7] & 0xffff);
        for l in avant.chunks(16).enumerate() {
            println!("   +{:02x} {}", l.0 * 16, l.1.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" "));
        }
    }
    Some(c.pc)
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let (Some(roms), Some(eeprom), Some(scenario), Some(frames)) = (a.get(1), a.get(2), a.get(3), a.get(4)) else {
        eprintln!("usage : familles <roms> <eeprom> <scénario> <images> [écarts]");
        std::process::exit(2);
    };
    if let Some(n) = a.get(5) {
        RESTE.with(|r| *r.borrow_mut() = n.parse().unwrap());
    }
    ddsom_game::families::activer_couverture();
    // les autres routines natives en mode contrôle : la partie suit exactement le temps de l'original
    ddsom_game::set_check(true);
    let ee = std::fs::read(eeprom).ok();
    let mut m = cps2::Machine::load_ddsom(Path::new(roms), ee.as_deref()).expect("roms");
    m.executor = Some(run);
    let script = ddsom_game::scenario::Scenario::load(Path::new(scenario)).expect("scénario");
    for f in 0..frames.parse().unwrap() {
        script.apply(f, &mut m);
        m.run_frame();
    }
    for (fichier, ligne, pris) in ddsom_game::families::couverture() {
        println!("COUV {} {ligne} {pris}", fichier.rsplit('/').next().unwrap());
    }
    for (nom, (appels, ecarts)) in APPELS.with(|m| m.borrow().clone()) {
        println!("  {nom:24} {appels:>8} appels {ecarts:>5} écarts");
    }
}
