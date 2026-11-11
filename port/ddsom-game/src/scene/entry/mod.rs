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

use crate::screens::vars as sv;
use crate::world::World;
use super::fading;

use super::{player, scratch, script, var, Ctx, PLAYERS, PLAYER_STRIDE};
use crate::families::effects_creators as creators;
use crate::game;
use crate::objects::Obj;
use crate::objects::pools;

mod levels;
mod later;
use levels::*;
use later::*;

/// ROM tables (data), indexed by scene number.
pub(crate) mod table {
    /// One byte per scene copied to `A5+0x48` / `A5+0x49` (scene palette parameters).
    pub const PALETTE_A: crate::screens::tables::Rom = crate::screens::tables::Rom::at(0x21b38);
    pub const PALETTE_B: crate::screens::tables::Rom = crate::screens::tables::Rom::at(0x21bb0);
    /// One byte per scene (copied to `A5+0x3e` and `A5+0x40`), index ×2.
    pub const SCENE_BYTE: crate::screens::tables::Rom = crate::screens::tables::Rom::at(0x21156);
    /// Players' entry position: 4 pairs (X, Y) of words per scene.
    pub const START_POSITIONS: crate::screens::tables::Rom = crate::screens::tables::Rom::at(0x2123c);
    /// One byte per player and per scene (`+0x2c` of the player).
    pub const START_STATES: crate::screens::tables::Rom = crate::screens::tables::Rom::at(0x2196c);
}

/// Work RAM variables outside the A5 ± 0x7fff page, cleared on entry.
mod ram {
    use crate::ram::{Byte, Word};
    pub const BYTE_A: Byte = Byte(0xff_2472);
    pub const BYTE_B: Byte = Byte(0xff_2474);
    pub const BYTE_C: Byte = Byte(0xff_2042);
    pub const WORD_D: Word = Word(0xff_2046);
    pub const WORD_E: Word = Word(0xff_2048);
    pub const WORD_F: Word = Word(0xff_204a);
    pub const WORD_G: Word = Word(0xff_204c);
    pub const WORD_H: Word = Word(0xff_2040);
}

/// Player field: rank of arrival in the scene.
const ARRIVAL_RANK: u32 = 0x7d;

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
    var::STEP.set(x.b, STEP_SEQUENCE);
    for v in [ram::BYTE_A, ram::BYTE_B, ram::BYTE_C] {
        v.set(x.b, 0);
    }
    for v in [ram::WORD_D, ram::WORD_E, ram::WORD_F] {
        v.set(x.b, 0);
    }
    ram::WORD_G.set(x.b, 5);
    ram::WORD_H.set(x.b, 9);
    x.set_v8(0x5f, 1);
    var::INPUT_LOCK.set(x.b, 0xff);
    x.set_v8(0x3c, 1);
    x.set_v8(0x3d, 0);
    var::HOLD.set(x.b, 0);
    x.set_v16(0x54, 0);
    x.set_v16(0x52, 0);
    x.set_v8(0x58, 0);
    x.set_v8(0x51, 0);
    let scene = var::SCENE.get(x.b) as u32;
    let (pa, pb) = (table::PALETTE_A.byte(x.b, scene), table::PALETTE_B.byte(x.b, scene));
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
    let marker = table::SCENE_BYTE.byte(x.b, 2 * scene);
    x.set_v8(0x3e, marker);
    x.set_v8(0x40, marker);
    let mut alive = 0u8;
    for k in 0..4 {
        let p = Obj(PLAYERS + k * PLAYER_STRIDE);
        if p.u8(x.b, player::ACTIVE) != 0 && p.u16(x.b, player::HEALTH) != 0 {
            alive += 1;
            p.set_u32(x.b, Obj::STATE, 0);
        }
    }
    var::PLAYERS.set(x.b, alive);
    place_players(x.b);
}

fn clear_player(x: &mut Ctx, p: u32) {
    let p = Obj(p);
    for off in PLAYER_CLEAR_BYTES {
        p.set_u8(x.b, off, 0);
    }
    for off in PLAYER_CLEAR_WORDS {
        p.set_u16(x.b, off, 0);
    }
    for off in PLAYER_CLEAR_LONGS {
        p.set_u32(x.b, off, 0);
    }
    for off in PLAYER_SPELL_SLOTS.step_by(4) {
        p.set_u32(x.b, off, 0);
    }
}

