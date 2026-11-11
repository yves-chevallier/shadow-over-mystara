//! Work RAM variables read or written by the ESHL, SOBJ, ITEM and BOX objects (`A5 + offset`; the shared ones
//! are in `crate::world::ram`). A meaning guessed from the code alone is marked (h).

use crate::world::ram::{Byte, Long, Word};
pub use crate::world::ram::{CAMERA1_X, CAMERA1_Y};

/// "Collectible picked up" flags, one byte per collectible object (48 longs cleared by `items::clear_picked_up`).
pub const PICKED: Byte = Byte(0xff2478); // A5 - 0x5b88
/// Stage 0x69: safe rectangles of the finale attack in effect, one per bit (h).
pub const SAFE_RECTS: Byte = Byte(0xff2064); // A5 - 0x5f9c
/// Crates broken in the scene.
pub const BROKEN: Byte = Byte(0xff24df); // A5 - 0x5b21
/// Pieces of equipment dropped at this hit of a player.
pub const DROPPED: Word = Word(0xff2538); // A5 - 0x5ac8

/// Collectible 5 picked up (entry of the flags `A5 - 0x5b88`, one byte per collectible).
pub const PICKED_5: Byte = Byte(0xff247d); // A5 - 0x5b83
/// Level reached, selects the animation of the guide (h).
pub const LEVEL: Byte = Byte(0xff253a); // A5 - 0x5ac6
/// Lock: a player is picking up a contact object.
pub const PICKUP_LOCK: Byte = Byte(0xff253c); // A5 - 0x5ac4
/// Counter of the scene icons, advanced by 4 by each (h).
pub const SCENE_ICONS: Byte = Byte(0xff2a38); // A5 - 0x55c8
/// Result of the SET object of the guide 3, copied by it.
pub const GUIDE_RESULT: Byte = Byte(0xff2ae4); // A5 - 0x551c
/// Non-zero: the blinking of the pickup sparkle is suspended.
pub const BLINK_SUSPENDED: Byte = Byte::a5(0x33);
/// Non-zero: the scene element of variant 3 takes its second list (h).
pub const ALT_LISTS: Word = Word::a5(0x38);
/// Game paused: the shots and objects do not move.
pub const GAME_PAUSED: Byte = Byte::a5(0x3d);
/// Non-zero: the players are frozen by a scene script (h).
pub const FROZEN: Byte = Byte::a5(0x42);
/// Lock of the screen taken by the finale attack (0x3f taken, 0xff free) (h).
pub const SCREEN_LOCK: Byte = Byte::a5(0x43);
/// Non-zero: the inputs of the players are ignored (h).
pub const INPUTS_LOCKED: Byte = Byte::a5(0x44);
/// Keys of the players.
pub const KEYS: Byte = Byte::a5(0x45);
/// Leading player followed by the camera (16-bit pointer) (h).
pub const LEADER: Word = Word::a5(0x46);
/// Non-zero: the players are driven by a scene script; freezes some shots (h).
pub const SCRIPTED: Byte = Byte::a5(0x4e);
/// Pickups of the equipment of variant 0xf by a class-1 player; at 8 it changes (h).
pub const SECRET_PICKUPS: Byte = Byte::a5(0x57);
/// Event of the pickup of a contact object (h).
pub const SECOND_PASS: Byte = Byte::a5(0x5e);
/// Low word of the frame counter (`A5+0x6c`): blinking phase.
pub const FRAME_LOW: Word = Word::a5(0x6e);
/// Low byte of the frame counter: phase of the "one frame out of n" effects.
pub const FRAME_BYTE: Byte = Byte::a5(0x6f);
/// Non-zero: the stage titles use their alternate script (h).
pub const ALT_TITLES: Word = Word::a5(0x70);
/// Number of players of the cabinet (2 or 4) (h).
pub const CABINET_PLAYERS: Word = Word::a5(0x72);
/// Palette fade: pages to fade.
pub const FADE_PAGES: Byte = Byte::a5(0x48e);
/// Palette fade: steps left (non-zero while fading).
pub const FADE_STEP: Byte = Byte::a5(0x48f);
/// Main camera x as a 16.16 long (`crate::world::ram::CAMERA_X`).
pub const CAMERA_X_LONG: Long = Long::a5(0x506);
/// Main camera y as a 16.16 long (`crate::world::ram::CAMERA_Y`).
pub const CAMERA_Y_LONG: Long = Long::a5(0x50a);
/// Target of the main camera, x (high word) (h).
pub const CAMERA_TARGET_X_WORD: Word = Word::a5(0x50e);
/// Target of the main camera, x (16.16) (h).
pub const CAMERA_TARGET_X: Long = Long::a5(0x50e);
/// Mode of the camera (`camera_modes`).
pub const CAMERA_MODE: Word = Word::a5(0x51e);
/// Previous main camera x (16.16): the scroll of the frame is derived from it.
pub const PREV_CAMERA_X: Long = Long::a5(0x524);
/// Previous main camera x, integer part.
pub const PREV_CAMERA_X_WORD: Word = Word::a5(0x524);
/// Previous main camera y (16.16).
pub const PREV_CAMERA_Y: Long = Long::a5(0x528);
/// Alternate camera x as a 16.16 long (`crate::world::ram::CAMERA_ALT_X`).
pub const CAMERA_ALT_X_LONG: Long = Long::a5(0x556);
/// Alternate camera y as a 16.16 long (`crate::world::ram::CAMERA_ALT_Y`).
pub const CAMERA_ALT_Y_LONG: Long = Long::a5(0x55a);
/// Target of the alternate camera, x (h).
pub const CAMERA_ALT_TARGET_X: Word = Word::a5(0x55e);
/// Target of the alternate camera, y (h).
pub const CAMERA_ALT_TARGET_Y: Word = Word::a5(0x562);
/// Previous alternate camera x (16.16).
pub const PREV_CAMERA_ALT_X: Long = Long::a5(0x574);
/// Scene script being read (address).
pub const SCENE_SCRIPT: Long = Long::a5(0x5c6);
/// First header word of the scene script (h).
pub const SCENE_SCRIPT_A: Word = Word::a5(0x5ca);
/// Second header word of the scene script (h).
pub const SCENE_SCRIPT_B: Word = Word::a5(0x5cc);
/// Limit of the twelve shots of type 5 (h).
pub const LIMIT_TWELVE: Word = Word::a5(0x7e64);
/// Difficulty rank (word): indexes the tables by rank / 2.
pub const RANK: Word = Word::a5(0x7f92);
