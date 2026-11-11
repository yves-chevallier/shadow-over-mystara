//! Scenery and animation objects (SET class, pool of 32 objects of 0x50 bytes, table 0x39eac), types 16 to 32. See
//! `docs/families/set_b.md`.
//!
//! SET objects are not in the depth-sorted lists by construction (the `reinsert` calls of the creations nevertheless
//! enter them there when they make them visible).

use crate::actors::families::cx::Bench;
use super::cx::{handlers, Cx};
use super::effects_creators::leftovers::{Pure, OnBench};
use super::eshl_creators;
use crate::hw::mem::{rd32, rd8, wr32, wr8, A5};
use crate::world::objects::Obj;
use crate::world::objects::pools;
use crate::world::objects::Current;
use crate::world::machine::Machine;
use crate::actors::families::set::vars;

/// Typed views of the SET B objects.
pub mod fields;
use crate::world::objects::player::Player;
use crate::actors::families::set_b::fields::DoorObj;

/// The four player records: A5+0x5de, 0x200 bytes each.
const PLAYERS: u32 = crate::world::ram::PLAYERS.base;
const PLAYER_SIZE: u32 = 0x200;

/// 0xc8312: SET object of the scene, of variant `variant`. Original glitch: the caller does not test the failure and
/// writes into the object that A1 leaves (`Pool::alloc_or_leftover`).
fn scene_set<R: Bench>(x: &mut Cx<'_, R>, variant: u8) -> Obj {
    match super::creators::scene_set_via(&mut OnBench(&mut x.c), x.b, variant) {
        Some(o) => o,
        None => pools::SET.alloc_or_leftover(x.b).expect_err("the SET pool was just found full"),
    }
}

mod pl {
    pub const ACTIVE: u32 = 0x00;
    pub const STATE: u32 = 0x04;
    pub const X: u32 = 0x08;
    pub const Z: u32 = 0x0c;
    pub const Y: u32 = 0x10;
    pub const HEALTH: u32 = 0x62;
}

fn players() -> impl Iterator<Item = Obj> {
    (0..4).map(|i| Obj(PLAYERS + i * PLAYER_SIZE))
}

// ---------------------------------------------------------------------------------------------
// Type 16 (0xd4110): trampoline (h)
// ---------------------------------------------------------------------------------------------

/// 0xd40e8, routine of the appearance scripts (scenes 14 and 109): releases the trampoline lock (`A5+0x46`) and creates
/// the thirteen trampolines of the scene, SET 16 of variants 13 to 1; stops if the pool is full. Returns the D5 that
/// the original leaves (loop counter: `moveq #12,D5`, high word zero; 0xFFFF after the last), a leftover reread by the
/// handlers (`dispatch::Leftovers`); A1 and D6 of the allocation are reread by nobody.
pub(crate) fn spawn_bumpers(b: &mut crate::world::World) -> u32 {
    crate::hw::mem::wr16(b, A5 + vars::CAMERA_FOCUS, 0);
    for counter in (0..13u32).rev() {
        let Some(o) = pools::SET.alloc(b) else { return counter };
        o.set_u8(b, Obj::ACTIVE, 1);
        o.set_u8(b, Obj::KIND, 0x10);
        o.set_u8(b, Obj::VARIANT, counter as u8 + 1);
    }
    0xffff
}

/// Trampoline / bumper (h): a contact zone (boxes read from the ROM according to `+0x2e` and bytes `+0x38..+0x3a`)
/// that, when a player touches it while pressing the right key, launches the player into the air (hardcoded speeds per
/// trampoline type `+0x42`, player state 0x2002400 or 0x2002402). Only one player at a time: `A5+0x46` remembers the
/// object holding the lock and `+0x14` the delay before releasing it. As long as a living player is out of range of the
/// two presence boxes, the object does nothing.
mod bumper;

/// Work RAM variables (A5+): camera abscissa and ordinate, secondary camera.
mod ram {
    pub const CAMERA_X: u32 = 0x506;
    pub const CAMERA_Y: u32 = 0x50a;
    pub const CAMERA2_Y: u32 = 0x55a;
}

