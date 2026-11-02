//! Phase 8 (`0x20c38`): scene entry.
//!
//! Step 0: prepares the scene (variables, palettes, cameras, players reset and placed according to
//! the scene tables). Step 2: on each frame, plays the scene's *opening sequence*
//! (a fade, sometimes a title card, a dialogue or a boss entering), runs the appearance script
//! reader (`script`), then closes the phase: the sequence sets the phase to 12 (the game)
//! when it is finished.
//!
//! The sequences are state machines: the sub-step (`A5+0x2e`, step of 2) tells where we
//! are, timers and object pointers are kept in temporary variables
//! (`scratch`). A scene without its own sequence (the majority) only does a fade and immediately goes
//! to phase 12. See `docs/scenes.md` for the scene table.

use cps2::Cps2Bus;
use super::fading;

use super::{player, scratch, script, var, Ctx, PLAYERS, PLAYER_STRIDE};
use crate::families::effects_creators as creators;
use crate::game;
use crate::mem::{rd16, rd8, rts, wr16, wr32, wr8, A5};
use crate::objects::Obj;
use crate::Native;
use crate::objects::pools;

/// Sound commands called by the sequences (wrappers of `sound_table.rs`).
mod sfx {
    /// Scene music (the scenes' "end of cutscene" command).
    pub const SCENE_MUSIC: u32 = 0xe0c5c;
    pub const COMMAND_21: u32 = 0xe1370;
    pub const COMMAND_2D: u32 = 0xe1768;
    pub const COMMAND_E17A4: u32 = 0xe17a4;
    pub const COMMAND_E62FC: u32 = 0xe62fc;
    pub const COMMAND_EAAF4: u32 = 0xeaaf4;
    pub const COMMAND_EA6E2: u32 = 0xea6e2;
    pub const COMMAND_EADD4: u32 = 0xeadd4;
}

/// ROM tables (data), indexed by scene number.
mod table {
    /// One byte per scene copied to `A5+0x48` / `A5+0x49` (scene palette parameters).
    pub const PALETTE_A: u32 = 0x21b38;
    pub const PALETTE_B: u32 = 0x21bb0;
    /// One byte per scene (copied to `A5+0x3e` and `A5+0x40`), index ×2.
    pub const SCENE_BYTE: u32 = 0x21156;
    /// Players' entry position: 4 pairs (X, Y) of words per scene.
    pub const START_POSITIONS: u32 = 0x2123c;
    /// One byte per player and per scene (`+0x2c` of the player).
    pub const START_STATES: u32 = 0x2196c;
}


/// Work RAM variables outside the A5 ± 0x7fff page, cleared on entry.
mod ram {
    pub const BYTE_A: u32 = 0xff_2472;
    pub const BYTE_B: u32 = 0xff_2474;
    pub const BYTE_C: u32 = 0xff_2042;
    pub const WORD_D: u32 = 0xff_2046;
    pub const WORD_E: u32 = 0xff_2048;
    pub const WORD_F: u32 = 0xff_204a;
    pub const WORD_G: u32 = 0xff_204c;
    pub const WORD_H: u32 = 0xff_2040;
}

const STEP_INIT: u8 = 0;
const STEP_SEQUENCE: u8 = 2;

/// Scenes where the world is in cutscene mode (`A5+0x44`): 7, 8 and 9 (entries of the cave boss).
const CINEMATIC_SCENES: [u8; 3] = [7, 8, 9];

/// Player fields cleared on scene entry (bytes, words, longs).
const PLAYER_CLEAR_BYTES: [u32; 8] = [0x16a, 0x15c, 0x8a, 0x10f, 0x18a, 0x188, 0x189, 0x53];
const PLAYER_CLEAR_WORDS: [u32; 3] = [0x8c, 0x8e, 0x8a];
const PLAYER_CLEAR_LONGS: [u32; 2] = [0x86, 0x24];
const PLAYER_SPELL_SLOTS: std::ops::RangeInclusive<u32> = 0x1a0..=0x1d8;

// ---------------------------------------------------------------------------------------------
// Step 0: scene preparation

