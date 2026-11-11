//! What the screens (`modes/`, `hud/`, `service/`, `scene/`, `credits*.rs`, `text.rs`, `popups.rs`) share: the typed
//! ROM data they read ([`tables`]).

pub mod tables;
pub mod block;
pub mod obj;
pub mod vars;
pub mod credits;
pub mod credits_screen;
pub mod credits_task;
pub mod hud;
pub mod modes;
pub mod popups;
pub mod scene;
pub mod service;