/// Freezes the game and darkens the screen: `A5+0x490` = 1, `A5+0x48e` = 0xc, `A5+0x48f` = `flag` steps, palettes
/// whitened (0x1686) then fade task 0x14de (`services/fading.rs`).
fn show_message<R: Bench>(x: &mut Cx<'_, R>, flag: u8) {
    x.set_a5_u16(vars::FADE_DELAY, 1);
    x.set_a5_u8(vars::FADE_PAGES, 0xc);
    x.set_a5_u8(vars::FADE_STEPS, flag);
    crate::services::palette::full_brightness(x.b);
    start_fade_task(x, FADE_TO_BLACK_TASK);
}

/// Fade-to-black task (steps of state `A5+0x48f`).
const FADE_TO_BLACK_TASK: crate::world::tasks::Task = crate::world::tasks::Task::Fading(crate::world::tasks::stages::Fade::DescentSteps);
/// Slot of the fade task (offset in the task table).
const FADE_TASK_SLOT: u32 = 0x10;

/// Creates the fade task `entry` (0x9b6, D0 = 0x10, A0 = the entry).
fn start_fade_task<R: Bench>(x: &mut Cx<'_, R>, task: crate::world::tasks::Task) {
    crate::world::tasks::traps::create_task(x.b, FADE_TASK_SLOT, task);
}

// ---------------------------------------------------------------------------------------------
// Type 17 (0xd466a): boss arrival sequence (h)
// ---------------------------------------------------------------------------------------------

/// Sequencer of the boss arrival cutscene (h): a series of 13 steps (`+5`) that wait for the camera to reach an
/// abscissa (0x100, 0x180, 0x200, 0x2b0) or for a countdown (`+0x42`) to run out, and each time launch sounds, messages
/// (0x2824, 0x2860, 0x289c, 0xeda26, 0xeda66...) and random explosions; the last step creates the boss (BOSS object of
/// type 2).
mod boss_intro;

// ---------------------------------------------------------------------------------------------
// Type 18 (0xd4950): vertical scroll toward a target
// ---------------------------------------------------------------------------------------------

/// Makes the secondary camera descend one pixel per frame down to the height `+0x42`, then frees the linked object
/// (`+0x46`) and notifies routine 0x2824.
mod scroll_to;

// ---------------------------------------------------------------------------------------------
// Type 19 (0xd49a2): enemy wave generator (h)
// ---------------------------------------------------------------------------------------------

/// Wave generator (h): it triggers when a living player comes within range (0xc0 in X, 0x30 in depth), then reads a ROM
/// script (entries of 0x14 bytes, one list per variant) and creates the described objects (enemies, bosses or scenery)
/// when the game is in play (`A5+0x2c` = 0xc), keeping the count of the objects still alive (`+0x2a`, which these
/// objects decrement at their death by finding their generator in `+0x46`).
///
/// Script entry: +0 word (> 0: delay before the next; < 0: wait until the number of living objects is at most the low
/// byte, bit 7 of this byte: wait until none lives), +2 minimum rank, +4/+6/+8 position (X, Z, Y), +0xa minimum number
/// of players, +0xb creation routine (byte index in table 0xd4b92), +0xc type, +0xd variant, +0xe/+0xf parameters,
/// +0x10 word `+0x42`, +0x12 absolute position if non-zero (otherwise relative to the camera). A word 0x8000 ends the
/// script.
mod wave_spawner;

// ---------------------------------------------------------------------------------------------
// Type 20 (0xd5f5a): health bar
// ---------------------------------------------------------------------------------------------

/// Health bar of a player (or of two players together if `+3`), drawn in three columns of 44 cells of background layer
/// 1 (GFX RAM 0x91215c, 0x912160, 0x912164; 0x80 bytes per line). The bar fills in pieces of 8 hit points: the
/// displayed life (`+0x2a`) catches up with the real life (`+0x32`, sum of `+0x62` of the player) by one point per
/// frame, advancing the write cursor (`+0x22`) and its sub-step (`+0x26`). Beyond 0x160 points the bar moves to the
/// next column. `+0x30` = maximum life (empty bar on the right), `+0x2c` / `+0x27` = length of the background in whole
/// pieces and remainder.
mod life_bar;

// ---------------------------------------------------------------------------------------------
// Type 21 (0xd6372): continue countdown
// ---------------------------------------------------------------------------------------------