fn init_scene(x: &mut Ctx) {
    wr8(x.b, A5 + var::STEP, STEP_SEQUENCE);
    for addr in [ram::BYTE_A, ram::BYTE_B, ram::BYTE_C] {
        wr8(x.b, addr, 0);
    }
    for addr in [ram::WORD_D, ram::WORD_E, ram::WORD_F] {
        wr16(x.b, addr, 0);
    }
    wr16(x.b, ram::WORD_G, 5);
    wr16(x.b, ram::WORD_H, 9);
    x.set_v8(0x5f, 1);
    x.set_v8(var::INPUT_LOCK, 0xff);
    x.set_v8(0x3c, 1);
    x.set_v8(0x3d, 0);
    x.set_v8(var::HOLD, 0);
    x.set_v16(0x54, 0);
    x.set_v16(0x52, 0);
    x.set_v8(0x58, 0);
    x.set_v8(0x51, 0);
    let scene = x.v8(var::SCENE) as u32;
    let (pa, pb) = (x.rd8(table::PALETTE_A + scene), x.rd8(table::PALETTE_B + scene));
    x.set_v8(0x48, pa);
    x.set_v8(0x49, pb);
    crate::modes::demo::reset_recorder(x.b);
    crate::sprites::clear_lists(x.b);
    // 0x33aa6: cameras at start, then their update (D4 of which is re-read: `scroll::update_cameras_in_game`)
    crate::scroll::cameras_start(x.b);
    crate::scroll::update_cameras_in_game(x.b);
    script::reset(x.b);
    for pool in crate::objects::pools::POOLS {
        pool.reset(x.b);
    }
    crate::palette::load_all_palettes(x.b);
    crate::events::clear_points(x.b);
    crate::events::clear_counter(x.b);
    crate::events::clear_popups(x.b);
    crate::events::clear_current_popup(x.b);
    super::screen::clear_longs(x.b, super::screen::blocks::EXIT, 8);
    crate::hud::forget_displayed(x.b);
    for k in 0..4 {
        clear_player(x, PLAYERS + k * PLAYER_STRIDE);
    }
    let cinematic = CINEMATIC_SCENES.contains(&(scene as u8));
    x.set_v8(0x44, cinematic as u8);
    let marker = x.rd8(table::SCENE_BYTE + 2 * scene);
    x.set_v8(0x3e, marker);
    x.set_v8(0x40, marker);
    let mut alive = 0u8;
    for k in 0..4 {
        let p = PLAYERS + k * PLAYER_STRIDE;
        if x.rd8(p + player::ACTIVE) != 0 && x.rd16(p + player::HEALTH) != 0 {
            alive += 1;
            wr32(x.b, p + 4, 0);
        }
    }
    x.set_v8(var::PLAYERS, alive);
    place_players(x.b);
}

fn clear_player(x: &mut Ctx, p: u32) {
    for off in PLAYER_CLEAR_BYTES {
        wr8(x.b, p + off, 0);
    }
    for off in PLAYER_CLEAR_WORDS {
        wr16(x.b, p + off, 0);
    }
    for off in PLAYER_CLEAR_LONGS {
        wr32(x.b, p + off, 0);
    }
    for off in PLAYER_SPELL_SLOTS.step_by(4) {
        wr32(x.b, p + off, 0);
    }
}

/// What the player placement leaves in the original's registers: D0/D1 = the camera (or the shifted
/// scene), D2 = the rank, A2/A3 = the ends of the tables.
pub(crate) struct PlacementLeftovers {
    d0: u32,
    d1: u32,
    pub(crate) rank: u8,
    positions: u32,
    states: u32,
}

/// 0x20de6: places each player in the game in front of the camera, from the scene tables (consumed in the order
/// of the players in the game).
pub fn place_players(b: &mut Cps2Bus) {
    place_players_leftovers(b);
}

pub(crate) fn place_players_leftovers(b: &mut Cps2Bus) -> PlacementLeftovers {
    let scene = rd8(b, A5 + var::SCENE) as u32;
    let mut positions = table::START_POSITIONS + (scene << 4);
    let mut states = table::START_STATES + (scene << 2);
    let camera = A5 + if rd8(b, A5 + var::USE_ALT_CAMERA) != 0 { 0x556 } else { 0x506 };
    let (mut d0, mut d1) = (scene << 4, scene << 2);
    let mut rank = 0u8;
    for k in 0..4 {
        let p = PLAYERS + k * PLAYER_STRIDE;
        if rd8(b, p + player::ACTIVE) == 0 || rd16(b, p + player::HEALTH) == 0 {
            continue;
        }
        let (dx, dy) = (rd16(b, camera), rd16(b, camera + 4));
        (d0, d1) = (dx as u32, dy as u32);
        let (px, py) = (rd16(b, positions), rd16(b, positions + 2));
        positions += 4;
        wr16(b, p + 8, px.wrapping_add(dx));
        wr16(b, p + 0x10, py.wrapping_add(dy));
        wr32(b, p + 0xc, 0);
        let state = rd8(b, states);
        wr8(b, p + 0x2c, state);
        states += 1;
        wr8(b, p + 0x7d, rank);
        rank += 1;
    }
    PlacementLeftovers { d0, d1, rank: rank, positions, states: states }
}

