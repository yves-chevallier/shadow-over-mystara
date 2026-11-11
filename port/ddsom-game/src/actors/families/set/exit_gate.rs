use super::super::set_b;
use super::*;
use crate::hw::mem::{rd16, rd8, wr8, A5};
use crate::world::World;
use crate::actors::families::set::tables as set_tables;
use crate::actors::families::set::vars;
use crate::world::objects::Obj;
use crate::world::objects::player::Player;
use crate::actors::families::set_b::fields::DoorObj;
use crate::actors::families::set::fields::SetObj;

// --- ROM table (program space) ---
const GATES: u32 = 0xcb344;
const GATE_SHIFT: u16 = 4;
const GATE_X: u32 = 0;
const GATE_Z: u32 = 2;
const GATE_Y: u32 = 4;
const GATE_EXIT_KIND: u32 = 6;
const GATE_ZONE: u32 = 7;
const GATE_SCENE: u32 = 8;
const GATE_IGNORE_DEPTH: u32 = 9;
const GATE_BONUS: u32 = 10;
/// Player states imposed according to the zone index (`+0x46`), longs (4 bytes per index).
const PLAYER_STATES: u32 = 0xcb998;

// --- variables de A5 ---
const IN_GAME: u32 = 0x35;
const PAUSED: u32 = 0x3d;
const TRANSITION: u32 = 0x42;
const EXIT_LOCK: u32 = 0x50;
const OWNER_GATE: u32 = 0x52;
/// Variable (A5 - 0x5ac4): non-zero, nobody is judged to be in the footprint anymore.
const NEAR_BLOCK: u32 = A5 - 0x5ac4;

// --- door fields ---
const PLAYER_COUNT_TIMER: u32 = 0x20;
const ARRIVED: u32 = 0x16;
const TRIGGERED: u32 = 0x17;
const COUNTED: u32 = 0x1a;
const NEAR_COUNTERS: u32 = 0x42;
const EXIT_KIND: u32 = 0x48;
const ZONE: u32 = 0x46;
const NEXT_SCENE: u32 = 0x4f;
const IGNORE_DEPTH: u32 = 0x1b;
const BONUS: u32 = 0x24;

// --- fields of a player ---
const P_ACTIVE: u32 = 0x0;
const P_STATE: u32 = 0x4;
const P_X: u32 = 0x8;
const P_Z: u32 = 0xc;
const P_Y: u32 = 0x10;
const P_LIFE: u32 = 0x62;
const P_BLOCK_A: u32 = 0x66;
const P_BLOCK_B: u32 = 0x15c;
const P_BLOCK_C: u32 = 0x8a;
const P_ZONES: u32 = 0x6c;
const P_WEAPON_OBJ: u32 = 0x86;
const P_RESET: u32 = 0x10f;

/// Footprint of the door around its position (width 0x60, depth 0x40) and duration in frames.
const REACH_X: u16 = 0x30;
const REACH_Y: u16 = 0x20;
const NEAR_FRAMES: i8 = 0x18;
/// Player states recognized in the footprint (low byte ignored).
const WALKING_STATES: [u32; 3] = [0x2000000, 0x2000600, 0x2000400];
const PLAYER_STATE_IDLE: u32 = 0x2002010;
const PLAYER_STATE_GONE: u16 = 0x402;
const STATE_DONE: u8 = 2;

pub fn run<R: Bench>(x: &mut Cx<'_, R>) {
    refresh_player_count(x);
    if !gate_active(x.b, x.o) {
        return;
    }
    match x.u8(Obj::STATE) {
        0 => init(x),
        2 => watch(x),
        4 => arrive(x),
        _ => x.free(&pools::SET),
    }
}

// --- common conditions (read only) --------------------------------------------------------------

fn players() -> impl Iterator<Item = Obj> {
    util::players()
}

// --- changement de zone (0xcb844) ---------------------------------------------------------------

/// Text rectangle cleared on zone change and on return (`text::CLEAR_BOX`, A4 = column, row,
/// width, height).
const CLEARED_TEXT_BOX: u32 = 0xcbc02;
/// Accumulated exit bonus (byte at A5 - 0x5b20).
const BONUS_TOTAL: u32 = A5 - 0x5b20;
const P_FLAGS: u32 = 0x1;
const P_HIDDEN: u8 = 0x2;
const PLAYER_STATE_DEAD: u32 = 0x402_0000;
/// Door variants that mark the scene (`A5+0x4a`).
const MARKING_VARIANTS: [u8; 3] = [0x13, 0x15, 0x16];
const SCENE: u32 = 0x34;
const NEXT_SCENE_FLAG: u32 = 0x51;
const GAME_OVER: u32 = 0x42;
const PHASE: u32 = 0x2c;
const PHASE_SCENE_ENTRY: u8 = 8;
/// Longwords reset to zero before the next scene (the four players' weapons).
const PLAYER_WEAPONS: [u32; 4] = [0x602, 0x802, 0xa02, 0xc02];
/// Genres de sortie (`+0x48`).
const KIND_RETURN: u8 = 1;
const KIND_NEXT_SCENE: u8 = 2;
const KIND_GAME_OVER: u8 = 3;

