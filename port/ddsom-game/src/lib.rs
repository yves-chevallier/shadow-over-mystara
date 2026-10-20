//! Logique de jeu de ddsom réécrite en Rust idiomatique.
//!
//! Une *routine native* remplace une routine 68000 à son adresse d'entrée. Elle produit exactement
//! le même état (mémoire, registres, drapeaux) et déclare la même durée, tirée du code original par
//! `timing::path_cycles`. Si la routine ne tient pas avant l'échéance de la ligne, la version
//! d'origine (recompilée) s'exécute à sa place : le moment des interruptions reste identique.
//!
//! Mode contrôle (`set_check(true)`) : chaque appel exécute aussi la routine d'origine sur une copie
//! de l'état et compare les deux ; les écarts sont comptés dans `report()`.

pub mod camera_modes;
pub mod chemin;
pub mod config;
pub mod credits;
pub mod demarrage;
pub mod dispatch;
pub mod ecran_credits;
pub mod effects_aids;
pub mod eeprom;
pub mod evenements;
pub mod game;
pub mod hud;
pub mod aides;
pub mod ai;
pub mod ai_aids;
pub mod combat;
pub mod families;
pub mod inputs;
pub mod kernel;
pub mod mem;
pub mod modes;
pub mod objects;
pub mod ops;
pub mod palette;
pub mod player;
pub mod popups;
pub mod rng;
pub mod scene;
pub mod scroll;
pub mod scenario;
pub mod sound;
pub mod terrain;
pub mod sprites;
pub mod tache_credits;
pub mod taches;
pub mod text;
pub mod timing;
pub mod video;
pub mod vblank;
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
    combat::COMBAT_UPDATE,
    ai::PICK_TARGET,
    ai::PICK_TARGET_ANY,
    ai::RANDOM_PLAYER,
    ai::INITIAL_HP_A,
    ai::INITIAL_HP_B,
    ai::DECISION_A,
    ai::DECISION_B,
    ai::PICK_BYTE,
    ai::IMMUNITIES,
    ai::IN_ZONE,
    ai::KEEP_ON_SCREEN,
    ai::APPROACH_TARGET,
    ai::TURN_TOWARD,
    ai::SPREAD_INDEX,
    ai::TARGET_IN_FRONT,
    ai::START_ANIMATION,
    ai::STEP_ANIMATION,
    ai::FACING_TARGET,
    ai::MOVING_FORWARD,
    inputs::READ_INPUTS,
    inputs::COPY_INPUTS_TO_PLAYERS,
    inputs::GET_PLAYER_INPUT,
    sprites::BUILD_SPRITE_LIST,
    sprites::RESET_LISTS,
    text::PRINT,
    text::ERASE,
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
        // DDSOM_SANS=préfixe,... : routines natives désactivées (recherche d'une routine fautive)
        let sans: Vec<String> = std::env::var("DDSOM_SANS")
            .map(|v| v.split(',').map(str::to_owned).collect())
            .unwrap_or_default();
        let par_adresse: HashMap<_, _> = all_natives()
            .filter(|n| !sans.iter().any(|p| n.name.starts_with(p.as_str())))
            .map(|n| (n.entry, n))
            .collect();
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
    // une source par ligne (les branches parallèles ajoutent chacune la leur)
    std::iter::empty()
        .chain(NATIVES)
        .chain(objects::NATIVES)
        .chain(player::NATIVES)
        .chain(video::NATIVES)
        .chain(palette::NATIVES)
        .chain(scroll::NATIVES)
        .chain(camera_modes::NATIVES)
        .chain(terrain::NATIVES)
        .chain(ai_aids::NATIVES)
        .chain(effects_aids::NATIVES)
        .chain(families::NATIVES)
        .chain(families::natives())
        .chain(dispatch::NATIVES)
        .chain(game::NATIVES)
        .chain(scene::NATIVES)
        .chain(scene::exit::NATIVES)
        .chain(modes::natives())
        .chain(sound::wrappers_natifs())
        .chain(demarrage::NATIVES)
        .chain(kernel::NATIVES)
        .chain(vblank::NATIVES)
        .chain(taches::natives())
        .chain(tache_credits::NATIVES)
        .chain(ecran_credits::NATIVES)
        .chain(aides::NATIVES)
        .chain(hud::NATIVES)
}