/// Display of the countdown of a player who can continue (h): three digits (`+0x14`, `+0x15`, `+0x16`, from 0:00 to
/// 9:59 (h)) drawn in background layer 1 (0x912b38) and counted down by the second (`+0x17` counts 60 frames); the
/// "ready" inscription blinks beside it (rows of constant cells at 0x912830). At zero, the players still alive are put
/// to death. Variant 1: version without count (invitation only), variant 2: does not depend on `A5+0x50`. States: 0
/// initialization (palette 0x91c420), 2 counting, 4 end of game (final inscription at 0x9127b0), 6 release. The sound
/// script 0xd6922 announces the display, 0xd694e its removal.
mod continue_countdown;

// ---------------------------------------------------------------------------------------------
// Type 22 (0xd69c2): rank adjustment at the end of a zone
// ---------------------------------------------------------------------------------------------

/// Adjusts the difficulty rank (A5+0x7f92) on arrival in a zone (h): from the lowest life level (`+0x62`) among the
/// present players, classed into four categories (<= 0x10, <= 0x30, <= 0x50, beyond), it reads a variation from a ROM
/// table (one row of 4 words per number of players, one table per variant) and brings it back into the interval
/// [minimum, maximum]. It acts only once, then frees itself.
mod rank_adjust;

// ---------------------------------------------------------------------------------------------
// Type 23 (0xd6c5c): zone exit door (h)
// ---------------------------------------------------------------------------------------------

/// Zone exit door (h): each living player who stays motionless on the ground in front of the door (X >= 0x1e0, within
/// 0x20 in depth), with key 0 held down (byte A5+number), for more than 8 frames has "entered": it is put in state
/// 0x2002006 and an effect (EFFCT 0x2f) is created for the first one. When all the living players (or dead ones who
/// remain) have entered, the door goes to state 4: level change (`A5+0x34`) or, at the last zone, end of game. Bits of
/// `+0x17`: players entered; of `+0x1a`: players already counted; `+0x16`: their number; `+0x42..+0x45`: waiting
/// counters of each player; `A5+0x52`: active door.
mod exit_gate;

// ---------------------------------------------------------------------------------------------
// Type 25 (0xd801e): volley of projectiles when the camera moves back (h)
// ---------------------------------------------------------------------------------------------

/// When the game is in play, the camera is in the left half of the level (X <= 0x180) and it has moved back since the
/// previous frame (the player turns around), the object launches three enemy shots (ESHL of type 0x12) at the right of
/// the screen, at the depth of a randomly drawn player (offsets -0x18, +0x18, 0). It waits 0x60 frames (`+8`) between
/// two volleys.
mod rearguard;

// ---------------------------------------------------------------------------------------------
// Type 26 (0xd810a): hunter of motionless players (h)
// ---------------------------------------------------------------------------------------------

/// Penalizes players who stay motionless (h): for each living player whose position (X + its high fractional part) does
/// not change for `+0x1c` frames, it creates pursuers (routine 0xa44bc, three ESHL of type 0x33), within a limit of
/// active ones that depends on the number of players. The delay `+0x1c` is drawn from a ROM table according to the
/// rank. The order in which the four players are examined is drawn at random among four permutations.
mod idle_hunter;

// ---------------------------------------------------------------------------------------------
// Types 27 and 29: passage doors (h)
// ---------------------------------------------------------------------------------------------

/// Fields of a dispatch loop object common to all the pools.
mod obj {
    pub const SUSPEND: u32 = 0x40;
    pub const SUSPEND_PHASE: u32 = 0x41;
    pub const STATE: u32 = 0x4;
}

/// Runs `run` of the handler with another object as the current object (recursive `bsr` call of the original, which
/// puts A0 on the other object).
pub(super) fn run_on<R: Bench>(x: &mut Cx<'_, R>, other: Obj, run: fn(&mut Cx<'_, R>)) {
    let me = x.o;
    x.o = other;
    run(x);
    x.o = me;
}

/// Suspends the objects of a pool (counter `+0x40` + 1, phase reset to zero).
pub(super) fn suspend<R: Bench>(x: &mut Cx<'_, R>, pool: &pools::Pool) {
    for o in pool.iter() {
        if o.u8(x.b, Obj::ACTIVE) != 0 {
            let v = o.u8(x.b, obj::SUSPEND).wrapping_add(1);
            o.set_u8(x.b, obj::SUSPEND, v);
            o.set_u8(x.b, obj::SUSPEND_PHASE, 0);
        }
    }
}

