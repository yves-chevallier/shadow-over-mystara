//! The registry of the natives by original address (tools only): the game calls its routines directly and finds the
//! object handlers in the dispatch tables (`dispatch`); the tools find a native by the address that the original code
//! calls or jumps to.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::*;

/// The natives of the game that only the original code calls (by address): memory only (`Body::Memory`).
const MEMORY: &[Native] = &[
    services::sprites::RESET_LISTS,
    hw::sound::SEND_TO_Z80,
    screens::credits::READ_COIN_SLOTS,
    core::config::DEFAULTS,
    core::config::VALIDATE,
    core::config::VARIABLES,
    core::config::INIT,
];

/// Index of the native routines by entry address. A binary sieve avoids the search for the vast majority of blocks.
struct Index {
    sieve: Vec<u64>,
    by_address: HashMap<u32, &'static Native>,
}

/// The native at the original address `pc`, if any (`DDSOM_SANS=prefix,...`: natives whose name starts with one of
/// the prefixes are left out, to search for a faulty routine).
pub(crate) fn lookup(pc: u32) -> Option<&'static Native> {
    static INDEX: OnceLock<Index> = OnceLock::new();
    let index = INDEX.get_or_init(|| {
        let without: Vec<String> = std::env::var("DDSOM_SANS")
            .map(|v| v.split(',').map(str::to_owned).collect())
            .unwrap_or_default();
        let by_address: HashMap<_, _> = all_natives()
            .filter(|n| !without.iter().any(|p| n.name.starts_with(p.as_str())))
            .map(|n| (n.entry, n))
            .collect();
        let mut sieve = vec![0u64; 0x100000 / 2 / 64];
        for &a in by_address.keys() {
            sieve[(a >> 7) as usize] |= 1 << (a >> 1 & 63);
        }
        Index { sieve, by_address }
    });
    let case = index.sieve.get((pc >> 7) as usize)?;
    if case >> (pc >> 1 & 63) & 1 == 0 {
        return None;
    }
    index.by_address.get(&pc).copied()
}

/// A native routine replaces the original code at address `pc`.
pub fn is_native(pc: u32) -> bool {
    lookup(pc).is_some()
}

/// All the native routines: those of the game and those for the original code only.
pub fn all_natives() -> impl Iterator<Item = &'static Native> {
    game_natives().chain(super::natives())
}

/// The natives of the game: register free (`Body::Memory`, `Body::Handler`), designated by the original address of the
/// routine (object handlers of the dispatch tables, actions of the players, routines that the original code calls).
pub fn game_natives() -> impl Iterator<Item = &'static Native> {
    // one source per line
    std::iter::empty()
        .chain(MEMORY)
        .chain(world::objects::NATIVES)
        .chain(actors::player::NATIVES)
        .chain(actors::player::init::JOIN)
        .chain(services::video::NATIVES)
        .chain(services::palette::NATIVES)
        .chain(services::scroll::NATIVES)
        .chain(services::camera_modes::NATIVES)
        .chain(world::terrain::NATIVES)
        .chain(actors::families::NATIVES)
        .chain(actors::families::natives())
        .chain(core::game::NATIVES)
        .chain(screens::scene::natives())
        .chain(screens::modes::natives())
        .chain(core::tasks::natives())
        .chain(services::aids::NATIVES)
        .chain(screens::hud::NATIVES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_inputs() {
        let mut views = std::collections::HashSet::new();
        for n in all_natives() {
            assert!(views.insert(n.entry), "{:06x} ({}) replaced twice", n.entry, n.name);
        }
    }

    /// Each handler of a dispatch table is a native of the game, at the address that the original table holds.
    #[test]
    fn dispatch_handlers_are_game_natives() {
        for class in core::dispatch::CLASSES {
            for handler in class.handlers {
                if let core::dispatch::Handler::Rust { native, .. } = handler {
                    assert!(game_natives().any(|n| n.entry == native.entry), "{:06x} ({})", native.entry, native.name);
                }
            }
        }
    }
}
