//! The registry of the natives by original address: the game calls its routines directly and finds the object
//! handlers in its dispatch tables (`crate::actors::dispatch`); the tools find a native by the address that the
//! original code calls or jumps to.
//!
//! The natives of the game are the handlers of its dispatch tables, and here the routines of the game that only the
//! original code designates by address (the game calls them directly): memory only, actions of the characters,
//! common reactions, getting up after a reaction.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::actors::dispatch::{Dispatch, Handler, CLASSES};
use crate::actors::families::{self as f, cx::Gx};
use crate::actors::player::{self, characters::Slot, Ctx};
use crate::world::World;
use crate::screens::scene::screen;

use crate::*;

const fn memory(entry: u32, name: &'static str, run: fn(&mut World)) -> Native {
    Native { entry, name, run: Body::Memory(run), touches_sound: false, outputs: 0 }
}

/// The routines of the game that only the original code calls (by address): memory only.
const MEMORY: &[Native] = &[
    memory(0x1de7c, "sprites_clear_lists", services::sprites::clear_lists),
    // the registers that 0x68c leaves (D0 = read head and A4 after the entry transmitted) are those that VBLANK saved
    Native {
        entry: 0x68c,
        name: "sound_send_z80",
        run: Body::Memory(|b| {
            services::sound::send(b);
        }),
        touches_sound: true,
        outputs: 0,
    },
    memory(0x143da, "credits_read_coin_slots", screens::credits::read_coin_slots),
    memory(0x2d7aa, "config_defaults", services::config::defaults),
    memory(0x2d662, "config_validate", services::config::validate),
    memory(0x2d358, "config_variables", services::config::variables),
    // startup calls it by address; after "EEPROM ERROR", the task is blocked (`traps::block`): the return is then that
    // of no call
    memory(0x2d30a, "config_init", |b| {
        services::config::initialize(b);
    }),
    memory(0x2d9e, "players_clear", player::init::clear_all_players),
    memory(0x2df8, "players_clear_overlays", player::init::clear_sub_objects),
    memory(0x2e58, "player_init_1", |b| player::init::initialize(b, 0)),
    memory(0x2e6e, "player_init_2", |b| player::init::initialize(b, 1)),
    memory(0x2e86, "player_init_3", |b| player::init::initialize(b, 2)),
    memory(0x2e9e, "player_init_4", |b| player::init::initialize(b, 3)),
    memory(0x2f46, "player_join_new_1", |b| player::init::join_player(b, 0, player::init::Join::New)),
    memory(0x2f58, "player_join_new_2", |b| player::init::join_player(b, 1, player::init::Join::New)),
    memory(0x2f6c, "player_join_new_3", |b| player::init::join_player(b, 2, player::init::Join::New)),
    memory(0x2f80, "player_join_new_4", |b| player::init::join_player(b, 3, player::init::Join::New)),
    memory(0x3048, "player_join_continue_1", |b| player::init::join_player(b, 0, player::init::Join::Continue)),
    memory(0x305a, "player_join_continue_2", |b| player::init::join_player(b, 1, player::init::Join::Continue)),
    memory(0x306e, "player_join_continue_3", |b| player::init::join_player(b, 2, player::init::Join::Continue)),
    memory(0x3082, "player_join_continue_4", |b| player::init::join_player(b, 3, player::init::Join::Continue)),
    memory(0x3142, "player_join_refill_1", |b| player::init::join_player(b, 0, player::init::Join::Refill)),
    memory(0x3154, "player_join_refill_2", |b| player::init::join_player(b, 1, player::init::Join::Refill)),
    memory(0x3168, "player_join_refill_3", |b| player::init::join_player(b, 2, player::init::Join::Refill)),
    memory(0x317c, "player_join_refill_4", |b| player::init::join_player(b, 3, player::init::Join::Refill)),
    memory(0x19038, "video_copy_fixed_screens", services::video::copy_fixed_screens),
    memory(0xc58, "video_clear_objects", services::video::clear_objects),
    memory(0xc0c, "video_black_screen", services::video::black_screen),
    memory(0xca8, "video_clear_scroll1", |b| services::video::clear_layer(b, services::video::Layer::One)),
    memory(0xcc8, "video_clear_scroll2", |b| services::video::clear_layer(b, services::video::Layer::Two)),
    memory(0xce8, "video_clear_scroll3", |b| services::video::clear_layer(b, services::video::Layer::Three)),
    memory(0x1cf3c, "palette_white_pages", services::palette::white_out),
    memory(0x1c522, "game_phase0", core::game::phase0),
    memory(0x1c6a0, "game_phase12", |b| {
        core::game::phase12(b);
    }),
    // a handler never cedes control there
    memory(0x1c86e, "world_update", |b| {
        actors::update::update_world(b);
    }),
    memory(0x21c2a, "scene_phase_24", screens::scene::intermission::phase_24),
    memory(0xf2e5c, "scene_script", screens::scene::script::step),
    // resets the reader to the "load the script" state (scene change)
    memory(0xf2e56, "scene_script_reset", screens::scene::script::reset),
    // no more calls by address to these five clears (the scene entry calls `clear_longs`): the registers left by the
    // original (A4 and D6 of its loop) are not reproduced
    memory(0xf3130, "scene_exit_reset", |b| screen::clear_longs(b, screen::blocks::EXIT, 8)),
    memory(0xf7916, "scene_shop_reset", |b| screen::clear_longs(b, screen::blocks::SHOP, 5)),
    memory(0xf7ee0, "scene_transition_reset", |b| screen::clear_longs(b, screen::blocks::TRANSITION, 4)),
    memory(0xfc562, "ranking_reset", |b| screen::clear_longs(b, screen::blocks::RANKING, 4)),
    memory(0xf683c, "scene_clear_text", screen::clear_text),
    memory(0xe05d6, "demo_recorder_stop", screens::modes::demo::stop_recorder),
    memory(0xe03c6, "prompt_stop", screens::prompt::stop),
    memory(0xe03e4, "prompt_start", screens::prompt::launch),
    memory(0x1507a, "hud_forget_displayed", screens::hud::forget_displayed),
];

