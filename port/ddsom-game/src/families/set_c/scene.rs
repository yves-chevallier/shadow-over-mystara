//! SET type 2 (0xbc814): the scene controller (h).
//!
//! There is one per level (created by type 8, `scene_spawner`). Its variant `+3` chooses an *event script* in ROM
//! (pointer table 0xbd276) that makes the enemies, objects and scenery of the scene appear over time. Many variants
//! have no useful script: they are markers that open or close a scene (intertitle, curtain, fade, players' exit...).
//!
//! State machine (`+4`):
//! * 0: initialization. Loads the variant's script, calls 0xd9778, then a byte table
//!   (0xbc880) indexed by the variant gives the start action (12 actions, see `start`).
//! * 2: event script interpreter (see `script`), active at level 4 or in phase 0xc.
//! * 4: end of scene: waits until all the created objects have disappeared, then a byte table
//!   (0xbcba8) gives the end action (16 actions, see `end_action`).
//! * 6: release of the object.
//!
//! Format of a script entry (0x14 bytes, end = word 0x8000): `+0` word: wait before the next entry (bit 15 clear: that
//! many frames + 1; bit 15 set: until the number of living objects created by the script drops to the low byte, whose
//! bit 7 additionally requests an external unlock of `+0x2e`), `+2` word: maximum rank, `+4/+6/+8`: X, Z, Y, `+0xa`:
//! minimum difficulty, `+0xb`: offset in the allocator table 0xbcb4e (object class), `+0xc`: type, `+0xd`: variant,
//! `+0xe`: `+0x2f`, `+0xf`: `+0x2c`, `+0x10`: word `+0x42`, `+0x12`: non-zero = absolute coordinates (otherwise
//! camera-relative).

use crate::families::cx::Bench;
use crate::objects::Current;
use crate::families::cx::Cx;
use crate::mem::{rd16, rd8, wr16, wr32, wr8, A5};
use crate::objects::pools::{BOSS, EFFCT, ENEMY, ESHL, ITEM, PEFF, SET, SOBJ};
use crate::objects::Obj;
use crate::objects::pools;

/// Controller fields.
mod f {
    /// Address of the next script entry (long).
    pub const CURSOR: u32 = 0x20;
    /// Frames remaining before the end of the scene (the script stops below zero).
    pub const TIMER: u32 = 0x24;
    /// Frames to wait before the next entry (-1: wait by object count).
    pub const WAIT: u32 = 0x26;
    /// Number of living objects tolerated before the next entry (-1: wait in frames).
    pub const LIVE_LIMIT: u32 = 0x18;
    /// Number of objects created by the script and still alive (word).
    pub const LIVE: u32 = 0x2a;
    /// Byte: waits for the external unlock of the entry.
    pub const WAIT_FLAG: u32 = 0x2e;
    /// Byte: the script is finished or interrupted.
    pub const DONE: u32 = 0x2c;
    /// Byte: suspension of the script by an external entry.
    pub const HOLD: u32 = 0x2d;
}

/// Work RAM variables (offsets from A5).
mod a5 {
    
    pub const PHASE: u32 = 0x2c;
    /// Current level.
    pub const STAGE: u32 = 0x34;
    /// Difficulty (0: the controller does not work).
    pub const DIFFICULTY: u32 = 0x35;
    pub const USE_ALT_CAMERA: u32 = 0x36;
    pub const CAMERA: u32 = 0x4fe;
    pub const ALT_CAMERA: u32 = 0x54e;
    /// Non-zero: the scene is frozen (the scripts wait).
    pub const FROZEN: u32 = 0x42;
    pub const SCENE_LATCH: u32 = 0x43;
    pub const PLAYERS_LOCK: u32 = 0x4a;
    /// Forbidden to launch the boss.
    pub const NO_BOSS: u32 = 0x50;
    /// Number of active controllers.
    pub const SCENES: u32 = 0x51;
    pub const TRANSITION_FLAG: u32 = 0x58;
    pub const SCENE_SYNC: u32 = 0x5f;
    pub const TRANSITION: u32 = 0x51e;
    /// Rank (dynamic difficulty), word.
    pub const RANK: u32 = 0x7f92;
}

/// Byte of the background screen at A5 - 0x5b8e.
const PARALLAX_FLAG: u32 = 0xff2472;
/// Players: four records of 0x200 bytes.
const PLAYERS: u32 = A5 + 0x5de;
const PLAYER_STRIDE: u32 = 0x200;