/// 0x20de6 called by the original code (respawn of a player, `families/creators.rs`): `place_players`, with
/// the original's registers (D6 = -1, A1 after the last player).
pub const PLACE_PLAYERS: Native = Native {
    entry: 0x20de6,
    name: "scene_place_players",
    run: crate::Body::WithCpu(|c, b| {
        let r = place_players_leftovers(b);
        (c.d[0], c.d[1], c.d[2], c.d[6]) = (r.d0, r.d1, r.rank as u32, 0xffff);
        (c.a[1], c.a[2], c.a[3]) = (PLAYERS + 4 * PLAYER_STRIDE, r.positions, r.states);
        crate::mem::rts(c, b);
    }),
    touches_sound: false,
    outputs: crate::regs::ALL,
};

// ---------------------------------------------------------------------------------------------
// Step 2: opening sequences

/// An opening sequence: one step per frame (the world is updated at the end).
type Sequence = fn(&mut Ctx);

/// Opening sequence of scene `scene`.
fn sequence(scene: u8) -> Sequence {
    match scene {
        1 => stage_1,
        2 => stage_2,
        4 => scene_4,
        5 | 20 => wait_players_10,
        7 | 9 => boss_cave,
        8 => scene_8,
        10 => scene_10,
        11 => scene_11,
        14 => title_card,
        24 => scene_24,
        25 | 106..=108 => quick_with_music,
        26 => scene_26,
        27 => scene_27,
        28 => scene_28,
        32 => scene_32,
        33 | 36 => plain_fade,
        34 => scene_34,
        35 => title_card_music,
        38 => scene_38,
        39 | 41 | 43 | 45 => wait_players_6,
        40 => scene_40,
        42 => scene_42,
        44 | 111 => scene_44,
        47 => scene_47,
        54 => scene_54,
        58 => scene_58,
        59 => title_card_timed,
        64 => scene_64,
        65 => scene_65,
        66 => scene_66,
        67 => scene_67,
        90..=104 => white_start,
        105 => scene_105,
        109 | 112..=114 => quick_with_sobj,
        110 => quick_with_sobj_flag,
        _ => quick,
    }
}

/// Phase 8: one frame. `handler`: the address of the phase handler, which the original leaves in A0 (`jsr (A0)`
/// of the game loop, table 0x1c502, or of the demo) and which two opening sequences reread as an object (original
/// glitch: effects 0x13 of the lightning, finale object).
pub fn phase_8(b: &mut Cps2Bus, handler: u32) {
    let mut x = Ctx::new(b);
    x.phase_handler = handler;
    match x.v8(var::STEP) {
        STEP_INIT => init_scene(&mut x),
        STEP_SEQUENCE => {
            let scene = x.v8(var::SCENE);
            sequence(scene)(&mut x);
            script::step(x.b);
            game::debug_scene_skip(x.b);
        }
        _ => {}
    }
}

pub const PHASE_8: Native = Native {
    entry: 0x20c38,
    name: "scene_phase_8",
    run: crate::Body::WithCpu(|c, b| {
        // called by the original code (tools): A0 = the phase handler
        phase_8(b, c.a[0]);
        rts(c, b);
    }),
    touches_sound: false,
    outputs: 0,
};

// --- stateless sequences -----------------------------------------------------------------------

/// Scene without its own sequence: an opening fade, the music, and the game starts.
fn quick(x: &mut Ctx) {
    x.leave_phase();
    x.set_v8(var::PLAYERS_FROZEN, 0);
    x.fade_in();
    crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
    x.world();
}

/// Scenes 25, 106-108: like `quick`, the music starts only once (`A5+0x67`).
fn quick_with_music(x: &mut Ctx) {
    x.leave_phase();
    x.set_v8(var::PLAYERS_FROZEN, 0);
    x.fade_in();
    if x.v8(0x67) == 0 {
        x.set_v8(0x67, 1);
        crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
    }
    x.world();
}

/// Scenes 90-104 (attract demos): fade from white and system object 6/23.
fn white_start(x: &mut Ctx) {
    x.leave_phase();
    x.set_v8(var::PLAYERS_FROZEN, 0);
    fading::PAGES.set(x.b, 15);
    crate::palette::full_brightness(x.b);
    x.sobj_6(23);
    x.world();
}

/// Scenes 109, 112-114: fade, system object 6/23, music.
fn quick_with_sobj(x: &mut Ctx) {
    x.leave_phase();
    x.set_v8(var::PLAYERS_FROZEN, 0);
    x.fade_in();
    x.sobj_6(23);
    crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
    x.world();
}

/// Scene 110: raises `A5+0x59` then like 109.
fn quick_with_sobj_flag(x: &mut Ctx) {
    x.set_v8(0x59, 1);
    quick_with_sobj(x);
}