/// 0x20de6: places each player in the game in front of the camera, from the scene tables (consumed in the order
/// of the players in the game). Returns the number of players placed: the rank of the next one (D2, that the original
/// leaves to its callers).
pub fn place_players(b: &mut World) -> u8 {
    let scene = var::SCENE.get(b) as u32;
    let mut positions = scene << 4;
    let mut states = scene << 2;
    let (camera_x, camera_y) =
        if var::USE_ALT_CAMERA.get(b) != 0 { (sv::CAMERA_2_X, sv::CAMERA_2_Y) } else { (sv::CAMERA_X, sv::CAMERA_Y) };
    let mut rank = 0u8;
    for k in 0..4 {
        let p = PLAYERS + k * PLAYER_STRIDE;
        if Obj(p).u8(b, player::ACTIVE) == 0 || Obj(p).u16(b, player::HEALTH) == 0 {
            continue;
        }
        let (dx, dy) = (camera_x.get(b), camera_y.get(b));
        let (px, py) = (table::START_POSITIONS.word(b, positions), table::START_POSITIONS.word(b, positions + 2));
        positions += 4;
        let p = Obj(p);
        p.set_u16(b, Obj::X, px.wrapping_add(dx));
        p.set_u16(b, Obj::Y, py.wrapping_add(dy));
        p.set_z(b, 0);
        let state = table::START_STATES.byte(b, states);
        p.set_u8(b, crate::player::Player::FACING, state);
        states += 1;
        p.set_u8(b, ARRIVAL_RANK, rank);
        rank += 1;
    }
    rank
}

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
pub fn phase_8(b: &mut World, handler: u32) {
    let mut x = Ctx::new(b);
    x.phase_handler = handler;
    match var::STEP.get(x.b) {
        STEP_INIT => init_scene(&mut x),
        STEP_SEQUENCE => {
            let scene = var::SCENE.get(x.b);
            sequence(scene)(&mut x);
            script::step(x.b);
            game::debug_scene_skip(x.b);
        }
        _ => {}
    }
}

// --- stateless sequences -----------------------------------------------------------------------

/// Scene without its own sequence: an opening fade, the music, and the game starts.
fn quick(x: &mut Ctx) {
    x.leave_phase();
    var::PLAYERS_FROZEN.set(x.b, 0);
    x.fade_in();
    crate::sound::play_global(x.b, crate::sound::SCENE_MUSIC);
    x.world();
}

/// Scenes 25, 106-108: like `quick`, the music starts only once (`A5+0x67`).
fn quick_with_music(x: &mut Ctx) {
    x.leave_phase();
    var::PLAYERS_FROZEN.set(x.b, 0);
    x.fade_in();
    if x.v8(0x67) == 0 {
        x.set_v8(0x67, 1);
        crate::sound::play_global(x.b, crate::sound::SCENE_MUSIC);
    }
    x.world();
}

/// Scenes 90-104 (attract demos): fade from white and system object 6/23.
fn white_start(x: &mut Ctx) {
    x.leave_phase();
    var::PLAYERS_FROZEN.set(x.b, 0);
    fading::PAGES.set(x.b, 15);
    crate::palette::full_brightness(x.b);
    x.sobj_6(23);
    x.world();
}

/// Scenes 109, 112-114: fade, system object 6/23, music.
fn quick_with_sobj(x: &mut Ctx) {
    x.leave_phase();
    var::PLAYERS_FROZEN.set(x.b, 0);
    x.fade_in();
    x.sobj_6(23);
    crate::sound::play_global(x.b, crate::sound::SCENE_MUSIC);
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
    var::HOLD.set(x.b, 0);
    var::INPUT_LOCK.set(x.b, 0x0f);
    var::PLAYERS_FROZEN.set(x.b, 1);
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

/// 0xb9aa0: finale object (type 74). Original glitch: it is linked to the "object" A0 left by the game loop (the
/// address of the phase handler), and the failure of the allocation is not tested (the A1 left is stored).
fn finale(x: &mut Ctx) -> Obj {
    let creator = Obj(x.phase_handler);
    let o = creators::finale(x.b, creator);
    creators::or_rest(x.b, o)
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
