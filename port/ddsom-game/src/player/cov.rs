//! Couverture des fonctions Rust du noyau joueur : compte les appels de chaque fonction qui contient
//! `cov!();` quand l'outil `examples/player_coverage` l'active. Sert à dire quels états et actions
//! les scénarios exercent (donc quelles réécritures sont validées).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

static ON: AtomicBool = AtomicBool::new(false);
static HITS: Mutex<BTreeMap<&'static str, u64>> = Mutex::new(BTreeMap::new());
static ROM_CALLS: Mutex<BTreeMap<u32, u64>> = Mutex::new(BTreeMap::new());

/// Compte un appel (nom complet de la fonction, sans le chemin de crate).
pub fn hit(name: &'static str) {
    if ON.load(Ordering::Relaxed) {
        *HITS.lock().unwrap().entry(name).or_default() += 1;
    }
}

/// Compte un appel de routine d'origine (pas encore réécrite) par le noyau joueur.
pub fn rom_call(addr: u32) {
    if ON.load(Ordering::Relaxed) {
        *ROM_CALLS.lock().unwrap().entry(addr).or_default() += 1;
    }
}

/// Routines d'origine appelées depuis le noyau joueur : (adresse, appels).
pub fn rom_report() -> Vec<(u32, u64)> {
    ROM_CALLS.lock().unwrap().iter().map(|(k, v)| (*k, *v)).collect()
}

/// Active le comptage.
pub fn enable() {
    ON.store(true, Ordering::Relaxed);
}

/// Nombre d'appels de chaque fonction depuis l'activation.
pub fn report() -> Vec<(&'static str, u64)> {
    HITS.lock().unwrap().iter().map(|(k, v)| (*k, *v)).collect()
}

/// Sources du noyau joueur, pour lister aussi les fonctions jamais appelées.
pub const SOURCES: &[(&str, &str)] = &[
    ("actions", include_str!("actions.rs")),
    ("characters::cleric", include_str!("characters/cleric.rs")),
    ("characters::common", include_str!("characters/common.rs")),
    ("characters::elf", include_str!("characters/elf.rs")),
    ("characters::foes", include_str!("characters/foes.rs")),
    ("characters::fighter", include_str!("characters/fighter.rs")),
    ("characters::items", include_str!("characters/items.rs")),
    ("characters::magic_user", include_str!("characters/magic_user.rs")),
    ("checks", include_str!("checks.rs")),
    ("combos", include_str!("combos.rs")),
    ("effects", include_str!("effects.rs")),
    ("hit_reactions", include_str!("hit_reactions.rs")),
    ("hurt", include_str!("hurt.rs")),
    ("init", include_str!("init.rs")),
    ("overlays", include_str!("overlays.rs")),
    ("scripted", include_str!("scripted.rs")),
    ("update", include_str!("update.rs")),
    ("walk", include_str!("walk.rs")),
];

/// Toutes les fonctions instrumentées (`module::fonction`), d'après les sources.
pub fn instrumented() -> Vec<String> {
    let mut names = Vec::new();
    for (module, src) in SOURCES {
        let lines: Vec<&str> = src.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let t = line.trim_start();
            let Some(rest) = t.strip_prefix("pub fn ").or_else(|| t.strip_prefix("fn ")) else { continue };
            let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            if lines.get(i + 1).map(|l| l.trim() == "cov!();").unwrap_or(false) {
                names.push(format!("{module}::{name}"));
            }
        }
    }
    names
}
