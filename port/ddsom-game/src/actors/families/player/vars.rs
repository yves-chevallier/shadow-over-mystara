//! Work RAM variables read or written by the players' shots and effects (`A5 + offset`; the shared ones are in
//! `crate::world::ram`). A meaning guessed from the code alone is marked (h).

use crate::world::ram::{Byte, Long, Word};

pub use crate::actors::player::ram::{FRAME_COUNTER, FROZEN as PAUSED, KILL_FLAG, PAUSE, SLOW_MODE};
pub use crate::world::ram::{
    CABINET as VERSION, CAMERA_ALT_ACTIVE as USE_CAM2, CAMERA_ALT_X as CAM2_X, CAMERA_ALT_Y as CAM2_Y,
    CAMERA_X as CAM_X, CAMERA_Y as CAM_Y, PLAYERS_ALIVE, SCENE, SCROLL_Y_PLANE1,
};

/// Main camera abscissa as a long (16.16).
pub const CAM_X_LONG: Long = Long::a5(0x506);
/// Main camera abscissa of the previous frame (long).
pub const PREV_CAM_X: Long = Long::a5(0x524);
/// Non-zero: alternate-palette version (`tst.w (0x70,A5)`).
pub const ALT_PALETTE: Word = Word::a5(0x70);
/// Second pass of the object loop in this frame (h).
pub const SECOND_PASS: Byte = Byte::a5(0x5e);
