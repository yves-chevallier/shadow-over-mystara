//! CPS2 video hardware: graphics ROMs, the tile atlas and rendering.
//! Translated from MAME (src/mame/capcom/cps1_v.cpp, cps2.cpp), BSD-3-Clause license.

pub mod atlas;
pub mod render;
pub mod rom;

pub use atlas::TileAtlas;
pub use render::{render, VideoState, SCREEN_H, SCREEN_W};