/// Appelle une routine d'origine (pas encore réécrite) depuis une routine native, comme un JSR :
/// exécution jusqu'au retour, natives et blocs recompilés compris. Les cycles s'accumulent.
/// L'adresse de retour empilée est une sentinelle : préférer `call_at` quand l'appelant d'origine
/// est connu.
pub fn call(c: &mut Cpu, b: &mut Cps2Bus, addr: u32) {
    // jamais exécutée par le jeu
    const RETURN: u32 = 0x00_fffe;
    call_at(c, b, addr, RETURN);
}

/// Point de bascule de l'ordonnanceur : une tâche qui cède la main (0xa40, 0xa86) ou se termine y
/// passe, sa pile et son PC sauvegardés.
const TASK_SWITCH: u32 = 0x984;

/// Comme `call`, en empilant `ret`, l'adresse de retour de l'appel d'origine (instruction qui suit
/// son `jsr`). Si la routine cède la main à l'ordonnanceur, les fonctions Rust en cours sont
/// abandonnées (`Unwound`) : la tâche, réveillée plus tard, reviendra par les adresses de retour de
/// sa pile dans le code d'origine, qui finit le travail comme l'original. Les appelants Rust doivent
/// donc empiler les mêmes adresses de retour que le code qu'ils remplacent.
pub fn call_at(c: &mut Cpu, b: &mut Cps2Bus, addr: u32, ret: u32) {
    let sp = c.a[7];
    c.a[7] = sp.wrapping_sub(4);
    mem::wr32(b, c.a[7], ret);
    c.pc = addr;
    while !(c.pc == ret && c.a[7] == sp) {
        if c.pc == TASK_SWITCH || c.sr & 0x2000 == 0 && c.a[7] > sp && c.a[7] - sp < 0x1000 {
            // la tâche cède la main, ou le code 68000 a dépilé plus que son adresse de retour et
            // poursuit ailleurs
            std::panic::resume_unwind(Box::new(Unwound));
        }
        if TRACE.get().is_some() {
            run_traced(c.pc, c, b, u64::MAX);
        } else if run(c.pc, c, b, u64::MAX).is_none() {
            c.step(b);
        }
    }
}

/// Inventaire du 68000 restant (`examples/reste68k`) : fonction appelée avec l'adresse de chaque
/// instruction 68000 exécutée hors routine native.
static TRACE: OnceLock<fn(u32)> = OnceLock::new();

pub fn set_trace(f: fn(u32)) {
    let _ = TRACE.set(f);
}

/// Exécuteur d'inventaire : routines natives, sinon une instruction 68000 à la fois, signalée à
/// `set_trace`. Les durées ne sont pas contrôlées (le 68000 est surcadencé).
pub fn run_traced(pc: u32, c: &mut Cpu, b: &mut Cps2Bus, _dl: u64) -> Option<u32> {
    if let Some(n) = lookup(pc) {
        let cost = (n.cost)(c, b);
        exec(n, c, b);
        c.cycles += cost;
    } else {
        if let Some(t) = TRACE.get() {
            t(pc);
        }
        c.step(b);
    }
    Some(c.pc)
}

/// Fin anormale d'un appel : une routine d'origine a quitté en dépilant les adresses de retour de ses
/// appelants (par exemple en revenant directement à la boucle de partie), ou a cédé la main à
/// l'ordonnanceur. Les fonctions Rust qui l'appelaient sont abandonnées ; l'exécution reprend au PC
/// atteint, comme dans l'original.
pub struct Unwound;

/// Exécute une native ; renvoie normalement, ou après abandon des fonctions Rust si le code 68000
/// appelé a quitté par un autre chemin que son retour (voir `Unwound`).
fn exec(n: &Native, c: &mut Cpu, b: &mut Cps2Bus) {
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (n.run)(c, b)));
    if let Err(payload) = r {
        if !payload.is::<Unwound>() {
            std::panic::resume_unwind(payload);
        }
    }
}