/// All players are ready (0xcb844): bonus, palette, dead players hidden, then the exit.
fn change_zone<R: Bench>(x: &mut Cx<'_, R>) {
    let bonus = rd8(x.b, BONUS_TOTAL).wrapping_add(x.u8(BONUS));
    wr8(x.b, BONUS_TOTAL, bonus);
    crate::services::text::clear_box(x.b, CLEARED_TEXT_BOX);
    for p in players() {
        if p.u8(x.b, P_ACTIVE) != 0 && p.u16(x.b, P_LIFE) == 0 {
            let flags = p.u8(x.b, P_FLAGS) | P_HIDDEN;
            p.set_u8(x.b, P_FLAGS, flags);
            p.set_u32(x.b, P_STATE, PLAYER_STATE_DEAD);
        }
    }
    match x.u8(EXIT_KIND) {
        KIND_NEXT_SCENE => {
            x.set_a5_u8(NEXT_SCENE_FLAG, 1);
            exit_fading::fade_to_black(x, exit_fading::Step::Scene)
        }
        KIND_GAME_OVER => x.set_a5_u8(GAME_OVER, 1),
        _ => {
            x.set_a5_u16(OWNER_GATE, 0);
            if MARKING_VARIANTS.contains(&x.u8(Obj::VARIANT)) {
                x.set_a5_u8(vars::PAUSE, 1);
            }
            exit_fading::fade_to_black(x, exit_fading::Step::Zone)
        }
    }
}

/// After the fade of a zone change (0xcb896): return from a side zone, or entry.
pub fn after_fade<R: Bench>(x: &mut Cx<'_, R>) {
    let scene = x.u8(NEXT_SCENE);
    x.set_a5_u8(SCENE, scene);
    if x.u8(EXIT_KIND) == KIND_RETURN {
        return leave_annex(x);
    }
    save_and_suspend(x);
    x.set16(Obj::STATE, 0x400);
    enter_annex(x);
}

/// After the end-of-scene fade (0xcb9c0): the game goes back to phase 8 on scene `+0x4f`.
pub fn next_scene<R: Bench>(x: &mut Cx<'_, R>) {
    for off in PLAYER_WEAPONS {
        x.set_a5_u32(off, 0);
    }
    let scene = x.u8(NEXT_SCENE);
    x.set_a5_u8(SCENE, scene);
    x.set_a5_u8(EXIT_LOCK, 0);
    x.set_a5_u8(PHASE, PHASE_SCENE_ENTRY);
    x.set_a5_u8(PHASE + 1, 0);
    x.set_a5_u16(PHASE + 2, 0);
    x.set_a5_u16(OWNER_GATE, 0);
    set_b::reset_players_after_exit(x);
}

/// Pools suspended on entering a side zone, in the original's order (the players' effects,
/// between the enemy shots and the players' shots, are released instead of suspended).
const SUSPENDED: [&pools::Pool; 8] = [
    &pools::SET, &pools::SOBJ, &pools::ENEMY, &pools::EFFCT, &pools::BOX,
    &pools::ITEM, &pools::BOSS, &pools::ESHL,
];
/// Scenery of type 0x13 released on entering the doors of variant 8 and 0xa.
const FREED_KIND: u8 = 0x13;
/// Cameras imposed by variants 8 and 0xa (`+0x30..+0x3c`).
const CAMERA_VARIANT_8: [u32; 4] = [0x2a0_0000, 0x108_0000, 0x150_0000, 0];
const CAMERA_VARIANT_A: [u32; 4] = [0x400_0000, 0x1fe_0000, 0x200_0000, 0x7b_0000];