/// Wakes up the objects of a pool; those whose suspension counter becomes negative are deleted (state 6).
pub(super) fn resume<R: Bench>(x: &mut Cx<'_, R>, pool: &pools::Pool) {
    resume_with(x, pool, |_, _| ());
}

pub(super) fn resume_with<'a, R: Bench>(x: &mut Cx<'a, R>, pool: &pools::Pool, mut when_awake: impl FnMut(&mut Cx<'a, R>, Obj)) {
    for o in pool.iter() {
        if o.u8(x.b, Obj::ACTIVE) == 0 {
            continue;
        }
        let n = o.u8(x.b, obj::SUSPEND).wrapping_sub(1);
        o.set_u8(x.b, obj::SUSPEND, n);
        if (n as i8) < 0 {
            o.set_u8(x.b, obj::STATE, 6);
            o.set_u8(x.b, obj::SUSPEND, 0);
            o.set_u8(x.b, obj::SUSPEND_PHASE, 0);
        } else {
            when_awake(x, o);
        }
    }
}

/// Variables (A5-0x55d0 and A5-0x55cc) stored with the camera.
const SCROLL_STATE: u32 = 0xff_2a30;
const STAGE_FLAG: u32 = 0xff_2a34;

/// Stores in the door the state of the camera and of the scroll (fields `+0x30..+0x4a`).
pub(super) fn save_camera<R: Bench>(x: &mut Cx<'_, R>) {
    for (src, dst) in [(0x506, 0x30), (0x50a, 0x34), (0x556, 0x38), (0x55a, 0x3c)] {
        let v = x.a5_u32(src);
        x.set32(dst, v);
    }
    let v = rd32(x.b, SCROLL_STATE);
    x.set32(DoorObj::SCROLL_STATE_SAVED, v);
    let v = x.a5_u16(vars::SCROLL_MODE);
    x.set16(DoorObj::SCROLL_MODE_SAVED, v);
    let v = x.a5_u16(vars::SCROLL_STOP);
    x.set16(DoorObj::SCROLL_STOP_SAVED, v);
    let v = rd8(x.b, STAGE_FLAG);
    x.set8(DoorObj::STAGE_FLAG_SAVED, v);
    let v = x.a5_u16(vars::MUSIC);
    x.set16(DoorObj::MUSIC, v);
}

/// Restores the camera and the scroll stored by `save_camera`.
pub(super) fn restore_camera<R: Bench>(x: &mut Cx<'_, R>) {
    for (dst, src) in [(0x506, 0x30), (0x50a, 0x34), (0x556, 0x38), (0x55a, 0x3c)] {
        let v = x.u32(src);
        x.set_a5_u32(dst, v);
    }
    let v = x.a5_u32(vars::CAMERA_X);
    x.set_a5_u32(vars::CAMERA_X_PREV, v);
    let v = x.a5_u32(vars::CAMERA_Y);
    x.set_a5_u32(vars::CAMERA_Y_PREV, v);
    let v = x.a5_u16(vars::CAMERA_ALT_X);
    x.set_a5_u16(vars::CAMERA_ALT_X_PREV, v);
    let v = x.a5_u16(vars::CAMERA_ALT_Y);
    x.set_a5_u16(vars::CAMERA_ALT_Y_PREV, v);
    let v = x.a5_u16(vars::CAMERA_X_PREV) & !0x1f;
    x.set_a5_u16(vars::CAMERA_X_PREV, v);
    let v = x.a5_u16(vars::CAMERA_ALT_X_PREV) & !0x1f;
    x.set_a5_u16(vars::CAMERA_ALT_X_PREV, v);
    let v = x.a5_u16(vars::CAMERA_Y_PREV).wrapping_sub(0x160);
    x.set_a5_u16(vars::CAMERA_Y_PREV, v);
    let v = x.a5_u16(vars::CAMERA_ALT_Y_PREV).wrapping_sub(0x160);
    x.set_a5_u16(vars::CAMERA_ALT_Y_PREV, v);
    let v = x.u32(DoorObj::SCROLL_STATE_SAVED);
    wr32(x.b, SCROLL_STATE, v);
    let v = x.u16(DoorObj::SCROLL_MODE_SAVED);
    x.set_a5_u16(vars::SCROLL_MODE, v);
    let v = x.u16(DoorObj::SCROLL_STOP_SAVED);
    x.set_a5_u16(vars::SCROLL_STOP, v);
    let v = x.u8(DoorObj::STAGE_FLAG_SAVED);
    wr8(x.b, STAGE_FLAG, v);
}