// ROM tables.
/// Pointers to the event scripts, one long per variant.
const SCRIPT_TABLE: u32 = 0xbd276;
/// Start action per variant (bytes).
const START_ACTION: u32 = 0xbc880;
/// End action per variant (bytes).
const END_ACTION: u32 = 0xbcba8;
/// Allocators of the class of the created object (longs; index = byte `+0xb` of the entry).
const ALLOC_TABLE: u32 = 0xbcb4e;
/// "Pickup objects" class in this table: the object is not counted in `LIVE`.
const ITEM_CLASS: u8 = 0x10;
/// Curtain: 6-byte records (palette offset, GFX RAM address).
const CURTAIN_TABLE: u32 = 0xbcea4;
const CURTAIN_PALETTE: u32 = 0xbcfc6;

// Engine routines called.
const PREPARE_EFFECTS: crate::sound::Sound = crate::sound::CMD_322_422;
const FADE_OUT: crate::sound::Sound = crate::sound::SCENE_MUSIC;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x40) != 0 || x.a5_u8(a5::DIFFICULTY) == 0 {
        return;
    }
    match x.u8(0x4) {
        0 => init(x),
        2 => script(x),
        4 => finish(x),
        6 => x.free(&pools::SET),
        _ => {}
    }
}

/// Allocates and activates an object of type `kind` (and variant `variant`).
fn spawn<R: Bench>(x: &mut Cx<'_, R>, pool: &pools::Pool, kind: u8, variant: Option<u8>) -> Option<Obj> {
    let o = x.alloc(pool)?;
    o.set_u8(x.b, 0, 1);
    o.set_u8(x.b, 2, kind);
    if let Some(v) = variant {
        o.set_u8(x.b, 3, v);
    }
    Some(o)
}

fn next_state<R: Bench>(x: &mut Cx<'_, R>, by: u8) {
    x.add8(0x4, by);
}

// --- state 0: initialization ----------------------------------------------------------------

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    let n = x.a5_u8(a5::SCENES).wrapping_add(1);
    x.set_a5_u8(a5::SCENES, n);
    x.set16(f::LIVE_LIMIT, 0xffff);
    x.set16(f::LIVE, 0);
    x.set8(f::DONE, 0);
    x.set8(f::HOLD, 0);
    let variant = x.u8(0x3) as u32;
    let script = x.data32(SCRIPT_TABLE + 4 * variant);
    let time = x.data16(script);
    x.set16(f::TIMER, time);
    x.set32(f::CURSOR, script + 2);
    crate::families::creators::play_scene_music(x.b, x.o);
    let action = x.rom8(START_ACTION + variant);
    start(x, action);
}

/// Start action (0 to 11), chosen by the variant.
fn start<R: Bench>(x: &mut Cx<'_, R>, action: u8) {
    match action {
        // 10 and 11: with no game in progress (A5+0x66 zero), the controller frees itself
        10 | 11 if x.a5_u8(0x66) == 0 => x.free(&pools::SET),
        10 | 0 => {
            x.set_a5_u16(a5::TRANSITION, 4);
            next_state(x, 2);
            x.set_a5_u8(a5::TRANSITION_FLAG, 1);
        }
        11 | 1 | 2 | 6 => next_state(x, 2),
        5 => {
            x.set_a5_u16(a5::TRANSITION, 4);
            x.set_a5_u8(a5::TRANSITION_FLAG, 1);
            next_state(x, 2)
        }
        // jumps directly to the release
        7 => next_state(x, 6),
        8 => {
            x.set_a5_u16(0x46, 0);
            next_state(x, 2)
        }
        3 => {
            next_state(x, 2);
            x.set_a5_u16(a5::TRANSITION, 0x30);
            let v = x.a5_u16(0x556);
            x.set_a5_u16(0x574, v);
        }
        4 => opening_wipe(x),
        9 => {
            x.set_a5_u16(a5::TRANSITION, 0x20);
            next_state(x, 2);
            x.set_a5_u8(a5::TRANSITION_FLAG, 1);
        }
        _ => unreachable!("start table: 0 to 11"),
    }
}

/// Action 4: clears the screen tilemap by columns (one on each side of the center per frame, 11 frames), then goes to
/// state 2.
fn opening_wipe<R: Bench>(x: &mut Cx<'_, R>) {
    const LEFT: u32 = 0x91203c;
    const RIGHT: u32 = 0x912040;
    let column = x.u16(f::WAIT) as i16;
    if column > 10 {
        next_state(x, 2);
        x.set_a5_u8(a5::SCENE_SYNC, 1);
        wr8(x.b, PARALLAX_FLAG, 0);
        return;
    }
    let step = column.wrapping_mul(4) as i32 as u32;
    clear_column(x, LEFT.wrapping_sub(step));
    x.add16(f::WAIT, 1);
    clear_column(x, RIGHT.wrapping_add(step));
}