/// Scene 67: jumps directly to the next phase, inputs locked.
fn scene_67(x: &mut Ctx) {
    x.leave_phase();
    x.set_v8(var::HOLD, 0);
    x.set_v8(var::INPUT_LOCK, 0x0f);
    x.set_v8(var::PLAYERS_FROZEN, 1);
    x.fade_in();
    x.title_card();
    x.inputs();
    x.world();
}

// --- stateful sequences ---------------------------------------------------------------------------

/// Usual start: next sub-step, scene and players locked, opening fade.
fn begin(x: &mut Ctx) {
    x.next_sub();
    x.lock();
}

/// Scene 1 (level 1): fade, title card, first dialogue, then three enemies enter from the right.
fn stage_1(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a32);
    const MESSAGE: u32 = scratch(0x9a34);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 0x140);
            x.fade_in();
            creators::trails_pure(x.b);
            x.title_card();
            crate::objects::rank::reset(x.b);
            crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                // Original glitch: the failure is not tested, the A1 left by the allocation is stored
                let message = crate::families::creators::scene_set_or_rest(x.b, 0);
                wr32(x.b, MESSAGE, message.0);
            }
        }
        4 => {
            let message = Obj(crate::mem::rd32(x.b, MESSAGE));
            if message.u8(x.b, 0x22) != 0 {
                x.next_sub();
                x.wr16(TIMER, 0xb0);
            }
        }
        6 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.wr16(TIMER, 0x60);
            }
        }
        8 => {
            if x.tick(TIMER) {
                x.leave_phase();
                x.set_v8(var::HOLD, 0);
                x.set_v8(var::PLAYERS_FROZEN, 0);
            } else if x.rd16(TIMER) == 0x50 {
                for (px, py) in [(0x180, 0x210), (0x230, 0x220), (0x2d0, 0x200)] {
                    let o = x.spawn(&pools::ENEMY, 0x14);
                    o.set_u8(x.b, Obj::ACTIVE, 2);
                    o.set_u8(x.b, Obj::VARIANT, 1);
                    o.set_u16(x.b, Obj::X, px);
                    o.set_u16(x.b, Obj::Y, py);
                    o.set_u16(x.b, 0x14, 0xfb80);
                }
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 2: fade, title card; waits a moment.
fn stage_2(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a38);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 1);
            x.fade_in();
            x.title_card();
            crate::objects::rank::reset(x.b);
            crate::sound::play_global(x.b, sfx::COMMAND_EAAF4);
            crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 4: title, then two scene effects at intervals.
fn scene_4(x: &mut Ctx) {
    const SHAKE: u32 = scratch(0x9a3c);
    const TIMER: u32 = scratch(0x9a3a);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(SHAKE, 0);
            x.wr16(TIMER, 0x158);
            x.fade_in();
            x.title_card();
            crate::objects::rank::reset(x.b);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.wr16(TIMER, 0x40);
                x.set_v8(0x54f, 8);
            }
        }
        4 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.wr16(SHAKE, 1);
                x.wr16(TIMER, 0x130);
                x.set_v8(0x500, 8);
            }
        }
        6 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scenes 7 and 9 (boss cave): an effect, a boss and a piece of scenery enter; 7 resets the rank to zero.
fn boss_cave(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a3a);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 0x90);
            x.fade_in();
            let scene = x.v8(var::SCENE);
            if scene != 9 {
                x.title_card();
            }
            let effect = x.spawn(&pools::EFFCT, 13);
            let camera_x = x.v16(0x506);
            let camera_y = x.v16(0x50a);
            effect.set_u16(x.b, Obj::X, camera_x.wrapping_sub(0x100));
            effect.set_u16(x.b, Obj::Y, camera_y.wrapping_add(0x50));
            x.set_v16(0x46, effect.0 as u16);
            x.spawn(&pools::BOSS, 13);
            let set = x.spawn(&pools::SET, 14);
            if scene == 9 {
                set.set_u8(x.b, Obj::VARIANT, 1);
            }
            if scene == 7 {
                crate::objects::rank::reset(x.b);
            }
            crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 8: an effect 0x22 near the camera, a sound command, then effect 0xb8956.
fn scene_8(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a3a);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 0x100);
            x.fade_in();
            let effect = x.spawn(&pools::EFFCT, 0x22);
            let camera_x = x.v16(0x506);
            let camera_y = x.v16(0x50a);
            effect.set_u16(x.b, Obj::X, camera_x.wrapping_sub(0x60));
            effect.set_u16(x.b, Obj::Y, camera_y.wrapping_add(0x50));
            x.set_v16(0x46, effect.0 as u16);
            crate::sound::service(x.b, crate::sound::Service::GeneralStop);
            creators::countdown_timer(x.b, 1);
        }
        2 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 10: forced transition (`A5+0x51e` = 4), an effect 0x13 and a piece of scenery.
