//! Work RAM variables read or written by the effects (`A5 + offset`; the shared ones are in `crate::ram`). A
//! meaning guessed from the code alone is marked (h).

use crate::ram::{Byte, Long, Word};
pub use crate::ram::{CAMERA1_X, CAMERA1_Y, CAMERA2_Y, SHAKE};

/// Inputs of the players for this frame, one byte per player slot (bits 0-3 directions, 4-7 buttons) (h).
pub const INPUTS: Byte = Byte::a5(0xc);
/// Game phase, step and sub-steps as one long (`crate::ram::PHASE`).
pub const PHASE_LONG: Long = Long::a5(0x2c);
/// Sub-step of the game phase (`crate::ram::STEP`).
pub const SUB_STEP: Byte = Byte::a5(0x2e);
/// Non-zero: the action is suspended (scene script, dialog) (h).
pub const PAUSE: Byte = Byte::a5(0x3d);
/// Non-zero: the players are frozen by a scene script (h).
pub const FROZEN: Byte = Byte::a5(0x42);
/// Non-zero: the inputs of the players are ignored (h).
pub const INPUTS_LOCKED: Byte = Byte::a5(0x44);
/// Leading object followed by the camera (16-bit pointer) (h).
pub const LEADER: Word = Word::a5(0x46);
/// Non-zero: the players are driven by a scene script (h).
pub const SCRIPTED: Byte = Byte::a5(0x4e);
/// Non-zero: the effects of the interface stay still (h).
pub const BLOCKED: Byte = Byte::a5(0x4f);
/// Non-zero while a scene script runs (h).
pub const SCRIPT_BUSY: Word = Word::a5(0x52);
/// Slow motion of the game (h).
pub const SLOW_MODE: Byte = Byte::a5(0x59);
/// Non-zero on the second pass of a scene (shop of scene 0x4c) (h).
pub const SECOND_PASS: Byte = Byte::a5(0x5e);
/// Non-zero while a passage to a sub-scene is open (h).
pub const GATE_ACTIVE: Byte = Byte::a5(0x5f);
/// Low word of the frame counter (`A5+0x6c`).
pub const FRAME_LOW: Word = Word::a5(0x6e);
/// Low byte of the frame counter: phase of the "one frame out of n" effects.
pub const FRAME_BYTE: Byte = Byte::a5(0x6f);
/// Damage setting of the cabinet (word of the configuration) (h).
pub const DAMAGE_SETTING: Word = Word::a5(0x70);
/// Number of players of the cabinet (2 or 4): indexes the tables by number of players, and with 2 the texts of the
/// text layer are in words (h).
pub const CABINET_PLAYERS: Word = Word::a5(0x72);
/// Palette fade: pages to fade.
pub const FADE_PAGES: Byte = Byte::a5(0x48e);
/// Palette fade: steps left (non-zero while fading).
pub const FADE_STEP: Byte = Byte::a5(0x48f);
/// Palette fade: delay between two steps.
pub const FADE_DELAY: Word = Word::a5(0x490);
/// Palette fade: source palettes.
pub const FADE_SOURCES: Long = Long::a5(0x492);
/// Source of the scroll 1 blocks of the scene (h).
pub const BLOCK_SOURCE_1: Long = Long::a5(0x49a);
/// Source of the scroll 2 blocks of the scene (h).
pub const BLOCK_SOURCE_2: Long = Long::a5(0x49e);
/// High word of `BLOCK_SOURCE_2` (h).
pub const BLOCK_SOURCE_2_WORD: Word = Word::a5(0x49e);
/// Step of the scene sequence.
pub const SCENE_STEP: Byte = Byte::a5(0x4ff);
/// Main camera x as a 16.16 long (`crate::ram::CAMERA_X`).
pub const CAMERA_X_LONG: Long = Long::a5(0x506);
/// Main camera y as a 16.16 long (`crate::ram::CAMERA_Y`).
pub const CAMERA_Y_LONG: Long = Long::a5(0x50a);
/// Target of the main camera, x (16.16) (h).
pub const CAMERA_TARGET_X: Long = Long::a5(0x50e);
/// Target of the main camera, y (16.16) (h).
pub const CAMERA_TARGET_Y: Long = Long::a5(0x512);
/// Mode of the camera (`camera_modes`).
pub const CAMERA_MODE: Word = Word::a5(0x51e);
/// Lock of the camera scrolling (h).
pub const CAMERA_LOCK: Word = Word::a5(0x52c);
/// Target of the alternate camera, x (h).
pub const CAMERA_ALT_TARGET_X: Word = Word::a5(0x55e);
/// Target of the alternate camera, y (h).
pub const CAMERA_ALT_TARGET_Y: Word = Word::a5(0x562);