/// Fills a tilemap column with white (48 lines of 0x80 bytes).
fn clear_column<R: Bench>(x: &mut Cx<'_, R>, top: u32) {
    for row in 0..0x30 {
        wr32(x.b, top.wrapping_add(row * 0x80), 0x100000);
    }
}

// --- state 2: event script interpreter ----------------------------------------------------------

fn script<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(a5::STAGE) != 4 && x.a5_u8(a5::PHASE) != 0xc {
        return;
    }
    if x.a5_u8(a5::FROZEN) != 0 || x.u8(f::HOLD) != 0 {
        return end_of_script(x, true);
    }
    x.add16(f::TIMER, 0xffff);
    if x.u16(f::TIMER) & 0x8000 != 0 {
        return end_of_script(x, true);
    }
    if x.u8(f::WAIT_FLAG) != 0 {
        return;
    }
    let limit = x.u16(f::LIVE_LIMIT) as i16;
    if limit >= 0 {
        if limit < x.u16(f::LIVE) as i16 {
            return;
        }
    } else {
        x.add16(f::WAIT, 0xffff);
        if x.u16(f::WAIT) & 0x8000 == 0 {
            return;
        }
    }
    next_entry(x);
}

/// End of the script (`done`: interrupted rather than exhausted): goes to state 4.
fn end_of_script<R: Bench>(x: &mut Cx<'_, R>, done: bool) {
    if done {
        x.set8(f::DONE, 1);
    }
    next_state(x, 2);
    x.set16(f::WAIT, 0);
}

/// Skips the entries outside the current difficulty or rank, executes the first suitable one.
fn next_entry<R: Bench>(x: &mut Cx<'_, R>) {
    loop {
        let entry = x.u32(f::CURSOR);
        let word = x.data16(entry);
        if word == 0x8000 {
            return end_of_script(x, false);
        }
        let min_difficulty = x.data8(entry + 0xa) as i8;
        let max_rank = x.data16(entry + 2) as i16;
        if min_difficulty > x.a5_u8(a5::DIFFICULTY) as i8 || max_rank > x.a5_u16(a5::RANK) as i16 {
            x.set32(f::CURSOR, entry + 0x14);
            continue;
        }
        return run_entry(x, entry, word);
    }
}

/// Executes an entry: sets the wait then creates the object.
fn run_entry<R: Bench>(x: &mut Cx<'_, R>, entry: u32, word: u16) {
    x.set8(f::WAIT_FLAG, 0);
    x.set16(f::LIVE_LIMIT, 0xffff);
    x.set16(f::WAIT, word);
    if word & 0x8000 != 0 {
        let low = word & 0xff;
        if low & 0x80 != 0 {
            x.set8(f::WAIT_FLAG, 1);
        }
        x.set16(f::LIVE_LIMIT, low);
    }
    let class_offset = x.data8(entry + 0xb);
    let allocator = x.data32(ALLOC_TABLE + class_offset as u32);
    let Some(o) = x.alloc(pools::pool_of_alloc_routine(allocator)) else {
        x.set16(f::LIVE_LIMIT, 0xffff);
        x.set16(f::WAIT, 0);
        return;
    };
    o.set_u8(x.b, 0, 1);
    for (to, from) in [(2, 0xc), (3, 0xd), (0x2f, 0xe), (0x2c, 0xf)] {
        let v = x.data8(entry + from);
        o.set_u8(x.b, to, v);
    }
    for (to, from) in [(0x8, 4), (0xc, 6), (0x10, 8), (0x42, 0x10)] {
        let v = x.data16(entry + from);
        o.set_u16(x.b, to, v);
    }
    if class_offset != ITEM_CLASS {
        x.add16(f::LIVE, 1);
        o.set_u16(x.b, 0x46, x.o.0 as u16);
    }
    if x.data8(entry + 0x12) == 0 {
        let cam = if x.a5_u8(a5::USE_ALT_CAMERA) != 0 { a5::ALT_CAMERA } else { a5::CAMERA };
        let (cam_x, cam_y) = (x.a5_u16(cam + 8), x.a5_u16(cam + 0xc));
        let (px, py) = (o.u16(x.b, 0x8), o.u16(x.b, 0x10));
        o.set_u16(x.b, 0x8, px.wrapping_add(cam_x));
        o.set_u16(x.b, 0x10, py.wrapping_add(cam_y));
    }
    x.set32(f::CURSOR, entry + 0x14);
}

