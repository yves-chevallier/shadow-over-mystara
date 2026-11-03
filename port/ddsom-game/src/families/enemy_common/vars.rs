//! Work RAM variables read by the enemies and bosses (`A5 + offset`; `crate::ram` for those of the whole game). A
//! meaning guessed from the code alone is marked (h).

use crate::ram::{Word, Byte};
pub use crate::ram::{CAMERA1_X, CAMERA1_Y, CAMERA2_X, CAMERA2_Y, LEVEL, SCENE_LEVEL};
pub use crate::ram::{CAMERA_ALT_ACTIVE as CAMERA_SELECT, CAMERA_ALT_X, CAMERA_ALT_Y, GAME as GAME_ON, PHASE, SCENE};
/// Main camera: position (word; `SCROLL_Y` can also be read as a long with the fraction).
pub use crate::ram::{CAMERA_X as SCROLL_X, CAMERA_Y as SCROLL_Y};
/// Object followed by the camera (word, RAM address; 0 = none): the "leader" that some bosses accompany.
pub const LEADER: Word = Word::a5(0x46);
/// Non-zero byte: everything is frozen (the boss handler does nothing).
pub const FROZEN: Byte = Byte::a5(0x4e);
/// Number of living players (`ram::DIFFICULTE` designates the same byte: to be checked).
pub const PLAYERS_ALIVE: Byte = Byte::a5(0x35);
/// Byte at 1 during a cutscene (game paused, inputs cut).
pub const PAUSE: Byte = Byte::a5(0x3d);
/// Set to 0xff when a boss dies (`effects::vars::BLOCKED`: the interface effects stay still).
pub const BOSS_DOWN: Byte = Byte::a5(0x4f);
/// Frame counter (low word; `effects::vars::FRAME_LOW`). Some enemies mask it in place (h: a glitch of the
/// original, which meant a copy).
pub const FRAME_COUNTER: Word = Word::a5(0x6e);
/// Rank (dynamic difficulty, `objects::rank`), 0 to 31: indexes the enemies' decision tables.
pub const RANK: Word = Word::a5(0x7f92);
/// Visibility of the two Nagpa (type 28) of the scene: bit 1 for variant 0, bit 0 for variant 1 (`A5-0x5f9c`).
pub const NAGPA_VISIBLE: Byte = Byte(0xff2064);
