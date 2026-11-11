//! World update (W1, `0x1c86e`): the players, then the ten object classes in the scene's order (see
//! `docs/boucle.md`). An object handler can make the task cede control in the middle of the update (fade to black
//! of the doors, fireworks): the pending calls of the update are then stages (`stages::World::World`), where the
//! task will resume (`core::game`).

use crate::world::World;

use crate::actors::dispatch::{self, Class};
use crate::hw::mem::{rd8, A5};
use crate::world::tasks::stages::World as Step;
use crate::world::tasks::table;
use crate::world::tasks::Flow;

/// Work RAM variables (offsets from A5).
mod ram {
    /// Scene number (signed byte).
    pub const SCENE: u32 = 0x34;
    /// Cutscene in progress (≠ 0).
    pub const CINEMATIC: u32 = 0x44;
}

/// One step of the world update.
#[derive(Clone, Copy)]
enum UpdateStep {
    /// Charge gauge, with its marker (D1).
    Gauge(u32),
    Players,
    PlayerParts,
    /// Dispatch of an object class.
    Objs(&'static Class),
    Cameras,
}
use UpdateStep::{Cameras, Gauge, Objs, PlayerParts, Players};

const G_PLAYER: u32 = 0x50_0001;
const G_ENEMY: u32 = 0x45_0002;
const G_SCENERY: u32 = 0x53_0003;

/// A step and the address of its call in the original code (`jsr`, 6 bytes; the last call of
/// each order is a `jmp`). The return address of a call is that of the resumption step ([`Step::World`]).
type Order = &'static [(u32, UpdateStep)];

/// Normal order (0x1c894): players and their shots, then bosses, enemies, enemy shots, effects, then
/// scenery, crates, items, cameras, system objects.
const ORDER_NORMAL: Order = &[
    (0x1c89a, Gauge(G_PLAYER)), (0x1c8a0, Players), (0x1c8a6, PlayerParts),
    (0x1c8ac, Objs(&dispatch::PSHL)), (0x1c8b2, Objs(&dispatch::PEFF)),
    (0x1c8be, Gauge(G_ENEMY)), (0x1c8c4, Objs(&dispatch::BOSS)), (0x1c8ca, Objs(&dispatch::ENEMY)),
    (0x1c8d0, Objs(&dispatch::ESHL)), (0x1c8d6, Objs(&dispatch::EFFCT)),
    (0x1c8e2, Gauge(G_SCENERY)), (0x1c8e8, Objs(&dispatch::SET)), (0x1c8ee, Objs(&dispatch::BOX)),
    (0x1c8f4, Objs(&dispatch::ITEM)), (0x1c8fa, Cameras),
    (0x1c900, Objs(&dispatch::SOBJ)),
];

/// Cutscene (`A5+0x44` ≠ 0, 0x1c906): the effects come first.
const ORDER_CINEMATIC: Order = &[
    (0x1c90c, Gauge(G_ENEMY)), (0x1c912, Objs(&dispatch::EFFCT)),
    (0x1c91e, Gauge(G_PLAYER)), (0x1c924, Players), (0x1c92a, PlayerParts),
    (0x1c930, Objs(&dispatch::PSHL)), (0x1c936, Objs(&dispatch::PEFF)),
    (0x1c942, Gauge(G_ENEMY)), (0x1c948, Objs(&dispatch::BOSS)), (0x1c94e, Objs(&dispatch::ENEMY)),
    (0x1c954, Objs(&dispatch::ESHL)),
    (0x1c960, Gauge(G_SCENERY)), (0x1c966, Objs(&dispatch::SET)), (0x1c96c, Objs(&dispatch::BOX)),
    (0x1c972, Objs(&dispatch::ITEM)), (0x1c978, Cameras),
    (0x1c97e, Objs(&dispatch::SOBJ)),
];

/// Scenes 0x45 to 0x52 (0x1c984): neither players nor enemies.
const ORDER_REDUCED: Order = &[
    (0x1c98a, Gauge(G_ENEMY)), (0x1c990, Objs(&dispatch::EFFCT)), (0x1c996, Objs(&dispatch::SET)),
    (0x1c99c, Cameras), (0x1c9a2, Objs(&dispatch::PEFF)),
    (0x1c9a8, Objs(&dispatch::SOBJ)),
];

/// Scene 8 (0x1c9ae): like the normal order, but the effects come after the cameras.
const ORDER_SCENE_8: Order = &[
    (0x1c9b4, Gauge(G_PLAYER)), (0x1c9ba, Players), (0x1c9c0, PlayerParts),
    (0x1c9c6, Objs(&dispatch::PSHL)), (0x1c9cc, Objs(&dispatch::PEFF)),
    (0x1c9d8, Gauge(G_ENEMY)), (0x1c9de, Objs(&dispatch::BOSS)), (0x1c9e4, Objs(&dispatch::ENEMY)),
    (0x1c9ea, Objs(&dispatch::ESHL)),
    (0x1c9f6, Gauge(G_SCENERY)), (0x1c9fc, Objs(&dispatch::SET)), (0x1ca02, Objs(&dispatch::BOX)),
    (0x1ca08, Objs(&dispatch::ITEM)), (0x1ca0e, Cameras), (0x1ca14, Objs(&dispatch::EFFCT)),
    (0x1ca1a, Objs(&dispatch::SOBJ)),
];

/// Update order according to the cutscene and the scene (signed comparisons on a byte).
fn world_order(b: &mut World) -> Order {
    let scene = rd8(b, A5 + ram::SCENE) as i8;
    if rd8(b, A5 + ram::CINEMATIC) != 0 {
        ORDER_CINEMATIC
    } else if scene >= 0x53 {
        ORDER_NORMAL
    } else if scene >= 0x45 {
        ORDER_REDUCED
    } else if scene == 8 {
        ORDER_SCENE_8
    } else {
        ORDER_NORMAL
    }
}

/// Updates all the objects: players, then the ten classes in the scene's order. `Flow::Yield`: an object
/// handler made the task cede control; it will resume at the step [`Step::World`], then at the continuation that the
/// caller stored in its pending calls before the call.
pub fn update_world(b: &mut World) -> Flow {
    let order = world_order(b);
    let order_index = ORDERS.iter().position(|&o| std::ptr::eq(o, order)).expect("update order") as u8;
    run_steps(b, order_index, 0)
}

/// The steps `from..` of the order `ORDERS[order_index]`.
pub(crate) fn run_steps(b: &mut World, order_index: u8, from: usize) -> Flow {
    let order = ORDERS[order_index as usize];
    for (i, &(_, step)) in order.iter().enumerate().skip(from) {
        match step {
            Gauge(marker) => crate::services::video::gauge_in_world(b, marker),
            // 0x35292
            Players => update_players(b),
            // 0x392e4: sub-objects of the players (front and back weapons)
            PlayerParts => update_player_parts(b),
            // last call: a `jmp`, the dispatch loop returns directly to the caller
            Objs(class) if i + 1 == order.len() => return class.update(b),
            Objs(class) => {
                table::call(b, Step::World { order_index, step: i as u8 });
                if class.update(b) == Flow::Yield {
                    return Flow::Yield;
                }
                table::pop_return(b);
            }
            // 0x33e20. Original glitch: the D4 of the scenery copy is reread by a following handler
            // (boss13_dragon, frame 7241): the leftover D4 of the game; the original's A0 (camera 2) is not
            Cameras => crate::actors::camera_modes::update_cameras_in_game(b),
        }
    }
    Flow::Done
}

/// The players' code `f` (`player::Ctx`), with the leftovers D2-D5 of the game, which it reads and writes by name.
fn with_players(b: &mut World, f: fn(&mut crate::actors::player::Ctx)) {
    let leftovers = b.leftovers;
    let mut g = crate::actors::player::Ctx { leftovers, ..crate::actors::player::Ctx::new(b) };
    f(&mut g);
    let leftovers = g.leftovers;
    b.leftovers = leftovers;
}

/// 0x35292: the players.
pub fn update_players(b: &mut World) {
    with_players(b, crate::actors::player::update::update_all);
}

/// 0x392e4: sub-objects of the players (front and back weapons).
pub fn update_player_parts(b: &mut World) {
    with_players(b, crate::actors::player::overlays::update);
}

/// 0x260ee: the frame's collisions (`combat`), with the leftovers D2-D5 of the game.
pub fn update_combat(b: &mut World) {
    let leftovers = b.leftovers;
    let mut w = crate::actors::combat::Combat { b, a2: 0, leftovers };
    crate::actors::combat::combat_update(&mut w);
    let leftovers = w.leftovers;
    b.leftovers = leftovers;
}

/// Length of an absolute `jsr`: the return address is that of the call site + 6.
pub(crate) const JSR: u32 = 6;

/// The four update orders.
const ORDERS: [Order; 4] = [ORDER_NORMAL, ORDER_CINEMATIC, ORDER_REDUCED, ORDER_SCENE_8];

/// Address the original resumes at after step `step` of the order `ORDERS[order_index]`: the one following its call.
pub(crate) const fn resume_address(order_index: u8, step: u8) -> u32 {
    ORDERS[order_index as usize][step as usize].0 + JSR
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each class is updated only once per frame, in all the complete orders.
    #[test]
    fn full_orders_without_duplicates() {
        for order in [ORDER_NORMAL, ORDER_CINEMATIC, ORDER_SCENE_8] {
            let mut entries: Vec<u32> =
                order.iter().filter_map(|(_, s)| if let Objs(class) = s { Some(class.entry) } else { None }).collect();
            entries.sort();
            entries.dedup();
            assert_eq!(entries.len(), 10);
        }
    }

    /// The call sites follow one another: a `jsr` (6 bytes), or a `move.l #,D1` (6 bytes) then the
    /// `jsr` of the gauge.
    #[test]
    fn increasing_sites() {
        for order in [ORDER_NORMAL, ORDER_CINEMATIC, ORDER_REDUCED, ORDER_SCENE_8] {
            for w in order.windows(2) {
                let gap = w[1].0 - w[0].0;
                assert!(gap == JSR || gap == 2 * JSR && matches!(w[1].1, Gauge(_)), "{:x}", w[1].0);
            }
        }
    }
}