// --- state 4: end of scene ----------------------------------------------------------------------

fn finish<R: Bench>(x: &mut Cx<'_, R>) {
    x.add16(f::TIMER, 0xffff);
    if x.u16(f::TIMER) & 0x8000 != 0 {
        x.set8(f::DONE, 1);
        x.set16(f::TIMER, 0x7fff);
    }
    if x.u16(f::LIVE) != 0 {
        return;
    }
    let n = x.a5_u8(a5::SCENES).wrapping_sub(1);
    x.set_a5_u8(a5::SCENES, if n & 0x80 != 0 { 0 } else { n });
    x.set_a5_u8(a5::TRANSITION_FLAG, 0);
    crate::families::creators::run_scene_exit(x.b, x.o);
    let variant = x.u8(0x3) as u32;
    let action = x.rom8(END_ACTION + variant);
    end_action(x, action);
}

/// End-of-scene action (0 to 15), chosen by the variant.
fn end_action<R: Bench>(x: &mut Cx<'_, R>, action: u8) {
    match action {
        12 => {
            x.set_a5_u8(a5::FROZEN, 1);
            end_action(x, 0)
        }
        0 => {
            let v = x.a5_u16(0x506);
            x.set_a5_u16(0x52c, v);
            end_action(x, 11)
        }
        11 => {
            x.set_a5_u16(a5::TRANSITION, 0);
            next_state(x, 2)
        }
        8 => next_state(x, 2),
        15 => {
            next_state(x, 2);
            x.set_a5_u8(a5::FROZEN, 1)
        }
        1 => {
            x.set_a5_u16(a5::TRANSITION, 0x3c);
            next_state(x, 2)
        }
        2 => title_cards(x, 0x10, [(0x0b, 0), (0x49, 2)]),
        3 => title_cards(x, 0x20, [(0x4a, 0), (0x4b, 2)]),
        4 => players_leave(x),
        5 => {
            x.set_a5_u16(a5::TRANSITION, 0);
            let v = x.a5_u16(0x556);
            x.set_a5_u16(0x57c, v);
            next_state(x, 2)
        }
        6 => curtain(x),
        7 => effects_wave(x),
        9 => {
            if spawn(x, &pools::SET, 0x11, None).is_some() {
                next_state(x, 2)
            }
        }
        10 => {
            x.set_a5_u8(a5::FROZEN, 1);
            next_state(x, 2)
        }
        13 => spawn_boss(x, 3),
        14 => spawn_boss(x, 1),
        _ => unreachable!("end table: 0 to 15"),
    }
}

/// Actions 2 and 3: intertitle. Two SET scenery objects (variants `cards`, the second with `+0x2f = 2`) and a SOBJ
/// object (variant 0x56 then 0x57), then transition request of 0x3c frames.
fn title_cards<R: Bench>(x: &mut Cx<'_, R>, param: u16, cards: [(u8, u8); 2]) {
    x.set_a5_u16(0x56e, param);
    x.set_a5_u8(0x54f, 0);
    for (variant, extra) in cards {
        let Some(o) = spawn(x, &pools::SET, 0, Some(variant)) else { return };
        o.set_u8(x.b, 0x2f, extra);
    }
    let Some(o) = spawn(x, &pools::SOBJ, 0, Some(0x56)) else { return };
    o.set_u8(x.b, 3, 0x57);
    end_action(x, 1)
}

/// Action 4: the living players must have returned to the waiting state (`+4 = 0x200`, `+6 = 0`); then they receive the
/// exit order (state 0x80400) and their arrival rank (`+0x7d`), and the game phase goes to 0x18 (level exit).
fn players_leave<R: Bench>(x: &mut Cx<'_, R>) {
    x.set_a5_u8(a5::SCENE_LATCH, 0);
    x.set_a5_u8(a5::PLAYERS_LOCK, 1);
    let alive = |x: &mut Cx<'_, R>, p: u32| rd8(x.b, p) != 0 && rd16(x.b, p + 0x62) != 0;
    for i in 0..4 {
        let p = PLAYERS + i * PLAYER_STRIDE;
        if alive(x, p) && (rd16(x.b, p + 4) != 0x200 || rd8(x.b, p + 6) != 0) {
            return;
        }
    }
    let mut rank = 0;
    for i in 0..4 {
        let p = PLAYERS + i * PLAYER_STRIDE;
        if alive(x, p) {
            wr32(x.b, p + 4, 0x80400);
            wr8(x.b, p + 0x66, 0);
            wr8(x.b, p + 0x15c, 0);
            wr8(x.b, p + 0x7d, rank);
            rank += 1;
        }
    }
    x.set_a5_u32(a5::PHASE, 0x1802_0000);
    next_state(x, 2);
}