/// Puts the players back in play after a passage (state, flags, delays); those that were beyond state 2 are marked dead
/// (0x402).
pub(super) fn reset_players_after_exit<R: Bench>(x: &mut Cx<'_, R>) {
    for p in players() {
        if p.u8(x.b, pl::ACTIVE) == 0 || p.u16(x.b, pl::HEALTH) == 0 {
            continue;
        }
        p.set_u8(x.b, pl::ACTIVE, 1);
        let before = p.u8(x.b, pl::STATE);
        p.set_u32(x.b, pl::STATE, 0);
        let v = p.u8(x.b, Player::FLAGS) & !2;
        p.set_u8(x.b, Player::FLAGS, v);
        for off in [0x66, 0x15c, 0x8a, 0x10f] {
            p.set_u8(x.b, off, 0);
        }
        if (before as i8) > 2 {
            let v = p.u8(x.b, Player::FLAGS) | 2;
            p.set_u8(x.b, Player::FLAGS, v);
            p.set_u16(x.b, pl::STATE, 0x402);
        }
    }
}

/// Sets the players walking again on arrival at an exit door (state 1, delay 0x20).
fn release_players<R: Bench>(x: &mut Cx<'_, R>) {
    for p in players() {
        if p.u8(x.b, pl::ACTIVE) == 0 {
            continue;
        }
        p.set_u8(x.b, pl::ACTIVE, 1);
        let v = p.u8(x.b, Player::FLAGS) & !2;
        p.set_u8(x.b, Player::FLAGS, v);
        for off in [0x66, 0x15c, 0x8a, 0x10f] {
            p.set_u8(x.b, off, 0);
        }
        p.set_u8(x.b, Player::INVULN, 0x20);
    }
}

/// Is player `p` motionless in front of the door? Counts the frames (byte `counter`). Conditions: nothing in progress
/// (`+0x66`, `+0x15c`, `+0x8a`), key `bit` of `input`, state 2 (walking or stopped), on the ground, within 0x10 in X
/// and 8 in depth (except `+0x1b`).
fn at_gate<R: Bench>(x: &mut Cx<'_, R>, p: Obj, input: u32, counter: u32) {
    let busy = p.u8(x.b, Player::SLOW).wrapping_add(p.u8(x.b, Player::SPECIAL_ACTIVE)).wrapping_add(p.u8(x.b, Player::SLOW_8A));
    let bit = x.u8(DoorObj::KEY) as u32 & 31;
    let state = p.u32(x.b, pl::STATE) & 0xffff_ff00;
    let dx = p.u16(x.b, pl::X).wrapping_sub(x.u16(Obj::X)).wrapping_add(0x10);
    let dy = p.u16(x.b, pl::Y).wrapping_sub(x.u16(Obj::Y)).wrapping_add(8);
    let aligned = dx <= 0x20 && (x.u8(DoorObj::IGNORE_DEPTH) != 0 || dy <= 0x10);
    if busy == 0
        && input >> bit & 1 != 0
        && p.u8(x.b, pl::STATE) == 2
        && matches!(state, 0x0200_0000 | 0x0200_0400)
        && p.u16(x.b, pl::Z) == 0
        && aligned
    {
        x.add8(counter, 1u8);
    } else {
        x.set8(counter, 0u8);
    }
}

/// Passage door between zones (type 27, h): like `exit_gate`, but the passage leads to another zone or back from it
/// (suspension of the objects of the current zone in `+0x40`, camera saved in `+0x30..+0x4a`), with a door, of variant
/// `+3`: 0 entry into the hidden zone, 1 return. The parameters of each variant (position, key `+0x46`, mode `+0x48`,
/// arrival level `+0x4f`, `+0x1b`) come from 16-byte rows of table 0xd84e8.
mod door_a;