/// Saves the scene state in the door and suspends the world (0xcbcba).
fn save_and_suspend<R: crate::actors::families::cx::Bench>(x: &mut Cx<'_, R>) {
    set_b::save_camera(x);
    let v = x.a5_u16(vars::CAMERA_MAX_X);
    x.set16(DoorObj::CAMERA_MAX_X_SAVED, v);
    let v = x.a5_u8(NEXT_SCENE_FLAG);
    x.set8(DoorObj::NEXT_SCENE_FLAG_SAVED, v);
    x.set_a5_u8(NEXT_SCENE_FLAG, 0);
    let v = x.a5_u8(vars::TRANSITION);
    x.set8(DoorObj::TRANSITION_SAVED, v);
    let camera = match x.u8(Obj::VARIANT) {
        0xa => Some(CAMERA_VARIANT_A),
        0x8 => Some(CAMERA_VARIANT_8),
        _ => None,
    };
    if let Some(camera) = camera {
        for (i, v) in camera.into_iter().enumerate() {
            x.set32(DoorObj::CAMERAS_SAVED + 4 * i as u32, v);
        }
        for o in pools::SET.iter() {
            if o.u8(x.b, Obj::ACTIVE) != 0 && o.u8(x.b, Obj::KIND) == FREED_KIND {
                pools::SET.release(x.b, o);
            }
        }
    }
    for pool in SUSPENDED {
        set_b::suspend(x, pool);
    }
    free_player_effects(x);
    set_b::suspend(x, &pools::PSHL);
}

fn free_player_effects<R: Bench>(x: &mut Cx<'_, R>) {
    for o in pools::PEFF.iter() {
        if o.u8(x.b, Obj::ACTIVE) != 0 {
            o.set_u8(x.b, Obj::STATE, 6);
        }
    }
}

/// Entry into the side zone (0xcc0ca): cameras, scene script, palettes, players repositioned.
fn enter_annex<R: Bench>(x: &mut Cx<'_, R>) {
    crate::actors::families::scenery::restart_scene(x);
}

/// Return from a side zone (0xcbeba): players released, the door releases itself, the world wakes up.
fn leave_annex<R: Bench>(x: &mut Cx<'_, R>) {
    for p in players() {
        if p.u8(x.b, P_ACTIVE) == 0 {
            continue;
        }
        p.set_u8(x.b, P_ACTIVE, 1);
        let flags = p.u8(x.b, P_FLAGS) & !P_HIDDEN;
        p.set_u8(x.b, P_FLAGS, flags);
        for off in [P_BLOCK_A, P_BLOCK_B, P_BLOCK_C, P_RESET] {
            p.set_u8(x.b, off, 0);
        }
        p.set_u8(x.b, Player::INVULN, 0x46);
    }
    x.set8(Obj::STATE, 6);
    // the other woken doors run immediately (the original's recursive `bsr`)
    set_b::resume_with(x, &pools::SET, |x, o| {
        if o.u8(x.b, Obj::KIND) == 7 {
            set_b::run_on(x, o, run);
        }
    });
    for pool in &SUSPENDED[1..] {
        set_b::resume(x, pool);
    }
    free_player_effects(x);
    set_b::resume(x, &pools::PSHL);
}

/// State 4 (0xcba48): return from the side zone, the previous scene is reloaded.
fn arrive<R: Bench>(x: &mut Cx<'_, R>) {
    crate::actors::families::scenery::resume_scenery_lit(x, crate::actors::families::scenery::Cameras::Return);
    x.set_a5_u16(OWNER_GATE, 0);
    crate::services::text::clear_box(x.b, CLEARED_TEXT_BOX);
    set_b::restore_camera(x);
    let v = x.u16(DoorObj::CAMERA_MAX_X_SAVED);
    x.set_a5_u16(vars::CAMERA_MAX_X, v);
    let v = x.u8(DoorObj::NEXT_SCENE_FLAG_SAVED);
    x.set_a5_u8(NEXT_SCENE_FLAG, v);
    let v = x.u8(DoorObj::TRANSITION_SAVED);
    x.set_a5_u8(vars::TRANSITION, v);
    let music = x.u16(DoorObj::MUSIC);
    crate::services::sound::play_with(x.b, crate::services::sound::FROM_D4, crate::services::sound::Source { d4: Some(music as u32), ..Default::default() });
    for p in players() {
        if p.u8(x.b, P_ACTIVE) != 0 {
            place(x, p);
        }
    }
    x.set8(Obj::STATE, 6);
}

/// Placement row per variant (byte, program space) then X, Y offsets per row and per
/// player (0x10 bytes per row, program space).
const PLACEMENT_ROW: u32 = 0xcbb92;
const PLACEMENT: u32 = 0xcbbe2;
const PLAYER_STATE_WALK: u32 = 0x200_0000;

