//! Logique de jeu de ddsom réécrite en Rust idiomatique.
//!
//! Une *routine native* remplace une routine 68000 à son adresse d'entrée. Elle produit exactement
//! le même état (mémoire, registres, drapeaux) et déclare la même durée, tirée du code original par
//! `timing::path_cycles`. Si la routine ne tient pas avant l'échéance de la ligne, la version
//! d'origine (recompilée) s'exécute à sa place : le moment des interruptions reste identique.
//!
//! Mode contrôle (`set_check(true)`) : chaque appel exécute aussi la routine d'origine sur une copie
//! de l'état et compare les deux ; les écarts sont comptés dans `report()`.

pub mod chemin;
pub mod config;
pub mod credits;
pub mod eeprom;
pub mod evenements;
pub mod families;
pub mod inputs;
pub mod mem;
pub mod objects;
pub mod ops;
pub mod palette;
pub mod popups;
pub mod rng;
pub mod scenario;
pub mod sound;
pub mod sprites;
pub mod text;
pub mod timing;
pub mod vue;

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

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
    /// Contrat : registres et drapeaux que l'appelant peut lire au retour (voir `regs`). Les autres
    /// registres sont libres ; la mémoire doit toujours être identique.
    pub outputs: u32,
    /// La durée déclarée est exacte (sinon c'est une estimation : seule la mémoire fait foi).
    pub exact_cost: bool,
}

/// Masques de registres pour `Native::outputs`.
pub mod regs {
    pub const fn d(n: u32) -> u32 {
        1 << n
    }
    pub const fn a(n: u32) -> u32 {
        1 << (8 + n)
    }
    pub const CCR: u32 = 1 << 16;
    /// Tous les registres et les drapeaux (remplacement exact d'une routine feuille).
    pub const ALL: u32 = 0x1_ffff;
}

/// Toutes les routines natives.
pub const NATIVES: &[Native] = &[
    inputs::READ_INPUTS,
    inputs::COPY_INPUTS_TO_PLAYERS,
    inputs::GET_PLAYER_INPUT,
    sprites::BUILD_SPRITE_LIST,
    text::PRINT,
    text::PRINT_OVER,
    text::PRINT_TALL,
    text::PRINT_TALL_AT,
    text::CLEAR_BLOCKS,
    text::CLEAR_BOX,
    palette::FULL_BRIGHTNESS,
    palette::NO_BRIGHTNESS,
    palette::OR_BRIGHTNESS,
    palette::SET_BRIGHTNESS,
    sound::ENFILER_SI_SON,
    sound::ENFILER_DIRECT,
    sound::ENVOYER_AU_Z80,
    rng::SEED,
    rng::NEXT,
    evenements::VIDER_POINTS,
    evenements::VIDER_COMPTEUR,
    evenements::VIDER_POPUPS,
    evenements::EFFACER_POPUP,
    evenements::ENFILER_POINTS,
    evenements::ENFILER_COMPTEUR,
    evenements::EVENEMENT_OBJET,
    evenements::CREER_POPUP,
    evenements::CREDITER_POINTS,
    evenements::CREDITER_COMPTEUR,
    credits::LIRE_MONNAYEURS,
    credits::LIRE_DEPARTS,
    credits::VERROUILLER_COMPTEURS,
    eeprom::LIRE,
    eeprom::ECRIRE,
    eeprom::ECRIRE_VERIFIER,
    eeprom::LIRE_VERIFIER,
    eeprom::COMPTER_PIECES,
    eeprom::COMPTER_DEPARTS,
    eeprom::COMPTER_CONTINUES,
    eeprom::COMPTER_PARTIES,
    eeprom::VBLANK_COMPTA,
    config::VARIANTE,
    config::DEFAUTS,
    config::VALIDER,
    config::VARIABLES,
    popups::ANIMER,
    credits::ECRIRE_CHIFFRE,
    eeprom::COMPLET_PIECES_NATIF,
    eeprom::COMPLET_DEPARTS_NATIF,
];

/// Index des routines natives par adresse d'entrée : `NATIVES`, le modèle d'objets et les wrappers
/// son (décrits par une table). Un tamis binaire évite la recherche pour la grande majorité des blocs.
struct Index {
    tamis: Vec<u64>,
    par_adresse: HashMap<u32, &'static Native>,
}

