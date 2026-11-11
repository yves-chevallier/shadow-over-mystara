//! Layer `screens` (docs/fin.md, section 2): the screens and sequences (`modes/`, `hud/`, `service/`, `scene/`,
//! credits), and what they share (`block`, `obj`, `vars`; their typed ROM data are `world::screen_tables`).

pub mod block;
pub mod obj;
pub mod vars;
pub mod credits;
pub mod credits_screen;
pub mod credits_task;
pub mod hud;
pub mod modes;
pub mod scene;
pub mod service;