/// Places a player back in front of the door (0xcbb3c); a player not in state 2 is only hidden
/// and put on the ground.
fn place<R: Bench>(x: &mut Cx<'_, R>, p: Obj) {
    if p.u8(x.b, P_STATE) != 2 {
        let flags = p.u8(x.b, P_FLAGS) | P_HIDDEN;
        p.set_u8(x.b, P_FLAGS, flags);
        crate::world::placement::place_on_free_ground(x.b, p);
        return;
    }
    p.set_u32(x.b, P_STATE, PLAYER_STATE_WALK);
    let flags = p.u8(x.b, P_FLAGS) & !P_HIDDEN;
    p.set_u8(x.b, P_FLAGS, flags);
    p.set_u8(x.b, P_ACTIVE, 1);
    let variant = x.u8(Obj::VARIANT) as u32;
    let row = (set_tables::pc_byte(x.b, PLACEMENT_ROW + variant) as u16) << 4;
    let index = row.wrapping_add(4 * p.u8(x.b, Player::INDEX) as u16) as u32;
    let (dx, dy) = (set_tables::pc_word(x.b, PLACEMENT + index), set_tables::pc_word(x.b, PLACEMENT + 2 + index));
    let (gx, gy) = (x.u16(Obj::X), x.u16(Obj::Y));
    p.set_u16(x.b, P_X, gx.wrapping_add(dx));
    p.set_u16(x.b, P_Y, gy.wrapping_add(dy));
    p.set_u16(x.b, P_Z, 0);
    // Original glitch: the Y offset stays in D2.w (`dispatch::Leftovers`)
    crate::hw::mem::set_low_word(x.c.d2(), dy);
}

fn alive(b: &mut World, p: Obj) -> bool {
    p.u8(b, P_ACTIVE) != 0 && p.u16(b, P_LIFE) != 0
}

/// Conditions of 0xcb2a0-0xcb2d8: does the door act on this frame?
fn gate_active(b: &mut World, me: Obj) -> bool {
    if me.u8(b, SetObj::PARAM) != 0 || me.u8(b, Obj::HIDDEN) != 0 {
        return false;
    }
    if rd8(b, A5 + PAUSED) != 0 || rd8(b, A5 + EXIT_LOCK) != 0 || rd8(b, A5 + TRANSITION) != 0 {
        return false;
    }
    let owner = rd16(b, A5 + OWNER_GATE);
    owner == 0 || owner == me.0 as u16
}

/// Is player `p` standing in the door's footprint, free to move, and is the door's
/// zone present in their mask `+0x6c`? (The original tests the zone index bit
/// in D0, whose high bits are leftovers: the zone indices are below 8.)
fn in_reach(b: &mut World, me: Obj, p: Obj) -> bool {
    if rd8(b, NEAR_BLOCK) != 0 {
        return false;
    }
    let blocked = p.u8(b, P_BLOCK_A).wrapping_add(p.u8(b, P_BLOCK_B)).wrapping_add(p.u8(b, P_BLOCK_C));
    let zones = p.u8(b, P_ZONES) as u32;
    if blocked != 0 || zones >> (me.u8(b, ZONE) as u32 & 31) & 1 == 0 {
        return false;
    }
    let state = p.u32(b, P_STATE);
    if state >> 24 != 2 || !WALKING_STATES.contains(&(state & !0xff)) || p.u16(b, P_Z) != 0 {
        return false;
    }
    let dx = p.u16(b, P_X).wrapping_sub(me.u16(b, Obj::X)).wrapping_add(REACH_X);
    if dx > 2 * REACH_X {
        return false;
    }
    if me.u8(b, IGNORE_DEPTH) != 0 {
        return true;
    }
    let dy = p.u16(b, P_Y).wrapping_sub(me.u16(b, Obj::Y)).wrapping_add(REACH_Y);
    dy <= 2 * REACH_Y
}

/// Are all living players ready (0xcb80c-0xcb840)? `arrived`: players already counted.
fn everyone_ready(b: &mut World, in_game: u8, arrived: u8) -> bool {
    if in_game == 0 {
        return false;
    }
    let mut missing = in_game;
    for p in players() {
        if p.u8(b, P_ACTIVE) != 0 && p.u16(b, P_LIFE) == 0 && p.u16(b, P_STATE) != PLAYER_STATE_GONE {
            missing = missing.wrapping_add(1);
        }
    }
    (missing as i8) <= (arrived as i8)
}

// --- l'automate ---------------------------------------------------------------------------------

