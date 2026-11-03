//! Work RAM variables read or written by the bosses (`A5 + offset`; those of all the enemies are in
//! `enemy_common::vars`, re-exported here, and those of the whole game in `crate::ram`). A meaning guessed from the code
//! alone is marked (h).

use crate::mem::A5;
use crate::ram::{Byte, Word};
pub use crate::ram::{SHAKE, STAGE_CLOCK};

pub use crate::families::enemy_common::vars::*;

/// Non-zero: the players are frozen by a scene script (set when a boss dies).
pub const PLAYERS_FROZEN: Byte = Byte::a5(0x42);
/// Non-zero while a boss fights (set at its creation, cleared at its death): the scenery waits, the frames of the
/// fragile players are cleared (`docs/couverture.md`) (h).
pub const BOSS_FIGHT: Byte = Byte::a5(0x50);
/// Non-zero while a scene script runs (h).
pub const SCRIPT_BUSY: Word = Word::a5(0x52);
/// Damage setting of the cabinet (word of the configuration); non-zero: the faster table of speeds of boss 9 (h).
pub const DAMAGE_SETTING: Word = Word::a5(0x70);
/// Sub-step of the game phase (progress of the scene entry).
pub const SUB_STEP: Byte = Byte::a5(0x2e);
/// Palette fade: pages to fade, steps left, delay between two steps.
pub const FADE_PAGES: Byte = Byte::a5(0x48e);
pub const FADE_STEPS: Byte = Byte::a5(0x48f);
pub const FADE_DELAY: Word = Word::a5(0x490);
/// Step of the scene sequence.
pub const SCENE_STEP: Byte = Byte::a5(0x4ff);
/// Mode of the camera (`camera_modes`); also a signal of the boss to the scripted players (throw, landing).
pub const CAMERA_MODE: Word = Word::a5(0x51e);
/// Previous main camera position (integer parts): the scroll of the frame is derived from it.
pub const PREV_CAMERA_X: Word = Word::a5(0x524);
pub const PREV_CAMERA_Y: Word = Word::a5(0x528);
/// Boss registered as an obstacle for the other objects (word, RAM address; 0 / positive: none).
pub const BODY_OWNER: Word = Word(A5 - 0x5fb2);
/// Object that a scene cinematic follows (word, RAM address) (h).
pub const SCENE_ACTOR: Word = Word(A5 - 0x55ba);
/// The "boss defeated" record read by the stage tally (`scene::tally`): the banner name (the type of the boss), its
/// first byte (0), and the index of the bonus (`Enemy::EVENT_KIND` of the boss). As a word, the name and its first byte.
pub const DEFEAT_NAME: Byte = Byte(A5 - 0x557c);
pub const DEFEAT_NAME_WORD: Word = Word(A5 - 0x557c);
pub const DEFEAT_FIRST: Byte = Byte(A5 - 0x557b);
pub const DEFEAT_BONUS: Byte = Byte(A5 - 0x557a);
/// Input mask of the players (0xff: free; 0x7f while a boss plays a cinematic attack: the players do not act) (h).
pub const INPUT_MASK: Byte = Byte::a5(0x43);
/// Non-zero: the blinking of the scenery objects is suspended (set by the black dragon during its cinematic) (h).
pub const BLINK_SUSPENDED: Byte = Byte::a5(0x33);
/// Display mode of the interface panels (`ram::DISPLAY_PANELS`).
pub use crate::ram::DISPLAY_PANELS;
/// Vertical scroll of plane 1 (`ram::SCROLL_Y_PLANE1`): toggled between 0 and 0x100 to shake the screen.
pub use crate::ram::SCROLL_Y_PLANE1;
/// Non-zero: the players do not walk into the scene (set by boss 17, variant 1) (`player::vars::NO_WALK_IN`) (h).
pub const NO_WALK_IN: Byte = Byte::a5(0x66);
/// Non-zero: no victory pose at the end of the scene (set by boss 21) (`player::vars::NO_VICTORY`) (h).
pub const NO_VICTORY: Byte = Byte::a5(0x5c);
/// Non-zero: the arrival cinematic of boss 17 or 21 may land (scene signal) (h).
pub const LANDING_CUE: Byte = Byte::a5(0x500);
/// Non-zero: boss 16 measures the screen edges from the alternative camera (`CAMERA_ALT_X`) (h).
pub const ALT_CAMERA_EDGES: Byte = Byte::a5(0xa);
/// Byte of the end countdown record (`A5-0x713a`, `hud::status`): non-zero, the black dragon withdraws and the
/// help of the SOBJ is inhibited.
pub const END_COUNTDOWN: Byte = Byte(A5 - 0x713a);
/// Step of the scene sequence of the menus and cinematics (`A5-0x55c8`; `shell::vars::SCENE_ICONS`).
pub const SCENE_SEQUENCE: Byte = Byte(A5 - 0x55c8);
/// High byte of the difficulty word `RANK` (0: the rank fits in the low byte), that boss 11 reads as a byte (h).
pub const RANK_HIGH: Byte = Byte::a5(0x7f92);
