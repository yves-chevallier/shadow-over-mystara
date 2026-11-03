//! Work RAM variables that several screens share (the others are declared in their module). A variable written with
//! two widths (a camera position cleared as a long) is declared with its usual width; `Long(v.0)` gives the other.

use crate::ram::{Byte, Long, Word};

/// Video flags (word; bit 0 of the low byte `+0x13`: row scroll of layer 1) and image of the layer enable register.
pub const VIDEO_FLAGS: Word = Word::a5(0x12);
pub const VIDEO_FLAGS_LOW: Byte = Byte::a5(0x13);
pub const LAYERS: Word = Word::a5(0x14);
/// Cabinet type (number of players, h).
pub const CABINET: Byte = Byte::a5(0x20);
/// Fade from white: the four library palettes it starts from (ROM addresses, longs).
pub const FADE_SOURCES: Long = Long::a5(0x492);
/// Requested scroll of layers 1 and 2 (longs: x then y), copied to the registers in VBLANK.
pub const SCROLL_1: Long = Long::a5(0x4b6);
pub const SCROLL_2: Long = Long::a5(0x4ba);
/// Camera 1: scroll position x and y; mode of the camera.
pub const CAMERA_X: Word = Word::a5(0x506);
pub const CAMERA_Y: Word = Word::a5(0x50a);
pub const CAMERA_MODE: Word = Word::a5(0x51e);
pub const CAMERA_MODE_HIGH: Byte = Byte::a5(0x51e);
/// Scroll mode of the scenes (byte).
pub const SCROLL_MODE: Byte = Byte::a5(0x53c);
/// Camera 2: scroll position x and y.
pub const CAMERA_2_X: Word = Word::a5(0x556);
pub const CAMERA_2_Y: Word = Word::a5(0x55a);

/// Fade source `k` (0 to 3).
pub const fn fade_source(k: u32) -> Long {
    Long(FADE_SOURCES.0 + 4 * k)
}

/// Buttons held by player `k` (one byte per player, `A5+0..3`).
pub const fn buttons(k: u32) -> Byte {
    Byte::a5(k)
}

/// Rising edges of the buttons of player `k` (`A5+0xc..0xf`).
pub const fn pressed(k: u32) -> Byte {
    Byte::a5(0xc + k)
}

/// Scrolling text or announcement (h): pointer to the text (long), then two header words (x and y).
pub const BANNER: Long = Long::a5(0x5c6);
