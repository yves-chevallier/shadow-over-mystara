//! Matériel vidéo CPS2 : ROMs graphiques et rendu.
//! Traduit de MAME (src/mame/capcom/cps1_v.cpp, cps2.cpp), licence BSD-3-Clause.

pub mod render;
pub mod rom;

pub use render::{render, VideoState, SCREEN_H, SCREEN_W};
