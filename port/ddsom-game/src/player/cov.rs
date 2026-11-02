//! Coverage of the Rust functions of the player core: counts the calls of each function that contains
//! `cov!();` when the `examples/player_coverage` tool enables it. Used to tell which states and actions
//! the scenarios exercise (hence which rewrites are validated).

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

static ON: AtomicBool = AtomicBool::new(false);
static HITS: Mutex<BTreeMap<&'static str, u64>> = Mutex::new(BTreeMap::new());

/// Counts a call (full name of the function, without the crate path).
pub fn hit(name: &'static str) {
    if ON.load(Ordering::Relaxed) {
        *HITS.lock().unwrap().entry(name).or_default() += 1;
    }
}

/// Enables counting.
pub fn enable() {
    ON.store(true, Ordering::Relaxed);
}

/// Number of calls of each function since activation.
pub fn report() -> Vec<(&'static str, u64)> {
    HITS.lock().unwrap().iter().map(|(k, v)| (*k, *v)).collect()
}

/// Sources of the player core, to also list the functions never called.
pub const SOURCES: &[(&str, &str)] = &[
    ("actions", include_str!("actions.rs")),
    ("characters::cleric", include_str!("characters/cleric/mod.rs")),
    ("characters::cleric::turning", include_str!("characters/cleric/turning.rs")),
    ("characters::cleric::prayers", include_str!("characters/cleric/prayers.rs")),
    ("characters::common", include_str!("characters/common/mod.rs")),
    ("characters::common::jumps", include_str!("characters/common/jumps.rs")),
    ("characters::common::strikes", include_str!("characters/common/strikes.rs")),
    ("characters::dwarf", include_str!("characters/dwarf.rs")),
    ("characters::elf", include_str!("characters/elf/mod.rs")),
    ("characters::elf::spells", include_str!("characters/elf/spells.rs")),
    ("characters::foes", include_str!("characters/foes.rs")),
    ("characters::fighter", include_str!("characters/fighter.rs")),
    ("characters::items", include_str!("characters/items.rs")),
    ("characters::magic_user", include_str!("characters/magic_user/mod.rs")),
    ("characters::magic_user::spells", include_str!("characters/magic_user/spells.rs")),
    ("characters::magic_user::conjurations", include_str!("characters/magic_user/conjurations.rs")),
    ("characters::thief", include_str!("characters/thief.rs")),
    ("checks", include_str!("checks.rs")),
    ("combos", include_str!("combos.rs")),
    ("effects", include_str!("effects.rs")),
    ("hit_reactions", include_str!("hit_reactions.rs")),
    ("hurt", include_str!("hurt/mod.rs")),
    ("hurt::throws", include_str!("hurt/throws.rs")),
    ("hurt::seizures", include_str!("hurt/seizures.rs")),
    ("init", include_str!("init.rs")),
    ("overlays", include_str!("overlays.rs")),
    ("scripted", include_str!("scripted.rs")),
    ("spawn", include_str!("spawn.rs")),
    ("update", include_str!("update.rs")),
    ("walk", include_str!("walk.rs")),
];

/// All the instrumented functions (`module::function`), from the sources.
pub fn instrumented() -> Vec<String> {
    let mut names = Vec::new();
    for (module, src) in SOURCES {
        let lines: Vec<&str> = src.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let t = line.trim_start();
            let t = t.strip_prefix("pub(super) ").or_else(|| t.strip_prefix("pub(crate) ")).unwrap_or(t);
            let Some(rest) = t.strip_prefix("pub fn ").or_else(|| t.strip_prefix("fn ")) else { continue };
            let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
            if lines.get(i + 1).map(|l| l.trim() == "cov!();").unwrap_or(false) {
                names.push(format!("{module}::{name}"));
            }
        }
    }
    names
}
