//! CPS2 video hardware: graphics ROMs, the tile atlas, the frame as drawing commands ([`DrawList`]) and the CPS2
//! renderer.
//! Translated from MAME (src/mame/capcom/cps1_v.cpp, cps2.cpp), BSD-3-Clause license.

pub mod atlas;
pub mod draw;
pub mod render;
pub mod rom;

pub use atlas::TileAtlas;
pub use draw::DrawList;
pub use render::{render, VideoState, SCREEN_H, SCREEN_W};