/// Exécuteur : routine native si disponible, sinon bloc recompilé.
pub fn run(pc: u32, c: &mut Cpu, b: &mut Cps2Bus, dl: u64) -> Option<u32> {
    if let Some(n) = lookup(pc) {
        let cost = (n.cost)(c, b);
        let limit = if n.touches_sound { dl } else { b.irq_horizon };
        if c.cycles + cost <= limit {
            if CHECK.load(Ordering::Relaxed) && checked(n) {
                check(n, c, b, cost);
            } else if adopt_regs(n) {
                // diagnostic : registres de sortie repris de l'original (registres hérités)
                let c2 = original(n, c, b);
                exec(n, c, b);
                c.cycles += cost;
                if c.pc == c2.pc {
                    // DDSOM_ADOPT_MASK=hex : seuls les registres du masque (bits de `regs`) sont repris
                    let mask = adopt_mask();
                    if std::env::var_os("DDSOM_ADOPT_TRACE").is_some() {
                        let d: Vec<String> = (0..8).filter(|&i| c.d[i] != c2.d[i]).map(|i| format!("d{i}={:08x}/{:08x}", c.d[i], c2.d[i])).chain((0..7).filter(|&i| c.a[i] != c2.a[i]).map(|i| format!("a{i}={:08x}/{:08x}", c.a[i], c2.a[i]))).collect();
                        eprintln!("ADOPT {} {}", n.name, d.join(" "));
                    }
                    for i in 0..8 {
                        if mask & regs::d(i) != 0 {
                            c.d[i as usize] = c2.d[i as usize];
                        }
                        if i < 7 && mask & regs::a(i) != 0 {
                            c.a[i as usize] = c2.a[i as usize];
                        }
                    }
                    if mask & regs::CCR != 0 {
                        c.sr = c2.sr;
                    }
                }
            } else {
                exec(n, c, b);
                c.cycles += cost;
                if COUNT.load(Ordering::Relaxed) {
                    STATS.lock().unwrap().entry(n.name).or_default().calls += 1;
                }
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

/// Début d'un pas de jeu dans la boucle principale (0x1c4b0 : `st (-0x660e,A5)` juste avant l'appel de la phase).
pub const GAME_STEP: u32 = 0x1c4b0;

/// Pas de jeu maximal par image.
///
/// La boucle principale (0x1c48c) cède d'ordinaire une image par pas (0xa40, D0 = 1). Mais quand un bouton est
/// pressé à l'entrée d'une scène ou à l'attract, une tâche auxiliaire (entrée 13 de la table, 0xee080) remet la
/// tâche principale prête à chaque passage de l'ordonnanceur : le jeu enchaîne ses pas sans attendre l'image
/// (« avance rapide »). Le nombre de pas par image vaut alors (durée de l'image) / (durée d'un pas) : 5 à 10 sur
/// l'arcade (mesuré à l'attract : 7-8, `examples/passages` ; en jeu : 5-7 ; entrée de scène : 9-10) mais 128 à
/// ×16, et il dépend de la durée des routines, donc diffère entre le code d'origine et les natives.
///
/// Écart volontaire avec le code d'origine surcadencé (pas avec l'arcade) : au plus `MAX_STEPS_PER_FRAME` pas
/// par image, quelle que soit la fréquence du 68000 (`cps2::Machine::step_cap`). Appliqué à l'identique par le
/// jeu (`install`) et par la machine de référence (`install_reference`), il rend ces phases déterministes.
pub const MAX_STEPS_PER_FRAME: u32 = 8;

/// Plafonne les pas de jeu par image (`DDSOM_STEP_CAP=n` : autre valeur, 0 = sans plafond, diagnostic).
fn cap_steps(m: &mut cps2::Machine) {
    let max = std::env::var("DDSOM_STEP_CAP").ok().and_then(|v| v.parse().ok()).unwrap_or(MAX_STEPS_PER_FRAME);
    m.step_cap = (max > 0).then(|| cps2::StepCap::new(GAME_STEP, max));
    m.patches = patches();
}

/// Correctifs communs au jeu et à la machine de référence (`cps2::Machine::patches`).
fn patches() -> Vec<cps2::Patch> {
    let mut v = vec![
        cps2::Patch { entry: player::scripted::END_BY_SCENE_ENTRY, apply: player::scripted::patch_end_by_scene },
        cps2::Patch { entry: scene::exit::SHARED_NEXT_ENTRY, apply: scene::exit::patch_shared_next },
    ];
    v.extend(sprites::EMPTY_FRAME_PATCHES);
    v
}

/// Active les routines natives (et le code recompilé) sur la machine.
pub fn install(m: &mut cps2::Machine) {
    m.executor = Some(run);
    m.idle = Some(idle);
    cap_steps(m);
}

/// Machine de référence de la validation : code d'origine seul, avec le même plafond de pas que `install`.
pub fn install_reference(m: &mut cps2::Machine) {
    ddsom_rc::install(m);
    m.idle = Some(idle);
    cap_steps(m);
}

/// L'ordonnanceur (0x932-0x95e) tourne à vide : aucune tâche prête et pas de nouvelle image, ou seule la tâche
/// du gel est prête et son prochain pas ne ferait rien (`taches::gel::attente_vide`).
pub fn idle(c: &Cpu, b: &Cps2Bus) -> bool {
    const TASKS: usize = 0x64 / 2; // FF0064 : 16 tâches de 16 octets, état dans l'octet haut
    const NEW_FRAME: usize = 0x164 / 2; // FF0164 (octet haut) : posé par le VBLANK, effacé au début d'un parcours
    (0x932..0x960).contains(&c.pc)
        && b.ram[NEW_FRAME] >> 8 == 0
        && ((0..16).all(|t| b.ram[TASKS + 8 * t] >> 8 < 4) || taches::gel::attente_vide(&b.ram))
}

static CHECK: AtomicBool = AtomicBool::new(false);
/// Compte les appels des routines natives hors mode contrôle (couverture).
static COUNT: AtomicBool = AtomicBool::new(false);

/// Mode contrôle : comparer tous les registres, pas seulement le contrat (diagnostic d'un contrat
/// incomplet ; beaucoup d'écarts sont alors sans conséquence).
static CHECK_ALL: AtomicBool = AtomicBool::new(false);

pub fn set_check_all(on: bool) {
    CHECK_ALL.store(on, Ordering::Relaxed);
}

pub fn set_count(on: bool) {
    COUNT.store(on, Ordering::Relaxed);
}

#[derive(Default, Clone)]
pub struct Stats {
    pub calls: u64,
    pub mismatches: u64,
    pub first: Option<String>,
    /// Dernier écart (outils de diagnostic).
    pub last: Option<String>,
    /// Cycles cumulés de l'original et de la native (sous-routines comprises) : calibrage des durées.
    pub original_cycles: u64,
    pub native_cycles: u64,
    /// Mode `CHECK_ALL` : nombre d'appels où chaque registre (D0-D7, A0-A7) diffère de l'original.
    pub regs: [u64; 16],
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
/// DDSOM_ADOPT_REGS=préfixe,... : les routines natives nommées ainsi rendent les registres de
/// l'original (exécuté sur une copie). Si un écart disparaît, l'une d'elles laisse un registre
/// qu'un appelant 68000 (ou le handler suivant) relit.
fn adopt_regs(n: &Native) -> bool {
    static PREFIXES: OnceLock<Vec<String>> = OnceLock::new();
    let p = PREFIXES.get_or_init(|| {
        std::env::var("DDSOM_ADOPT_REGS").map(|v| v.split(',').map(str::to_owned).collect()).unwrap_or_default()
    });
    !p.is_empty() && p.iter().any(|x| n.name.starts_with(x.as_str()))
}

fn adopt_mask() -> u32 {
    static MASK: OnceLock<u32> = OnceLock::new();
    *MASK.get_or_init(|| {
        std::env::var("DDSOM_ADOPT_MASK").ok().and_then(|v| u32::from_str_radix(&v, 16).ok()).unwrap_or(regs::ALL)
    })
}

/// Exécute la routine d'origine sur une copie de la machine jusqu'à son retour ; rend le CPU obtenu.
fn original(n: &Native, c: &Cpu, b: &Cps2Bus) -> Cpu {
    let (mut c2, mut b2) = (c.clone(), b.clone());
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
    c2
}

/// `DDSOM_CHECK_ONLY=préfixe,...` : seules les natives nommées sont contrôlées (les autres, dont celles
/// du noyau qui ne reviennent jamais à leur appelant, s'exécutent sans contrôle).
fn checked(n: &Native) -> bool {
    static ONLY: OnceLock<Option<Vec<String>>> = OnceLock::new();
    let only = ONLY.get_or_init(|| std::env::var("DDSOM_CHECK_ONLY").ok().map(|v| v.split(',').map(str::to_owned).collect()));
    only.as_ref().map_or(true, |p| p.iter().any(|p| n.name.starts_with(p.as_str())))
}

fn check(n: &Native, c: &mut Cpu, b: &mut Cps2Bus, cost: u64) {
    let pre = if std::env::var("DDSOM_TRACE_ORIG").map_or(false, |v| n.name == v) { Some((c.clone(), b.clone())) } else { None };
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
    let native_cycles = c.cycles - start;
    let mut diffs = Vec::new();
    // durée totale de la native, sous-routines d'origine appelées comprises
    if n.exact_cost && c.cycles - start != c2.cycles - start {
        diffs.push(format!("cycles {} au lieu de {}", c.cycles - start, c2.cycles - start));
    }
    let mut reg_diffs = [false; 16];
    let outputs = if CHECK_ALL.load(Ordering::Relaxed) { regs::ALL } else { n.outputs };
    for i in 0..8 {
        if outputs & regs::d(i as u32) != 0 && c.d[i] != c2.d[i] {
            diffs.push(format!("d{i}={:08x} au lieu de {:08x}", c.d[i], c2.d[i]));
            reg_diffs[i] = true;
        }
        // A7 (pile) doit toujours être rétabli
        if (outputs & regs::a(i as u32) != 0 || i == 7) && c.a[i] != c2.a[i] {
            diffs.push(format!("a{i}={:08x} au lieu de {:08x}", c.a[i], c2.a[i]));
            reg_diffs[8 + i] = true;
        }
    }
    let sr_mask = if outputs & regs::CCR != 0 { 0xffff } else { 0xff00 };
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
    if let (Some((mut c3, mut b3)), false) = (pre, diffs.is_empty()) {
        eprintln!("TRACE {} A0={:06x}: {}", n.name, c3.a[0], diffs.join(", "));
        let ret = mem::rd32(&mut b3, c3.a[7]);
        let sp_after = c3.a[7].wrapping_add(4);
        let mut k = 0;
        while !(c3.pc == ret && c3.a[7] == sp_after) && k < 3000 {
            eprintln!("  {:06x} d0={:08x} d1={:08x} d2={:08x} d5={:08x} d6={:08x} a1={:08x}", c3.pc, c3.d[0], c3.d[1], c3.d[2], c3.d[5], c3.d[6], c3.a[1]);
            c3.step(&mut b3);
            k += 1;
        }
    }
    let mut st = STATS.lock().unwrap();
    let s = st.entry(n.name).or_default();
    s.calls += 1;
    s.original_cycles += c2.cycles - start;
    s.native_cycles += native_cycles;
    for (n, &d) in reg_diffs.iter().enumerate() {
        s.regs[n] += d as u64;
    }
    if !diffs.is_empty() {
        if std::env::var("DDSOM_VERBOSE").is_ok() {
            eprintln!("[{}] pc entry {:06x}: {}", n.name, n.entry, diffs.join(", "));
        }
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
