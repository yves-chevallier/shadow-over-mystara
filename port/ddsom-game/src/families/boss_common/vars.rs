//! Work RAM variables read or written by the bosses (`A5 + offset`; those of all the enemies are in
//! `enemy_common::vars`, re-exported here, and those of the whole game in `crate::ram`). A meaning guessed from the code
//! alone is marked (h).

use crate::mem::A5;
use crate::ram::{Byte, Long, Word};

pub use crate::families::enemy_common::vars::*;

/// Stage clock, BCD minutes, seconds and frames (`docs/boucle.md`); a boss sets it to 4:59:59 (0x04595900) at its
/// creation.
pub const STAGE_CLOCK: Long = Long::a5(0x38);
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
/// Screen shake requested by an effect (h).
pub const SHAKE: Word = Word::a5(0x4be);
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