/// Every 32 frames: number of living players in `A5+0x35`.
fn refresh_player_count<R: Bench>(x: &mut Cx<'_, R>) {
    let t = x.u16(PLAYER_COUNT_TIMER).wrapping_sub(1);
    x.set16(PLAYER_COUNT_TIMER, t);
    if (t as i16) >= 0 {
        return;
    }
    x.set16(PLAYER_COUNT_TIMER, 0x20);
    let n = players().filter(|&p| alive(x.b, p)).count();
    x.set_a5_u8(IN_GAME, n as u8);
}

fn init<R: Bench>(x: &mut Cx<'_, R>) {
    x.add8(Obj::STATE, 2);
    for off in [NEAR_COUNTERS, NEAR_COUNTERS + 1, NEAR_COUNTERS + 2, NEAR_COUNTERS + 3, ARRIVED, TRIGGERED] {
        x.set8(off, 0u8);
    }
    let row = GATES + (((x.u8(Obj::VARIANT) as u16) << GATE_SHIFT) as i16 as i32 as u32);
    let (gx, gz, gy) = (set_tables::pc_word(x.b, row + GATE_X), set_tables::pc_word(x.b, row + GATE_Z), set_tables::pc_word(x.b, row + GATE_Y));
    x.set16(Obj::X, gx);
    x.set16(Obj::Z, gz);
    x.set16(Obj::Y, gy);
    let fields = [(EXIT_KIND, GATE_EXIT_KIND), (ZONE, GATE_ZONE), (NEXT_SCENE, GATE_SCENE), (IGNORE_DEPTH, GATE_IGNORE_DEPTH), (BONUS, GATE_BONUS)];
    for (field, entry) in fields {
        let v = set_tables::pc_byte(x.b, row + entry);
        x.set8(field, v);
    }
}

/// State 2: a player stays in the footprint long enough (trigger), or reaches the "ready"
/// state (counted once); when all are ready, the zone changes.
fn watch<R: Bench>(x: &mut Cx<'_, R>) {
    for (i, p) in players().enumerate() {
        let bit = 1u8 << i;
        let counter = NEAR_COUNTERS + i as u32;
        if x.u8(TRIGGERED) & bit == 0 {
            if p.u8(x.b, P_ACTIVE) != 1 {
                continue;
            }
            if (x.u8(counter) as i8) >= 0 {
                let inside = in_reach(x.b, x.o, p);
                let n = if inside { x.u8(counter).wrapping_add(1) } else { 0 };
                x.set8(counter, n);
                if (n as i8) > NEAR_FRAMES {
                    return trigger(x, p, bit);
                }
                continue;
            }
        }
        if x.u8(COUNTED) & bit == 0 && p.u32(x.b, P_STATE) == PLAYER_STATE_IDLE {
            x.add8(ARRIVED, 1);
            x.set_bit(COUNTED, i as u8, true);
        }
    }
    let (in_game, arrived) = (x.a5_u8(IN_GAME), x.u8(ARRIVED));
    if everyone_ready(x.b, in_game, arrived) {
        change_zone(x);
    }
}

/// Player `p` triggers the exit: they go to the state imposed by the zone; the door
/// becomes the owner of the transition (`A5+0x52`).
fn trigger<R: Bench>(x: &mut Cx<'_, R>, p: Obj, bit: u8) {
    if p.u8(x.b, P_ACTIVE) == 0 || p.u16(x.b, P_LIFE) == 0 {
        return;
    }
    let triggered = x.u8(TRIGGERED) | bit;
    x.set8(TRIGGERED, triggered);
    if x.a5_u16(OWNER_GATE) == 0 {
        crate::actors::families::effects_creators::beacon(x.b, x.o);
    }
    let me = x.o.0 as u16;
    x.set_a5_u16(OWNER_GATE, me);
    let zone = x.u8(ZONE) as u32;
    let state = set_tables::pc_long(x.b, PLAYER_STATES + 4 * zone);
    p.set_u32(x.b, P_STATE, state);
    p.set_u8(x.b, P_ACTIVE, STATE_DONE);
    p.set_u8(x.b, P_RESET, 1);
    match x.u8(Obj::VARIANT) {
        0xa | 0xe => p.set_u32(x.b, P_STATE, 0x2002008),
        0x8 => p.set_u32(x.b, P_STATE, 0x200200a),
        0x28 => {
            p.set_u16(x.b, P_WEAPON_OBJ, x.o.0 as u16);
            p.set_u32(x.b, P_STATE, 0x200200c);
        }
        _ => {}
    }
}
