//! Work RAM variables that the SET types read and write: offsets from A5, for the `a5_*` accessors of the context.
//!
//! The names come from `crate::ram`, `player` and the docs when the variable is known there; a guessed meaning is
//! marked (h). The players' records are `crate::ram::PLAYERS` (fields: `crate::player::Player`).

/// Input bits newly pressed this frame, one byte per player (h).
pub const INPUTS_NEW: u32 = 0x0c;
/// Game phase (step of 2: 8 scene entry, 0xc game, 0x10 exit, 0x14 end of game), its step and sub-steps: the long
/// writes all four at once.
pub const PHASE: u32 = 0x2c;
pub const STEP: u32 = 0x2d;
pub const SUB_STEPS: u32 = 0x2e;
/// A game is in progress.
pub const GAME: u32 = 0x30;
/// Bonus screen shown (h): set and cleared around the bonus selection of SET 15.
pub const BONUS_SCREEN: u32 = 0x33;
/// Scene number.
pub const SCENE: u32 = 0x34;
/// Number of players alive (0: no game in progress, the SET types do nothing).
pub const PLAYERS_ALIVE: u32 = 0x35;
/// Alternative camera selected.
pub const CAMERA_ALT_ACTIVE: u32 = 0x36;
/// Score of the level (h), signed word read by the bonus of SET 15.
pub const LEVEL_SCORE: u32 = 0x38;
/// The game is paused.
pub const PAUSED: u32 = 0x3d;
/// The scene is frozen: transition, end of game or cutscene in progress.
pub const FROZEN: u32 = 0x42;
/// Mask of the players' inputs (0x1f: inputs ignored, 0xff: free).
pub const INPUT_MASK: u32 = 0x43;
/// Word: object holding the trampoline lock, or camera focus (h).
pub const CAMERA_FOCUS: u32 = 0x46;
/// Pause: one bit per player.
pub const PAUSE: u32 = 0x4a;
/// Passage in progress through a door (h): counted by the doors of SET B.
pub const PASSAGE: u32 = 0x4e;
/// An exit is locked (no other door may act).
pub const EXIT_LOCK: u32 = 0x50;
/// Number of active scene controllers (SET 2).
pub const SCENES: u32 = 0x51;
/// Word: the door (low word of its address) that took over the exit.
pub const OWNER_GATE: u32 = 0x52;
/// A transition between two scenes is in progress (SET 2).
pub const TRANSITION: u32 = 0x58;
/// Slow motion of the game (h).
pub const SLOW_MODE: u32 = 0x59;
/// The bonus of a third clear is denied (h): tested by the reward tests of SET 15.
pub const NO_THIRD_BONUS: u32 = 0x5d;
/// Players may walk into the scene (zero: the scene controller frees its markers 10 and 11).
pub const NO_WALK_IN: u32 = 0x66;
/// VBLANK frame counter: low byte, low word, parity byte.
pub const FRAMES_LOW: u32 = 0x6d;
pub const FRAMES_WORD: u32 = 0x6e;
pub const FRAME_PARITY: u32 = 0x6f;
/// Screen fade: pages to fade, steps, delay before it starts (`show_message`, fade tasks).
pub const FADE_PAGES: u32 = 0x48e;
pub const FADE_STEPS: u32 = 0x48f;
pub const FADE_DELAY: u32 = 0x490;
/// Desired vertical scroll of plane 1.
pub const SCROLL_Y_PLANE1: u32 = 0x4ba;
/// Level end of the bonus game reached (h): set to 4 by SET 11.
pub const BONUS_END: u32 = 0x4ff;
/// Main camera: position (16.16 longs, or their integer words), and the copies of the previous frame.
pub const CAMERA_X: u32 = 0x506;
pub const CAMERA_Y: u32 = 0x50a;
pub const CAMERA_X_PREV: u32 = 0x50e;
pub const CAMERA_Y_PREV: u32 = 0x512;
/// Scroll limit word: the camera stops there (4: blocked) (h).
pub const SCROLL_STOP: u32 = 0x51e;
/// Right bound of the camera (h).
pub const CAMERA_MAX_X: u32 = 0x52c;
/// Left bound of the camera (h).
pub const CAMERA_MIN_X: u32 = 0x532;
/// Plane 2 shown (h): cleared by the title cards of SET 2.
pub const PLANE2_SHOWN: u32 = 0x54f;
/// Alternative camera (plane 2): position and copies of the previous frame.
pub const CAMERA_ALT_X: u32 = 0x556;
pub const CAMERA_ALT_Y: u32 = 0x55a;
pub const CAMERA_ALT_X_PREV: u32 = 0x55e;
pub const CAMERA_ALT_Y_PREV: u32 = 0x562;
/// Parameter of the title card (h).
pub const TITLE_CARD: u32 = 0x56e;
/// Camera speed of the scene start (h).
pub const CAMERA_SPEED: u32 = 0x574;
/// Scroll mode of the scene (h): saved and restored with the camera by the doors.
pub const SCROLL_MODE: u32 = 0x57c;
/// Text engine: the first cell (8 bytes per cell).
pub const TEXT_SLOTS: u32 = 0x5c6;
/// Rank (dynamic difficulty).
pub const RANK: u32 = 0x7f92;
/// Music of the scene (h): saved by the doors, which play it again on the way back.
pub const MUSIC: u32 = 0x7fc4;