fn scene_10(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a3a);
    match x.sub() {
        0 => {
            x.next_sub();
            x.set_v16(0x51e, 4);
            x.lock();
            x.wr16(TIMER, 0xc0);
            x.fade_in();
            let effect = x.spawn(&pools::EFFCT, 0x13);
            x.set_v16(0x46, effect.0 as u16);
            let set = x.spawn(&pools::SET, 14);
            set.set_u8(x.b, Obj::VARIANT, 1);
            crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.leave_phase();
                x.set_v16(0x51e, 0);
                x.set_v8(var::HOLD, 0);
                x.set_v8(var::PLAYERS_FROZEN, 0);
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 11: title card, two dialogues (95), an enemy (21), a forced scroll.
fn scene_11(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a3e);
    const MESSAGE: u32 = scratch(0x9a40);
    const ENEMY: u32 = scratch(0x9a42);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 0x78);
            x.fade_in();
            x.title_card();
            creators::swimmers(x.b);
            crate::objects::rank::reset(x.b);
            crate::sound::play_global(x.b, sfx::COMMAND_EA6E2);
            crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.wr16(TIMER, 0xa0);
                creators::bench(x.b);
            }
        }
        4 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(95, MESSAGE);
            } else if x.rd16(TIMER) == 0x40 {
                creators::bench(x.b);
            }
        }
        6 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                x.wr16(TIMER, 0x10);
            }
        }
        8 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.wr16(TIMER, 0xe0);
                let o = x.spawn(&pools::ENEMY, 0x15);
                o.set_u8(x.b, Obj::VARIANT, 8);
                o.set_u16(x.b, 0x42, 0x401);
                x.store_obj(ENEMY, o);
            }
        }
        10 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.set_v16(0x4ba, 0x100);
                x.wr16(TIMER, 12);
                crate::palette::set_flash_color(x.b, crate::palette::FLASH_COLOR_WHITE_VALUE);
                crate::camera_modes::start_shake(x.b, crate::camera_modes::shake::B, 60);
            }
        }
        12 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.set_v16(0x4ba, 0);
                x.wr16(TIMER, 0x100);
                let o = x.load_obj(ENEMY);
                crate::mem::wr32(x.b, o.0 + 4, 0x0206_0000);
            }
        }
        14 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scenes 5 and 20: waits for all the players in the game to be in state 10.
fn wait_players_10(x: &mut Ctx) {
    match x.sub() {
        0 => {
            x.next_sub();
            x.set_v8(0x67, 0);
            x.lock();
            x.fade_in();
            x.title_card();
            crate::objects::rank::reset(x.b);
            crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
        }
        2 => {
            if x.players_in_state(10) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 14 and variants: fade, title card, one waiting frame.
fn title_card(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a44);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 1);
            x.fade_in();
            x.title_card();
            crate::objects::rank::reset(x.b);
        }
        2 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 24 (Manticore): two dialogues (31, then 32 if a player is in state 4), a boss.
fn scene_24(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a46);
    const MESSAGE: u32 = scratch(0x9a48);
    const BOSS: u32 = scratch(0x9a4a);
    match x.sub() {
        0 => {
            x.next_sub();
            x.set_v8(0x67, 0);
            x.lock();
            x.wr16(TIMER, 0xb4);
            x.fade_in();
            crate::objects::rank::reset(x.b);
            x.title_card();
            crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
        }
        2 => {
            if x.players_in_state(6) {
                x.next_sub();
                x.message(31, MESSAGE);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                let mut found = false;
                for k in 0..4 {
                    let p = PLAYERS + k * PLAYER_STRIDE;
                    if rd8(x.b, p) != 0 {
                        let character = rd8(x.b, p + 3) as u16 >> 1;
                        found |= character as u8 == 2;
                    }
                }
                if found {
                    x.message(32, MESSAGE);
                } else {
                    x.next_sub();
                    x.wr16(TIMER, 60);
                }
            }
        }
        6 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                x.wr16(TIMER, 90);
            }
        }
        8 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.wr16(TIMER, 0xa0);
                let boss = x.spawn(&pools::BOSS, 6);
                x.store_obj(BOSS, boss);
            }
        }
        10 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 26 (Lich): fade, dialogue 34, then a sound command.
fn scene_26(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a46);
    const MESSAGE: u32 = scratch(0x9a48);
    match x.sub() {
        0 => {
            x.next_sub();
            x.set_v8(0x67, 0);
            x.lock();
            x.wr16(TIMER, 0x60);
            x.fade_in();
            crate::objects::rank::reset(x.b);
            crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(34, MESSAGE);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                x.wr16(TIMER, 30);
                creators::camera_in_mode_60(x.b);
            }
        }
        6 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 27: title card, dialogue 37.