/// Action 6: the curtain. On the first pass, freezes the scene; 0x80 frames later, and as soon as a player is in state
/// 2, copies two blocks of 22 colors (0xbcfc6) to the addresses of table 0xbcea4 on each frame; when the table is
/// exhausted, pushes all the objects toward their disappearance (see `clear_all`).
fn curtain<R: Bench>(x: &mut Cx<'_, R>) {
    if x.u8(0x5) == 0 {
        x.add8(0x5, 2);
        x.set16(0x14, 0x80);
        x.set_a5_u8(a5::SCENE_LATCH, 0x1f);
        x.set_a5_u8(a5::FROZEN, 1);
        x.set_a5_u8(a5::SCENE_SYNC, 0);
        wr8(x.b, PARALLAX_FLAG, 1);
        return;
    }
    x.add16(0x14, 0xffff);
    if x.u16(0x14) & 0x8000 == 0 {
        return;
    }
    x.set16(0x14, 0);
    if !(0..4).any(|i| rd8(x.b, PLAYERS + i * PLAYER_STRIDE + 4) == 2) {
        return;
    }
    crate::events::clear_popups(x.b);
    crate::events::clear_current_popup(x.b);
    for _ in 0..2 {
        let at = x.u16(f::WAIT) as i16 as i32 as u32;
        let offset = x.rom16(CURTAIN_TABLE.wrapping_add(at)) as i16;
        if offset < 0 {
            return clear_all(x);
        }
        let hi = x.rom16(CURTAIN_TABLE.wrapping_add(at) + 2) as u32;
        let dest = hi << 16 | x.rom16(CURTAIN_TABLE.wrapping_add(at) + 4) as u32;
        x.add16(f::WAIT, 6);
        let colors = CURTAIN_PALETTE.wrapping_add(offset as i32 as u32);
        for i in 0..22 {
            let c = x.data16(colors + 2 * i);
            wr16(x.b, dest + 4 * i, c);
            wr16(x.b, dest + 4 * i + 2, 0x1f);
        }
    }
}

/// End of the curtain: every active object whose counter `+0x40` falls below zero goes to state 6 (disappearance), in
/// the eight pools.
fn clear_all<R: Bench>(x: &mut Cx<'_, R>) {
    next_state(x, 2);
    x.set_a5_u8(a5::SCENE_LATCH, 0xff);
    x.set_a5_u8(a5::FROZEN, 0);
    crate::sound::play(x.b, FADE_OUT, x.o);
    for pool in [&SET, &ENEMY, &SOBJ, &BOSS, &EFFCT, &ITEM, &ESHL, &PEFF] {
        for i in 0..pool.slots {
            let o = pool.slot(i);
            if o.u8(x.b, 0) == 0 {
                continue;
            }
            let hold = o.u8(x.b, 0x40).wrapping_sub(1);
            o.set_u8(x.b, 0x40, hold);
            if hold & 0x80 != 0 {
                o.set_u16(x.b, 0x4, 0x600);
                o.set_u8(x.b, 0x40, 0);
                o.set_u8(x.b, 0x41, 0);
            }
        }
    }
}

/// Action 7: four effects of type 0x21 (variants 0 to 3, 1 and 3 with `+0x2c = 0x20`).
fn effects_wave<R: Bench>(x: &mut Cx<'_, R>) {
    if x.a5_u8(a5::FROZEN) != 0 {
        return next_state(x, 2);
    }
    crate::sound::play(x.b, PREPARE_EFFECTS, x.o);
    if spawn(x, &pools::EFFCT, 0x21, Some(0)).is_none() {
        return;
    }
    next_state(x, 2);
    for (variant, extra) in [(1, 0x20), (2, 0), (3, 0x20)] {
        let Some(o) = spawn(x, &pools::EFFCT, 0x21, Some(variant)) else { return };
        if extra != 0 {
            o.set_u8(x.b, 0x2c, extra);
        }
    }
}

/// Actions 13 and 14: creates a boss (type 3 or 1) unless it is forbidden or the scene is frozen.
fn spawn_boss<R: Bench>(x: &mut Cx<'_, R>, kind: u8) {
    if x.a5_u8(a5::NO_BOSS) == 0 && x.a5_u8(a5::FROZEN) == 0 && spawn(x, &pools::BOSS, kind, None).is_none() {
        return;
    }
    next_state(x, 2);
}