/// Three-stage zone door (type 28, h): when the camera reaches X = 0x1c0 it blocks the scroll (`A5+0x51e` = 4), creates
/// a SET event (type 0x3f) and waits for it to finish (`+0x22` of that event); it then launches the invitation (SOBJ
/// 11) and a delay of 0x78 frames, then frees the scroll and launches the end-of-zone routines (0xa462c, 0xe55a2,
/// 0xe13b0).
mod zone_end;

/// Passage door (type 29, h): variant of door 27 that shifts all the scenery (`SET`, `SOBJ`, `EFFCT`, `BOX`, `ITEM`,
/// `BOSS`, `ESHL`) instead of three pools, and puts the players back on a fixed marker (table 0xd9052). Same fields
/// (`+0x42..+0x45` counters, `+0x16` number entered, `+0x17` bits entered, `+0x30..` saved camera).
pub(super) mod door_b;

// ---------------------------------------------------------------------------------------------
// Type 30 (0xd9424): end of game (continue screen)
// ---------------------------------------------------------------------------------------------

/// When `A5+0x42` (end of game) is raised, plays the end music (routine 0xa8ae8, D0 = 7), waits 0x80 frames then
/// launches the per-component fade task 0x1ccf8 and frees itself.
mod game_over;

// ---------------------------------------------------------------------------------------------
// Type 31 (0xd9492): waiting message in a long level (h)
// ---------------------------------------------------------------------------------------------

/// Level timer (h): counts frames as long as a player is in play (`+0x42`) and triggers at 0x258, 0x4b0 and 0x708
/// frames three messages (speech + effects: routines 0xa66c0, 0x2824, 0x2860, 0x289c, 0xeda26, 0xeda66); if the camera
/// advances by more than 0xa00 since the start (`+0x1e`) it cuts short and goes directly to the last step.
mod level_clock;

// ---------------------------------------------------------------------------------------------
// Type 32 (0xd96d0): test selector (h)
// ---------------------------------------------------------------------------------------------

/// Test selector (h): with player 1's keys (bytes A5+2 pressed, A5+8 previous), button 5 increases a number (0 to 0x32,
/// `+0x2f`), button 6 decreases it, button 4 confirms it (routine 0xcc352 on the first player). The number is displayed
/// as two hexadecimal digits in the background (0x912bc0).
mod test_selector;

// ---------------------------------------------------------------------------------------------
// Type 24 (0xd7056): dialogue window
// ---------------------------------------------------------------------------------------------

/// Dialogue window (h): frame drawn in background layer 1 (GFX RAM 0x912000, 0x80 bytes per line) that opens, receives
/// a text, waits for a key then closes. The variant `+3` designates, through table 0x1d78d0, a window definition in the
/// ROM (position, size, text `+0x24`, kind `+0x2f`...). The kind (24 values) sets the initialization (palettes, choice
/// of another text according to the number of players...) and the construction of the text's token list (words in the
/// buffer 0xff2072, reread by the text engine through the cell `A5+0x5c6+ (+0x38)`). Special tokens: 0 end, negatives =
/// commands, 0x7000 to 0x7009 = substitutions (number of players, player names...).
///
/// States: 0 opening, 2 life (substates `+5`: 0 frame opening, 2 unfolding, 4 waiting for a key, 6 closing), 4 and 6
/// release.
mod dialog;

handlers! {
    SET_16 0xd4110 "set_16" bumper::run,
    SET_24 0xd7056 "set_24" dialog::run,
    SET_30 0xd9424 "set_30" game_over::run,
    SET_31 0xd9492 "set_31" level_clock::run,
    SET_32 0xd96d0 "set_32" test_selector::run,
    SET_27 0xd8458 "set_27" door_a::run,
    SET_28 0xd8ce4 "set_28" zone_end::run,
    SET_29 0xd8dce "set_29" door_b::run,
    SET_25 0xd801e "set_25" rearguard::run,
    SET_26 0xd810a "set_26" idle_hunter::run,
    SET_22 0xd69c2 "set_22" rank_adjust::run,
    SET_23 0xd6c5c "set_23" exit_gate::run,
    SET_20 0xd5f5a "set_20" life_bar::run,
    SET_21 0xd6372 "set_21" continue_countdown::run,
    SET_17 0xd466a "set_17" boss_intro::run,
    SET_19 0xd49a2 "set_19" wave_spawner::run,
    SET_18 0xd4950 "set_18" scroll_to::run,
}