fn scene_27(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a4c);
    const MESSAGE: u32 = scratch(0x9a4e);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 0x100);
            x.fade_in();
            x.title_card();
            crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(37, MESSAGE);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 28: title card, dialogue 140; an external trigger (`A5+0x4e`) jumps to sub-step 6.
fn scene_28(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a50);
    const MESSAGE: u32 = scratch(0x9a52);
    const MUSIC_STATE: u32 = scratch(0x9a54);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 0x100);
            x.fade_in();
            x.title_card();
            if x.rd16(MUSIC_STATE) == 0 {
                x.wr16(MUSIC_STATE, 1);
                crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
            }
            if x.v8(0x4e) != 0 {
                x.set_sub(6);
                x.wr16(TIMER, 1);
                if x.rd16(MUSIC_STATE) != 2 {
                    x.wr16(MUSIC_STATE, 2);
                    crate::sound::play_with(x.b, sfx::COMMAND_E17A4, crate::sound::Source { d4: Some(24), ..Default::default() });
                    x.world();
                    return;
                }
            }
            x.set_v32(0x38, 0x0459_5900);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(140, MESSAGE);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                x.wr16(TIMER, 80);
            }
        }
        6 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 32 (black dragon): title card; an opening boss (type 0x13) drives the rest; ends when the
/// players are done (state 6).
fn scene_32(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a50);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 0xe0);
            x.fade_in();
            x.title_card();
            crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.spawn(&pools::BOSS, 0x13);
            }
        }
        4 => {} // the opening boss itself moves to sub-step 6
        6 => {
            if x.players_in_state(6) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 34: partial input lock (0x0f), opening sound command.
fn scene_34(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a50);
    match x.sub() {
        0 => {
            begin(x);
            x.set_v8(var::INPUT_LOCK, 0x0f);
            x.wr16(TIMER, 1);
            x.fade_in();
            if x.v8(0x59) != 0 {
                creators::type_68_pure(x.b);
            }
            crate::sound::service(x.b, crate::sound::Service::GeneralStop);
            crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
            crate::sound::service(x.b, crate::sound::Service::Ff06 { lock: 144, line: 0 });
        }
        2 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scenes 33 and 36: simple fade, one waiting frame.
fn plain_fade(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a50);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 1);
            x.fade_in();
            crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 35: title card, music, one waiting frame.
fn title_card_music(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a50);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 1);
            x.fade_in();
            x.title_card();
            crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 59: title card, music, one waiting frame.
fn title_card_timed(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a5a);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 1);
            x.fade_in();
            x.title_card();
            crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 38: the number of players sets the duration; dialogue 68; the end leaves `A5+0x4a` in place.
fn scene_38(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a50);
    const MESSAGE: u32 = scratch(0x9a52);
    const DURATIONS: u32 = 0x22c28;
    match x.sub() {
        0 => {
            x.next_sub();
            x.set_v8(var::PLAYERS_FROZEN, 1);
            x.set_v8(var::INPUT_LOCK, 0);
            let players = x.v8(var::PLAYERS) as u32;
            let duration = x.prog16(DURATIONS + 2 * players);
            x.wr16(TIMER, duration);
            fading::DELAY.set(x.b, 2);
            fading::PAGES.set(x.b, 15);
            crate::palette::no_brightness(x.b);
            crate::kernel::create_task(x.b, 0x10, fading::RISE);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(68, MESSAGE);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                x.wr16(TIMER, 60);
            }
        }
        6 => {
            if x.tick(TIMER) {
                x.leave_phase();
                x.set_v8(0x42, 1);
                x.set_v8(var::HOLD, 0);
            }
        }
        _ => {}
    }
    x.inputs();
    x.world();
}