/// Native of an object handler without registers: what the dispatch loop passes as argument (A0, D7).
const fn handler(entry: u32, name: &'static str, run: fn(&mut World, &mut Dispatch)) -> Native {
    Native { entry, name, run: Body::Handler(run), touches_sound: false, outputs: regs::a(0) | regs::d(7) }
}

/// Native of a character action (`characters::routine`): the Rust function receives the player (A0) and the leftovers
/// D2-D5; no other register is read by the callers. It serves the differential validation of each action
/// (`fuzz_player`, `fuzz_couv`).
macro_rules! action {
    ($entry:literal, $name:literal, $slot:ident, $character:literal) => {
        action!($entry, $name, |g, p| player::characters::routine(Slot::$slot, $character)(g, p))
    };
    ($entry:literal, $name:literal, $f:expr) => {
        Native {
            entry: $entry,
            name: $name,
            run: Body::Handler(|b, a| Ctx::on_player(b, a, $f)),
            touches_sound: false,
            outputs: 0,
        }
    };
}

/// The handlers of the game that only the original code calls by address: actions of the characters (tables
/// 0x35918-0x35a48, one address per character), rare poses (`fuzz_player player_action`), reactions specific to a type
/// (`small::own_reaction`, from the reaction table of the enemy) and getting up after a common reaction (table
/// 0x3e000 of `reactions`).
const HANDLERS: &[Native] = &[
    action!(0x3e578, "fighter_jump", Jump, 0),
    action!(0x3e7e4, "fighter_attack", Attack, 0),
    action!(0x3f0ae, "fighter_special", Special, 0),
    action!(0x3f9bc, "fighter_dash", Action10, 0),
    action!(0x3fd08, "cleric_jump", Jump, 2),
    action!(0x3ff2a, "cleric_attack", Attack, 2),
    action!(0x406d4, "cleric_special", Special, 2),
    action!(0x41f22, "cleric_dash", Action10, 2),
    action!(0x42012, "elf_jump", Jump, 4),
    action!(0x4241c, "elf_attack", Attack, 4),
    action!(0x42b0c, "elf_special", Special, 4),
    action!(0x444ca, "elf_dash", Action10, 4),
    action!(0x445ba, "dwarf_jump", Jump, 6),
    action!(0x4491a, "dwarf_attack", Attack, 6),
    action!(0x45148, "dwarf_special", Special, 6),
    action!(0x45988, "dwarf_dash", Action10, 6),
    action!(0x35b18, "dwarf_roll", Action12, 6),
    action!(0x45a78, "mage_jump", Jump, 8),
    action!(0x45cfc, "mage_attack", Attack, 8),
    action!(0x4619e, "mage_special", Special, 8),
    action!(0x4879a, "mage_dash", Action10, 8),
    action!(0x488bc, "thief_jump", Jump, 10),
    action!(0x48d48, "thief_attack", Attack, 10),
    action!(0x49498, "thief_special", Special, 10),
    action!(0x49d46, "thief_dash", Action10, 10),
    action!(0x35b50, "thief_roll", Action12, 10),
    action!(0x36b88, "player_action_26", player::moves::curse_drop),
    action!(0x36cb8, "player_action_2c", player::moves::dust_puffs),
    action!(0x36f1a, "player_action_30", player::moves::box_recoil),
    handler(0x78ad2, "enemy_shake_a", |b, a| {
        f::small::own_reaction(&mut Gx::dispatch(b, a), 0x78ad2);
    }),
    handler(0x71680, "enemy_shake_b", |b, a| {
        f::small::own_reaction(&mut Gx::dispatch(b, a), 0x71680);
    }),
    handler(0x65236, "enemy_shake_c", |b, a| {
        f::small::own_reaction(&mut Gx::dispatch(b, a), 0x65236);
    }),
    handler(0x9a6d4, "boss_21_getup", |b, a| f::boss_21::hit::recovered(&mut Gx::dispatch(b, a))),
    handler(0x7bece, "boss_1_getup", |b, a| f::boss_1::hit::get_up(&mut Gx::dispatch(b, a))),
    handler(0x7a5ac, "boss_0_getup", |b, a| f::boss_0::hit::get_up(&mut Gx::dispatch(b, a))),
    handler(0x96ce0, "boss_17_getup", |b, a| f::boss_17::hit::recovered_tail(&mut Gx::dispatch(b, a))),
    handler(0x78c3c, "enemy_27_getup", |b, a| f::enemy_warmachine::hit::recover(&mut Gx::dispatch(b, a))),
    handler(0x653b2, "enemy_8_getup", |b, a| f::enemy_gargoyle::attacks::regroup(&mut Gx::dispatch(b, a))),
    handler(0x85490, "boss_6_getup", |b, a| f::boss_6::hit::recovered_tail(&mut Gx::dispatch(b, a))),
    handler(0x82ea0, "boss_4_getup", |b, a| f::boss_4::hit::resume(&mut Gx::dispatch(b, a))),
    handler(0x717ea, "enemy_18_getup", |b, a| f::enemy_ogre::hit::regroup(&mut Gx::dispatch(b, a))),
];

/// A common reaction routine (`reactions::ROUTINES`), called by `jsr (A1)` from the per-type tables.
fn reaction(b: &mut World, a: &mut Dispatch, entry: u32) {
    f::reactions::run_common(&mut Gx::dispatch(b, a), entry);
}

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
    static LIST: OnceLock<Vec<Native>> = OnceLock::new();
    LIST.get_or_init(|| {
        let mut list: Vec<Native> = MEMORY.iter().chain(HANDLERS).copied().collect();
        list.extend(f::reactions::ROUTINES.iter().map(|&entry| Native {
            entry,
            name: "enemy_reaction",
            run: Body::HandlerIn(reaction),
            touches_sound: false,
            outputs: 0,
        }));
        // the handlers of the dispatch tables (a handler may serve several types)
        for class in CLASSES {
            for &entry in class.handlers {
                if let Handler::Rust { run, native } = entry {
                    if !list.iter().any(|n| n.entry == native.entry) {
                        list.push(handler(native.entry, native.name, run));
                    }
                }
            }
        }
        list
    })
    .iter()
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
}
