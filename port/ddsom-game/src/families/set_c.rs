//! Types SET 2 (scene controller, 0xbc814), 3 (text window, 0xc8352) and 15 (0xce024): see
//! `docs/families/set_c.md`.
//! The tables (event scripts, animation set addresses...) stay in the ROM.

use super::cx::handlers;

/// Typed views of the SET C objects.
pub mod fields;

mod scene;
mod finale;
mod window;

handlers! {
    SET_SCENE_BC814 0xbc814 "set_scene_bc814" scene::run,
    SET_WINDOW_C8352 0xc8352 "set_window_c8352" window::run,
    SET_SCREENS_CE024 0xce024 "set_screens_ce024" finale::run,
}