/// Scenes 39, 41, 43, 45: waits for the players in state 6; 41 also launches 0xdfa4a.
fn wait_players_6(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a56);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 1);
            x.fade_in();
            x.title_card();
            crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
        }
        2 => {
            if x.players_in_state(6) {
                if x.v8(var::SCENE) == 41 {
                    let _ = crate::effects_aids::scenery_11(x.b, 0, 0);
                }
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 40: players in state 4, grey lightning (0x1ab56) and four effects 0x13 (0x4cfbc), then a boss (type 12)
/// appears during the countdown.
fn scene_40(x: &mut Ctx) {
    boss_gate(x, Opening::Lightning, 0x60, 12, 0x90)
}

/// Scene 42: like 40 with a lightning bolt (0x1aaba) and rain (0xb9942), and the type 11 boss.
fn scene_42(x: &mut Ctx) {
    boss_gate(x, Opening::Rain, 0x40, 11, 0x80)
}

/// What scenes 40 and 42 launch when the players are in place.
#[derive(Clone, Copy)]
enum Opening {
    Lightning,
    Rain,
}

/// Subroutine common to scenes 40 and 42.
fn boss_gate(x: &mut Ctx, opening: Opening, spawn_at: u16, boss: u8, wait: u16) {
    const TIMER: u32 = scratch(0x9a56);
    match x.sub() {
        0 => {
            begin(x);
            x.fade_in();
            crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
        }
        2 => {
            if x.players_in_state(4) {
                x.next_sub();
                x.wr16(TIMER, wait);
                match opening {
                    Opening::Lightning => {
                        crate::palette::flash(x.b, crate::palette::Flash::Gray);
                        // Original glitch: the effects are linked to the "object" A0 left by the game loop (the
                        // address of the phase handler)
                        crate::families::creators::effects_13(x.b, Obj(x.phase_handler));
                    }
                    Opening::Rain => {
                        crate::palette::flash(x.b, crate::palette::Flash::B);
                        creators::rain(x.b);
                    }
                }
            }
        }
        4 => {
            if x.tick(TIMER) {
                x.end_scene();
                crate::palette::restore_flash(x.b);
            } else if x.rd16(TIMER) == spawn_at {
                x.spawn(&pools::BOSS, boss);
            }
        }
        _ => {}
    }
    x.world();
}

/// Scenes 44 and 111: type 0x11 boss, waits for state 10, dialogue 43.
fn scene_44(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a56);
    const MESSAGE: u32 = scratch(0x9a58);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 1);
            x.fade_in();
            if x.v8(0x30) == 0 {
                x.sobj_6(23);
            }
            crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
        }
        2 => {
            x.next_sub();
            let boss = x.spawn(&pools::BOSS, 0x11);
            boss.set_u8(x.b, Obj::VARIANT, 1);
        }
        4 => {}
        6 => {
            if x.players_in_state(10) {
                x.next_sub();
                x.wr16(TIMER, 48);
            }
        }
        8 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(43, MESSAGE);
            }
        }
        10 => {
            if x.message_done(MESSAGE) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 47: fade to a level in steps; the end waits for the end of the fade.
fn scene_47(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a5a);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 1);
            x.fade_in_stepped();
            x.title_card();
            crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
        }
        2 => {
            if fading::STEP.get(x.b) == 0 {
                x.end_scene();
                crate::video::clear_layer1_start(x.b);
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 54 (Tel'Arin): players in states 4 then 8, dialogue 43; a type 0x11 boss if `A5+0x66`.
fn scene_54(x: &mut Ctx) {
    const MESSAGE: u32 = scratch(0x9a5c);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(scratch(0x9a5a), 1);
            x.fade_in();
            x.title_card();
            if x.v8(0x66) == 0 {
                x.set_sub(4);
                let boss = x.spawn(&pools::BOSS, 0x11);
                boss.set_u8(x.b, Obj::VARIANT, 2);
            }
            crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
        }
        2 => {
            if x.players_in_state(4) {
                x.end_scene();
            }
        }
        4 => {
            if x.players_in_state(8) {
                x.next_sub();
                x.message(43, MESSAGE);
            }
        }
        6 => {
            if x.message_done(MESSAGE) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 58: a type 5 boss, title card; players in state 4, dialogue 75.
fn scene_58(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a5a);
    const MESSAGE: u32 = scratch(0x9a5c);
    match x.sub() {
        0 => {
            begin(x);
            x.fade_in();
            x.spawn(&pools::BOSS, 5);
            x.title_card();
            crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
        }
        2 => {
            if x.players_in_state(4) {
                x.next_sub();
                x.wr16(TIMER, 48);
            }
        }
        4 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(75, MESSAGE);
            }
        }
        6 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                x.wr16(TIMER, 8);
            }
        }
        8 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 64: effect 75, title card; players in state 4, dialogue 77.
fn scene_64(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a5a);
    const MESSAGE: u32 = scratch(0x9a5c);
    match x.sub() {
        0 => {
            begin(x);
            x.fade_in();
            x.spawn(&pools::EFFCT, 75);
            x.title_card();
            crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
        }
        2 => {
            if x.players_in_state(4) {
                x.next_sub();
                x.message(77, MESSAGE);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                x.wr16(TIMER, 0x100);
            }
        }
        6 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 65: players in state 6, dialogue 81; the end opens transition 0xeadd4.
fn scene_65(x: &mut Ctx) {
    const MESSAGE: u32 = scratch(0x9a5c);
    match x.sub() {
        0 => {
            begin(x);
            x.fade_in();
            x.title_card();
            crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
        }
        2 => {
            if x.players_in_state(6) {
                x.next_sub();
                x.message(81, MESSAGE);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                x.leave_phase();
                x.set_v8(0x42, 1);
                x.set_v8(var::HOLD, 0);
                crate::sound::play_global(x.b, sfx::COMMAND_EADD4);
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 66 (last level): chains three dialogues (82, 83, 84) around a forced scroll and
/// four effects 0x50; `A5+0x5c` jumps directly to the effects step.
fn scene_66(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a5a);
    const MESSAGE: u32 = scratch(0x9a5c);
    match x.sub() {
        0 => {
            if x.v8(0x5c) != 0 {
                x.set_sub(8);
                final_effects(x);
            } else {
                x.next_sub();
                x.wr16(scratch(0x9a60), 0);
                x.lock();
                x.fade_in_stepped();
                x.spawn(&pools::BOSS, 10);
            }
        }
        2 => {
            if x.players_in_state(4) {
                x.next_sub();
                x.message(82, MESSAGE);
                crate::video::clear_layer1_start(x.b);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                x.wr16(TIMER, 0x100);
            }
        }
        6 => {
            if x.tick(TIMER) {
                x.end_scene();
            }
            if x.rd16(TIMER) == 0x80 {
                crate::sound::play_global(x.b, sfx::COMMAND_21);
            }
        }
        8 => final_effects(x),
        10 => {
            if x.v16(0x51e) != 0 {
                x.next_sub();
                x.wr16(TIMER, 48);
                crate::sound::play_global(x.b, sfx::COMMAND_2D);
            }
        }
        12 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(83, MESSAGE);
                crate::video::clear_layer1_start(x.b);
            }
        }
        14 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                x.message(84, MESSAGE);
            }
        }
        16 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
            }
        }
        _ => {}
    }
    x.world();
}