fn lookup(pc: u32) -> Option<&'static Native> {
    static INDEX: OnceLock<Index> = OnceLock::new();
    let index = INDEX.get_or_init(|| {
        let par_adresse: HashMap<_, _> =
            all_natives().map(|n| (n.entry, n)).collect();
        let mut tamis = vec![0u64; 0x100000 / 2 / 64];
        for &a in par_adresse.keys() {
            tamis[(a >> 7) as usize] |= 1 << (a >> 1 & 63);
        }
        Index { tamis, par_adresse }
    });
    let case = index.tamis.get((pc >> 7) as usize)?;
    if case >> (pc >> 1 & 63) & 1 == 0 {
        return None;
    }
    index.par_adresse.get(&pc).copied()
}

/// Toutes les routines natives, wrappers son compris (outils de couverture).
pub fn all_natives() -> impl Iterator<Item = &'static Native> {
    NATIVES.iter().chain(objects::NATIVES).chain(families::NATIVES).chain(sound::wrappers_natifs())
}

/// Appelle une routine d'origine (pas encore réécrite) depuis une routine native, comme un JSR :
/// exécution jusqu'au retour, natives et blocs recompilés compris. Les cycles s'accumulent.
pub fn call(c: &mut Cpu, b: &mut Cps2Bus, addr: u32) {
    // adresse de retour sentinelle : jamais exécutée par le jeu
    const RETURN: u32 = 0x00_fffe;
    let sp = c.a[7];
    c.a[7] = sp.wrapping_sub(4);
    mem::wr32(b, c.a[7], RETURN);
    c.pc = addr;
    while !(c.pc == RETURN && c.a[7] == sp) {
        if run(c.pc, c, b, u64::MAX).is_none() {
            c.step(b);
        }
    }
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

/// Exécuteur : routines natives seulement (le reste est laissé à l'interpréteur). Outils de diagnostic.
pub fn run_native_only(pc: u32, c: &mut Cpu, b: &mut Cps2Bus, dl: u64) -> Option<u32> {
    let n = lookup(pc)?;
    let cost = (n.cost)(c, b);
    let limit = if n.touches_sound { dl } else { b.irq_horizon };
    if c.cycles + cost > limit {
        return None;
    }
    (n.run)(c, b);
    c.cycles += cost;
    Some(c.pc)
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
    /// Dernier écart (outils de diagnostic).
    pub last: Option<String>,
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
    if n.exact_cost && c2.cycles - start != cost {
        diffs.push(format!("cycles {} au lieu de {}", cost, c2.cycles - start));
    }
    for i in 0..8 {
        if n.outputs & regs::d(i as u32) != 0 && c.d[i] != c2.d[i] {
            diffs.push(format!("d{i}={:08x} au lieu de {:08x}", c.d[i], c2.d[i]));
        }
        // A7 (pile) doit toujours être rétabli
        if (n.outputs & regs::a(i as u32) != 0 || i == 7) && c.a[i] != c2.a[i] {
            diffs.push(format!("a{i}={:08x} au lieu de {:08x}", c.a[i], c2.a[i]));
        }
    }
    let sr_mask = if n.outputs & regs::CCR != 0 { 0xffff } else { 0xff00 };
    if c.sr & sr_mask != c2.sr & sr_mask {
        diffs.push(format!("sr={:04x} au lieu de {:04x}", c.sr, c2.sr));
    }
    if c.pc != c2.pc {
        diffs.push(format!("pc={:06x} au lieu de {:06x}", c.pc, c2.pc));
    }
    let sp = c2.a[7] & 0xff_ffff;
    mem::diff_bus(b, &b2, sp.saturating_sub(0x400)..sp, &mut diffs);
    // On reprend la durée réelle de l'original : le reste de l'exécution suit exactement le timing
    // du code d'origine. Les registres hors contrat gardent les valeurs de la routine native : si un
    // appelant en dépendait (contrat incomplet), la suite divergera et la validation le verra.
    c.cycles = c2.cycles;
    let mut st = STATS.lock().unwrap();
    let s = st.entry(n.name).or_default();
    s.calls += 1;
    if !diffs.is_empty() {
        s.mismatches += 1;
        if s.first.is_none() {
            s.first = Some(diffs.join(", "));
        }
        s.last = Some(diffs.join(", "));
        // on garde l'état d'origine pour ne pas propager l'écart
        *c = c2;
        *b = b2;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn entrees_uniques() {
        let mut vues = std::collections::HashSet::new();
        for n in super::all_natives() {
            assert!(vues.insert(n.entry), "{:06x} ({}) remplacée deux fois", n.entry, n.name);
        }
    }
}
