//! Logique de jeu de ddsom réécrite en Rust idiomatique.
//!
//! Une *routine native* remplace une routine 68000 à son adresse d'entrée. Elle produit exactement
//! le même état (mémoire, registres, drapeaux) et déclare la même durée, tirée du code original par
//! `timing::path_cycles`. Si la routine ne tient pas avant l'échéance de la ligne, la version
//! d'origine (recompilée) s'exécute à sa place : le moment des interruptions reste identique.
//!
//! Mode contrôle (`set_check(true)`) : chaque appel exécute aussi la routine d'origine sur une copie
//! de l'état et compare les deux ; les écarts sont comptés dans `report()`.

pub mod inputs;
pub mod mem;
pub mod physics;
pub mod timing;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use cps2::Cps2Bus;
use m68k::Cpu;

/// Routine native.
pub struct Native {
    pub entry: u32,
    pub name: &'static str,
    /// Durée de la routine d'origine en cycles, évaluée sans effet de bord.
    pub cost: fn(&Cpu, &mut Cps2Bus) -> u64,
    /// Exécute la routine, RTS final compris (cycles non comptés : ajoutés par l'appelant).
    pub run: fn(&mut Cpu, &mut Cps2Bus),
    /// La routine accède à la RAM partagée avec le Z80 : elle doit tenir dans la ligne en cours,
    /// sinon l'entrelacement avec le Z80 changerait.
    pub touches_sound: bool,
}

/// Toutes les routines natives.
pub const NATIVES: &[Native] = &[
    physics::OBJ_MOVE_XY,
    physics::OBJ_MOVE_X,
    inputs::READ_INPUTS,
    inputs::COPY_INPUTS_TO_PLAYERS,
    inputs::GET_PLAYER_INPUT,
];

fn lookup(pc: u32) -> Option<&'static Native> {
    NATIVES.iter().find(|n| n.entry == pc)
}

/// Exécuteur : routine native si disponible, sinon bloc recompilé.
pub fn run(pc: u32, c: &mut Cpu, b: &mut Cps2Bus, dl: u64) -> Option<u32> {
    if let Some(n) = lookup(pc) {
        let cost = (n.cost)(c, b);
        let limit = if n.touches_sound { dl } else { b.irq_horizon };
        if c.cycles + cost <= limit {
            if CHECK.load(Ordering::Relaxed) {
                check(n, c, b, cost);
            } else {
                (n.run)(c, b);
                c.cycles += cost;
            }
            return Some(c.pc);
        }
    }
    ddsom_rc::run_block(pc, c, b, dl)
}

/// Active les routines natives (et le code recompilé) sur la machine.
pub fn install(m: &mut cps2::Machine) {
    m.executor = Some(run);
}

static CHECK: AtomicBool = AtomicBool::new(false);

#[derive(Default, Clone)]
pub struct Stats {
    pub calls: u64,
    pub mismatches: u64,
    pub first: Option<String>,
}

static STATS: Mutex<BTreeMap<&'static str, Stats>> = Mutex::new(BTreeMap::new());

pub fn set_check(on: bool) {
    CHECK.store(on, Ordering::Relaxed);
}

/// Bilan du mode contrôle par routine.
pub fn report() -> BTreeMap<&'static str, Stats> {
    STATS.lock().unwrap().clone()
}

/// Exécute la routine d'origine sur une copie et la native sur l'état réel, puis compare.
fn check(n: &Native, c: &mut Cpu, b: &mut Cps2Bus, cost: u64) {
    let mut c2 = c.clone();
    let mut b2 = b.clone();
    let ret = mem::rd32(&mut b2, c2.a[7]);
    let sp_after = c2.a[7].wrapping_add(4);
    let mut guard = 0;
    while !(c2.pc == ret && c2.a[7] == sp_after) {
        if ddsom_rc::run_block(c2.pc, &mut c2, &mut b2, u64::MAX).is_none() {
            c2.step(&mut b2);
        }
        guard += 1;
        assert!(guard < 1_000_000, "{} : la routine d'origine ne revient pas", n.name);
    }
    let start = c.cycles;
    (n.run)(c, b);
    c.cycles += cost;
    let mut diffs = Vec::new();
    if c2.cycles - start != cost {
        diffs.push(format!("cycles {} au lieu de {}", cost, c2.cycles - start));
    }
    for i in 0..8 {
        if c.d[i] != c2.d[i] {
            diffs.push(format!("d{i}={:08x} au lieu de {:08x}", c.d[i], c2.d[i]));
        }
        if c.a[i] != c2.a[i] {
            diffs.push(format!("a{i}={:08x} au lieu de {:08x}", c.a[i], c2.a[i]));
        }
    }
    if c.sr != c2.sr {
        diffs.push(format!("sr={:04x} au lieu de {:04x}", c.sr, c2.sr));
    }
    if c.pc != c2.pc {
        diffs.push(format!("pc={:06x} au lieu de {:06x}", c.pc, c2.pc));
    }
    mem::diff_bus(b, &b2, &mut diffs);
    let mut st = STATS.lock().unwrap();
    let s = st.entry(n.name).or_default();
    s.calls += 1;
    if !diffs.is_empty() {
        s.mismatches += 1;
        if s.first.is_none() {
            s.first = Some(diffs.join(", "));
        }
        // on garde l'état d'origine pour ne pas propager l'écart
        *c = c2;
        *b = b2;
    }
}