/// Scene 66, sub-step 8: fade to a level in steps and four effects 0x50 (variants 0 to 3).
fn final_effects(x: &mut Ctx) {
    x.next_sub();
    x.lock();
    x.fade_in_stepped();
    for variant in 0..4 {
        let effect = x.spawn(&pools::EFFCT, 0x50);
        effect.set_u8(x.b, Obj::VARIANT, variant);
    }
    crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
}

/// Scene 105: an effect 0x24, dialogues 86 and 87, two possible endings (depending on `A5+0x5c`), dialogue 42.
fn scene_105(x: &mut Ctx) {
    const TIMER: u32 = scratch(0x9a5a);
    const MESSAGE: u32 = scratch(0x9a5c);
    const BOSS: u32 = scratch(0x9a5e);
    match x.sub() {
        0 => {
            begin(x);
            x.wr16(TIMER, 0x80);
            x.fade_in();
            let effect = x.spawn(&pools::EFFCT, 0x24);
            effect.set_u8(x.b, Obj::VARIANT, 2);
            crate::sound::play_global(x.b, sfx::SCENE_MUSIC);
        }
        2 => {
            if x.tick(TIMER) {
                x.next_sub();
                x.message(86, MESSAGE);
            }
        }
        4 => {
            if x.message_done(MESSAGE) {
                if x.v8(0x5c) == 0 {
                    x.set_sub(12);
                    x.wr16(TIMER, 0x40);
                    let finale = finale(x);
                    x.wr16(BOSS, finale.0 as u16);
                    crate::sound::play_global(x.b, sfx::COMMAND_E62FC);
                } else {
                    x.next_sub();
                    x.message(87, MESSAGE);
                    let effect = x.spawn(&pools::EFFCT, 0x50);
                    effect.set_u8(x.b, Obj::VARIANT, 6);
                }
            }
        }
        6 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                crate::sound::play_global(x.b, sfx::COMMAND_E62FC);
            }
        }
        8 => {
            if x.players_in_state(8) {
                x.next_sub();
                x.message(42, MESSAGE);
            }
        }
        10 => {
            if x.message_done(MESSAGE) {
                x.next_sub();
                let finale = finale(x);
                x.wr16(BOSS, finale.0 as u16);
            }
        }
        12 | 14 => {
            let boss = x.load_obj(BOSS);
            if boss.u8(x.b, 0x2f) != 0 {
                x.end_scene();
            }
        }
        _ => {}
    }
    x.world();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// All scenes have an opening sequence.
    #[test]
    fn one_sequence_per_scene() {
        for scene in 0..=114u8 {
            let _ = sequence(scene);
        }
    }
}

/// 0xb9aa0: finale object (type 74). Original glitch: it is linked to the "object" A0 left by the game loop (the
/// address of the phase handler), and the failure of the allocation is not tested (the A1 left is stored).
fn finale(x: &mut Ctx) -> Obj {
    let creator = Obj(x.phase_handler);
    let o = creators::finale(x.b, creator);
    creators::or_rest(x.b, o)
}
